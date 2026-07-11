mod code_assist;
mod oauth;
mod quota;

use crate::{create_codex_home_if_missing, print_wrapped_stderr};
use anyhow::{Context, Result};
use code_assist::{GeminiCodeAssistSetupMode, resolve_gemini_code_assist_project_with_endpoint};
pub(crate) use code_assist::{
    ensure_gemini_code_assist_project_if_missing, gemini_code_assist_endpoint,
};
use oauth::{
    exchange_google_oauth_code, fetch_google_user_email, gemini_oauth_authorize_url,
    gemini_oauth_secret_from_token, open_browser, random_hex, refresh_google_oauth_token,
    wait_for_google_oauth_code,
};
use quota::verify_gemini_code_assist_quota_access;
pub(crate) use quota::{
    fetch_gemini_quota, fetch_gemini_quota_json, fetch_gemini_quota_with_code_assist_endpoint,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::Server as TinyServer;

pub(crate) const GEMINI_OAUTH_SECRET_FILE: &str = "gemini_oauth.json";
const GEMINI_OAUTH_EXPIRY_SKEW_MS: i64 = 60_000;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct GeminiOAuthSecret {
    pub(crate) auth_mode: String,
    pub(crate) access_token: String,
    #[serde(default)]
    pub(crate) refresh_token: Option<String>,
    #[serde(default)]
    pub(crate) token_type: Option<String>,
    #[serde(default)]
    pub(crate) scope: Option<String>,
    #[serde(default)]
    pub(crate) expiry_date: Option<i64>,
    pub(crate) email: String,
    #[serde(default)]
    pub(crate) project_id: Option<String>,
}

impl fmt::Debug for GeminiOAuthSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeminiOAuthSecret")
            .field("auth_mode", &"<redacted>")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "token_type",
                &self.token_type.as_ref().map(|_| "<redacted>"),
            )
            .field("scope", &self.scope.as_ref().map(|_| "<redacted>"))
            .field("expiry_date", &self.expiry_date.map(|_| "<redacted>"))
            .field("email", &"<redacted>")
            .field(
                "project_id",
                &self.project_id.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

pub(crate) fn gemini_oauth_secret_path(codex_home: &Path) -> PathBuf {
    codex_home.join(GEMINI_OAUTH_SECRET_FILE)
}

pub(crate) fn read_gemini_oauth_secret(codex_home: &Path) -> Result<GeminiOAuthSecret> {
    let path = gemini_oauth_secret_path(codex_home);
    let text = secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::SecretLocation::file(&path))
        .map_err(secret_file_read_error)
        .with_context(|| format!("failed to read {}", path.display()))?
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn secret_file_read_error(error: secret_store::SecretError) -> anyhow::Error {
    let is_non_regular_file = matches!(
        &error,
        secret_store::SecretError::InvalidLocation { reason }
            if reason.ends_with(" is not a regular secret file")
                || reason.ends_with("secret path contains a symlink")
    );
    let error = anyhow::Error::new(error);
    if is_non_regular_file {
        error.context("not a regular secret file")
    } else {
        error
    }
}

pub(crate) fn write_gemini_oauth_secret(
    codex_home: &Path,
    secret: &GeminiOAuthSecret,
) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    let path = gemini_oauth_secret_path(codex_home);
    let text =
        serde_json::to_string_pretty(secret).context("failed to serialize Gemini OAuth secret")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&path), &text)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn login_with_google_oauth(codex_home: &Path) -> Result<GeminiOAuthSecret> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Google OAuth HTTP client")?;
    let server = TinyServer::http("127.0.0.1:0")
        .map_err(|err| anyhow::anyhow!("failed to bind Google OAuth callback server: {err}"))?;
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("Google OAuth callback server did not expose a TCP address")?;
    let redirect_uri = format!("http://{listen_addr}/oauth2callback");
    let state = random_hex(24)?;
    let auth_url = gemini_oauth_authorize_url(&redirect_uri, &state)?;

    print_wrapped_stderr("Opening Google sign-in in your browser.");
    print_wrapped_stderr(&format!("If it does not open, visit: {auth_url}"));
    let _ = open_browser(&auth_url);

    let code = wait_for_google_oauth_code(&server, &state)?;
    let token = exchange_google_oauth_code(&client, &code, &redirect_uri)?;
    let email = fetch_google_user_email(&client, &token.access_token)?;
    let secret = complete_gemini_oauth_secret(
        &client,
        gemini_oauth_secret_from_token(email, token, None),
        GeminiCodeAssistSetupMode::Interactive,
    )?;
    write_gemini_oauth_secret(codex_home, &secret)?;
    Ok(secret)
}

fn complete_gemini_oauth_secret(
    client: &Client,
    mut secret: GeminiOAuthSecret,
    mode: GeminiCodeAssistSetupMode,
) -> Result<GeminiOAuthSecret> {
    let code_assist_endpoint = gemini_code_assist_endpoint();
    let project_id = resolve_gemini_code_assist_project_with_endpoint(
        client,
        &secret,
        &code_assist_endpoint,
        mode,
    )?
    .context("Gemini Code Assist setup did not return a project")?;
    secret.project_id = Some(project_id.clone());
    verify_gemini_code_assist_quota_access(
        client,
        &secret,
        &project_id,
        &code_assist_endpoint,
        mode,
    )?;
    Ok(secret)
}

pub(crate) fn refresh_gemini_oauth_secret_if_needed(
    codex_home: &Path,
) -> Result<GeminiOAuthSecret> {
    let secret = read_gemini_oauth_secret(codex_home)?;
    if !gemini_oauth_secret_expired(&secret) {
        return Ok(secret);
    }
    refresh_gemini_oauth_secret(codex_home, secret)
}

pub(crate) fn force_refresh_gemini_oauth_secret(codex_home: &Path) -> Result<GeminiOAuthSecret> {
    let secret = read_gemini_oauth_secret(codex_home)?;
    refresh_gemini_oauth_secret(codex_home, secret)
}

fn refresh_gemini_oauth_secret(
    codex_home: &Path,
    mut secret: GeminiOAuthSecret,
) -> Result<GeminiOAuthSecret> {
    let refresh_token = secret.refresh_token.as_deref().context(
        "Gemini OAuth refresh token is missing; run `prodex login` and choose Sign in with Google",
    )?;
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Google OAuth refresh HTTP client")?;
    let token = refresh_google_oauth_token(&client, refresh_token)?;
    secret.access_token = token.access_token;
    if let Some(refresh_token) = token.refresh_token {
        secret.refresh_token = Some(refresh_token);
    }
    if let Some(token_type) = token.token_type {
        secret.token_type = Some(token_type);
    }
    if let Some(scope) = token.scope {
        secret.scope = Some(scope);
    }
    secret.expiry_date = token.expires_in.map(gemini_oauth_expiry_date_ms);
    write_gemini_oauth_secret(codex_home, &secret)?;
    Ok(secret)
}

pub(crate) fn gemini_oauth_project_from_env() -> Option<String> {
    [
        "GOOGLE_CLOUD_PROJECT",
        "GOOGLE_CLOUD_PROJECT_ID",
        "GCLOUD_PROJECT",
    ]
    .into_iter()
    .find_map(|key| {
        env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn normalize_gemini_project_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn gemini_oauth_secret_expired(secret: &GeminiOAuthSecret) -> bool {
    let Some(expiry_date) = secret.expiry_date else {
        return false;
    };
    now_ms().saturating_add(GEMINI_OAUTH_EXPIRY_SKEW_MS) >= expiry_date
}

fn gemini_oauth_expiry_date_ms(expires_in_seconds: i64) -> i64 {
    now_ms().saturating_add(expires_in_seconds.saturating_mul(1000))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tiny_http::Response as TinyResponse;

    fn test_gemini_oauth_secret() -> GeminiOAuthSecret {
        GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-123".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: None,
            email: "user@example.com".to_string(),
            project_id: Some("project-123".to_string()),
        }
    }

    #[cfg(unix)]
    #[test]
    fn read_gemini_oauth_secret_rejects_symlink() {
        let root = env::temp_dir().join(format!(
            "prodex-gemini-oauth-symlink-{}-{}",
            std::process::id(),
            oauth::random_hex(4).expect("random suffix")
        ));
        std::fs::create_dir_all(&root).expect("test dir should be created");
        let target = root.join("target.json");
        std::fs::write(
            &target,
            serde_json::to_string_pretty(&test_gemini_oauth_secret()).unwrap(),
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, gemini_oauth_secret_path(&root)).unwrap();

        let err = read_gemini_oauth_secret(&root).expect_err("symlink secret must be rejected");

        assert!(err.to_string().contains("failed to read"));
        assert!(format!("{err:#}").contains("regular secret file"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gemini_oauth_secret_debug_output_redacts_sensitive_fields() {
        let secret = GeminiOAuthSecret {
            auth_mode: "gemini-oauth-secret-mode".to_string(),
            access_token: "gemini-access-token-secret".to_string(),
            refresh_token: Some("gemini-refresh-token-secret".to_string()),
            token_type: Some("Bearer-secret".to_string()),
            scope: Some("secret-scope".to_string()),
            expiry_date: Some(123_456_789),
            email: "alice@example.test".to_string(),
            project_id: Some("gemini-project-secret".to_string()),
        };
        let rendered = format!("{secret:?}");

        assert!(rendered.contains("GeminiOAuthSecret"));
        assert!(rendered.contains("<redacted>"));
        for raw in [
            "gemini-oauth-secret-mode",
            "gemini-access-token-secret",
            "gemini-refresh-token-secret",
            "Bearer-secret",
            "secret-scope",
            "123456789",
            "alice@example.test",
            "gemini-project-secret",
        ] {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }

    #[test]
    fn google_login_completion_requires_quota_probe_after_code_assist_setup() {
        let _env_lock = crate::TestEnvVarGuard::lock();
        let server = TinyServer::http("127.0.0.1:0").expect("setup test server should bind");
        let listen_addr = server.server_addr().to_ip().unwrap();
        let endpoint = format!("http://{listen_addr}/v1internal");
        let _endpoint_guard =
            crate::TestEnvVarGuard::set("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT", endpoint.as_str());
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("client should build");
        let secret = GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-123".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: Some(now_ms() + 3_600_000),
            email: "gemini-user@example.com".to_string(),
            project_id: None,
        };

        let handle = thread::spawn(move || {
            let mut load = server.recv().expect("loadCodeAssist request should arrive");
            assert_eq!(load.method().as_str(), "POST");
            assert_eq!(load.url(), "/v1internal:loadCodeAssist");
            let mut load_body = String::new();
            load.as_reader()
                .read_to_string(&mut load_body)
                .expect("loadCodeAssist body should read");
            assert!(load_body.contains("\"pluginType\":\"GEMINI\""));
            load.respond(TinyResponse::from_string(
                r#"{"currentTier":{"id":"standard-tier"},"cloudaicompanionProject":"gemini-project"}"#,
            ))
            .expect("loadCodeAssist response should send");

            let mut quota = server.recv().expect("quota probe should arrive");
            assert_eq!(quota.method().as_str(), "POST");
            assert_eq!(quota.url(), "/v1internal:retrieveUserQuota");
            let mut quota_body = String::new();
            quota
                .as_reader()
                .read_to_string(&mut quota_body)
                .expect("quota probe body should read");
            assert!(quota_body.contains("\"project\":\"gemini-project\""));
            quota
                .respond(
                    TinyResponse::from_string(
                        r#"{"error":{"code":403,"message":"Verify your account to continue.","status":"PERMISSION_DENIED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"VALIDATION_REQUIRED","domain":"cloudcode-pa.googleapis.com","metadata":{"validation_error_message":"Verify your account to continue.","validation_url":"https://accounts.google.com/verify"}},{"@type":"type.googleapis.com/google.rpc.Help","links":[{"description":"Verify your account","url":"https://accounts.google.com/verify"},{"description":"Learn more","url":"https://support.google.com/accounts?p=al_alert"}]}]}}"#,
                    )
                    .with_status_code(403),
                )
                .expect("validation response should send");
        });

        let err = complete_gemini_oauth_secret(
            &client,
            secret,
            GeminiCodeAssistSetupMode::NonInteractive,
        )
        .expect_err("login completion should not succeed while account validation is required");
        handle.join().expect("setup test server should finish");
        let message = format!("{err:#}");
        assert!(message.contains("Verify your account"));
        assert!(message.contains("https://accounts.google.com/verify"));
    }
}
