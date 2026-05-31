use crate::{create_codex_home_if_missing, print_wrapped_stderr};
use anyhow::{Context, Result, bail};
use prodex_quota::GeminiQuotaInfo;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Response as TinyResponse, Server as TinyServer};

pub(crate) const GEMINI_OAUTH_SECRET_FILE: &str = "gemini_oauth.json";
const GEMINI_OAUTH_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GEMINI_OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GEMINI_OAUTH_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const GEMINI_CODE_ASSIST_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com/v1internal";
const GEMINI_OAUTH_EXPIRY_SKEW_MS: i64 = 60_000;
const GEMINI_OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct GeminiTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GeminiUserInfoResponse {
    email: String,
}

pub(crate) fn gemini_oauth_secret_path(codex_home: &Path) -> PathBuf {
    codex_home.join(GEMINI_OAUTH_SECRET_FILE)
}

pub(crate) fn read_gemini_oauth_secret(codex_home: &Path) -> Result<GeminiOAuthSecret> {
    let path = gemini_oauth_secret_path(codex_home);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
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
    let mut secret = gemini_oauth_secret_from_token(email, token, None);
    secret.project_id = resolve_gemini_code_assist_project(&client, &secret);
    write_gemini_oauth_secret(codex_home, &secret)?;
    Ok(secret)
}

pub(crate) fn refresh_gemini_oauth_secret_if_needed(
    codex_home: &Path,
) -> Result<GeminiOAuthSecret> {
    let mut secret = read_gemini_oauth_secret(codex_home)?;
    if !gemini_oauth_secret_expired(&secret) {
        return Ok(secret);
    }
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

pub(crate) fn gemini_code_assist_endpoint() -> String {
    env::var("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT")
        .or_else(|_| env::var("GEMINI_CODE_ASSIST_ENDPOINT"))
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| GEMINI_CODE_ASSIST_ENDPOINT.to_string())
}

pub(crate) fn fetch_gemini_quota(
    codex_home: &Path,
    provider_project_id: Option<&str>,
) -> Result<GeminiQuotaInfo> {
    let value = fetch_gemini_quota_json(codex_home, provider_project_id)?;
    gemini_quota_info_from_value(codex_home, value)
}

pub(crate) fn fetch_gemini_quota_with_code_assist_endpoint(
    codex_home: &Path,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<GeminiQuotaInfo> {
    let value = fetch_gemini_quota_json_with_code_assist_endpoint(
        codex_home,
        provider_project_id,
        code_assist_endpoint,
    )?;
    gemini_quota_info_from_value(codex_home, value)
}

fn gemini_quota_info_from_value(codex_home: &Path, value: Value) -> Result<GeminiQuotaInfo> {
    serde_json::from_value(value).with_context(|| {
        format!(
            "invalid JSON returned by Gemini quota backend for {}",
            codex_home.display()
        )
    })
}

pub(crate) fn fetch_gemini_quota_json(
    codex_home: &Path,
    provider_project_id: Option<&str>,
) -> Result<Value> {
    let code_assist_endpoint = gemini_code_assist_endpoint();
    fetch_gemini_quota_json_with_code_assist_endpoint(
        codex_home,
        provider_project_id,
        &code_assist_endpoint,
    )
}

fn fetch_gemini_quota_json_with_code_assist_endpoint(
    codex_home: &Path,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<Value> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Gemini quota HTTP client")?;
    let mut secret = refresh_gemini_oauth_secret_if_needed(codex_home)?;
    let project_id = resolve_gemini_quota_project_id(
        &client,
        codex_home,
        &mut secret,
        provider_project_id,
        code_assist_endpoint,
    )?;
    let mut value =
        retrieve_gemini_user_quota(&client, &secret, &project_id, code_assist_endpoint)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("email".to_string(), Value::String(secret.email.clone()));
        object.insert("project_id".to_string(), Value::String(project_id));
    }
    Ok(value)
}

fn resolve_gemini_quota_project_id(
    client: &Client,
    codex_home: &Path,
    secret: &mut GeminiOAuthSecret,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<String> {
    if let Some(project_id) = normalize_gemini_project_id(provider_project_id) {
        return Ok(project_id);
    }
    if let Some(project_id) = normalize_gemini_project_id(secret.project_id.as_deref()) {
        return Ok(project_id);
    }
    if let Some(project_id) = gemini_oauth_project_from_env() {
        return Ok(project_id);
    }
    if let Some(project_id) =
        resolve_gemini_code_assist_project_with_endpoint(client, secret, code_assist_endpoint)
    {
        secret.project_id = Some(project_id.clone());
        write_gemini_oauth_secret(codex_home, secret)?;
        return Ok(project_id);
    }
    bail!(
        "Gemini quota requires a Code Assist project; run `prodex login --with-google` again or set GOOGLE_CLOUD_PROJECT"
    )
}

fn normalize_gemini_project_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn retrieve_gemini_user_quota(
    client: &Client,
    secret: &GeminiOAuthSecret,
    project_id: &str,
    code_assist_endpoint: &str,
) -> Result<Value> {
    let response = client
        .post(format!("{code_assist_endpoint}:retrieveUserQuota"))
        .bearer_auth(&secret.access_token)
        .json(&json!({
            "project": project_id,
        }))
        .send()
        .context("failed to fetch Gemini quota")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read Gemini quota response")?;
    if !status.is_success() {
        bail!(
            "Gemini quota request failed (HTTP {}): {body}",
            status.as_u16()
        );
    }
    serde_json::from_str(&body).context("failed to parse Gemini quota response")
}

fn gemini_oauth_authorize_url(redirect_uri: &str, state: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(GEMINI_OAUTH_AUTH_URL)
        .context("failed to parse Google OAuth authorize URL")?;
    let client_id = gemini_oauth_client_id();
    url.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &GEMINI_OAUTH_SCOPES.join(" "))
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", state);
    Ok(url.to_string())
}

fn wait_for_google_oauth_code(server: &TinyServer, expected_state: &str) -> Result<String> {
    loop {
        let request = server
            .recv()
            .context("failed to receive Google OAuth callback")?;
        let request_url = request.url().to_string();
        let parsed = reqwest::Url::parse(&format!("http://localhost{request_url}"))
            .context("failed to parse Google OAuth callback URL")?;
        if parsed.path() != "/oauth2callback" {
            let _ = request.respond(TinyResponse::from_string("Not found").with_status_code(404));
            continue;
        }
        let mut code = None;
        let mut state = None;
        let mut error = None;
        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "code" => code = Some(value.into_owned()),
                "state" => state = Some(value.into_owned()),
                "error" => error = Some(value.into_owned()),
                _ => {}
            }
        }
        if let Some(error) = error {
            let _ = request.respond(
                TinyResponse::from_string("Google sign-in failed. You can close this tab.")
                    .with_status_code(400),
            );
            bail!("Google sign-in failed: {error}");
        }
        if state.as_deref() != Some(expected_state) {
            let _ = request.respond(
                TinyResponse::from_string("Google sign-in state mismatch. You can close this tab.")
                    .with_status_code(400),
            );
            bail!("Google sign-in callback state mismatch");
        }
        let code = code.context("Google sign-in callback did not include an authorization code")?;
        let _ = request.respond(
            TinyResponse::from_string("Google sign-in complete. You can close this tab.")
                .with_status_code(200),
        );
        return Ok(code);
    }
}

fn exchange_google_oauth_code(
    client: &Client,
    code: &str,
    redirect_uri: &str,
) -> Result<GeminiTokenResponse> {
    let client_id = gemini_oauth_client_id();
    let client_secret = gemini_oauth_client_secret();
    let response = client
        .post(GEMINI_OAUTH_TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(oauth_form_body(&[
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ]))
        .send()
        .context("failed to exchange Google OAuth code")?;
    parse_google_token_response(response, "Google OAuth code exchange")
}

fn refresh_google_oauth_token(client: &Client, refresh_token: &str) -> Result<GeminiTokenResponse> {
    let client_id = gemini_oauth_client_id();
    let client_secret = gemini_oauth_client_secret();
    let response = client
        .post(GEMINI_OAUTH_TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(oauth_form_body(&[
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ]))
        .send()
        .context("failed to refresh Google OAuth token")?;
    parse_google_token_response(response, "Google OAuth token refresh")
}

fn gemini_oauth_client_id() -> String {
    gemini_oauth_env_value("PRODEX_GEMINI_OAUTH_CLIENT_ID")
        .or_else(|| gemini_oauth_env_value("GEMINI_OAUTH_CLIENT_ID"))
        .unwrap_or_else(|| {
            [
                "681255809395-",
                "oo8ft2oprdrnp9e3aqf6",
                "av3hmdib135j",
                ".apps.googleusercontent.com",
            ]
            .concat()
        })
}

fn gemini_oauth_client_secret() -> String {
    gemini_oauth_env_value("PRODEX_GEMINI_OAUTH_CLIENT_SECRET")
        .or_else(|| gemini_oauth_env_value("GEMINI_OAUTH_CLIENT_SECRET"))
        .unwrap_or_else(|| ["GOCSPX-", "4uHgMPm-", "1o7Sk-", "geV6Cu5", "clXFsxl"].concat())
}

fn gemini_oauth_env_value(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_google_token_response(
    response: reqwest::blocking::Response,
    context_label: &str,
) -> Result<GeminiTokenResponse> {
    let status = response.status();
    let body = response
        .text()
        .with_context(|| format!("failed to read {context_label} response"))?;
    if !status.is_success() {
        bail!("{context_label} failed (HTTP {}): {body}", status.as_u16());
    }
    serde_json::from_str(&body).with_context(|| format!("failed to parse {context_label} response"))
}

fn oauth_form_body(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn fetch_google_user_email(client: &Client, access_token: &str) -> Result<String> {
    let response = client
        .get(GEMINI_OAUTH_USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .context("failed to fetch Google user info")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read Google user info response")?;
    if !status.is_success() {
        bail!(
            "Google user info request failed (HTTP {}): {body}",
            status.as_u16()
        );
    }
    let user: GeminiUserInfoResponse =
        serde_json::from_str(&body).context("failed to parse Google user info response")?;
    if user.email.trim().is_empty() {
        bail!("Google user info response did not include an email");
    }
    Ok(user.email)
}

fn gemini_oauth_secret_from_token(
    email: String,
    token: GeminiTokenResponse,
    project_id: Option<String>,
) -> GeminiOAuthSecret {
    GeminiOAuthSecret {
        auth_mode: "gemini_oauth".to_string(),
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        token_type: token.token_type,
        scope: token.scope,
        expiry_date: token.expires_in.map(gemini_oauth_expiry_date_ms),
        email,
        project_id,
    }
}

fn resolve_gemini_code_assist_project(
    client: &Client,
    secret: &GeminiOAuthSecret,
) -> Option<String> {
    let code_assist_endpoint = gemini_code_assist_endpoint();
    resolve_gemini_code_assist_project_with_endpoint(client, secret, &code_assist_endpoint)
}

fn resolve_gemini_code_assist_project_with_endpoint(
    client: &Client,
    secret: &GeminiOAuthSecret,
    code_assist_endpoint: &str,
) -> Option<String> {
    let project_id = gemini_oauth_project_from_env();
    let response = client
        .post(format!("{code_assist_endpoint}:loadCodeAssist"))
        .bearer_auth(&secret.access_token)
        .json(&json!({
            "cloudaicompanionProject": project_id,
            "metadata": gemini_code_assist_metadata(project_id.as_deref()),
        }))
        .send()
        .ok()?;
    let status = response.status();
    let body = response.text().ok()?;
    if !status.is_success() {
        return project_id;
    }
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| gemini_code_assist_project_id(value.get("cloudaicompanionProject")?))
        .or(project_id)
}

fn gemini_code_assist_metadata(project_id: Option<&str>) -> serde_json::Value {
    json!({
        "ideType": "IDE_UNSPECIFIED",
        "platform": "PLATFORM_UNSPECIFIED",
        "pluginType": "GEMINI",
        "duetProject": project_id,
    })
}

fn gemini_code_assist_project_id(value: &serde_json::Value) -> Option<String> {
    value.as_str().map(str::to_string).or_else(|| {
        value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    })
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

fn random_hex(byte_count: usize) -> Result<String> {
    let mut bytes = vec![0u8; byte_count];
    getrandom::fill(&mut bytes).context("failed to generate Google OAuth state")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn open_browser(url: &str) -> Result<()> {
    if let Some(browser) = env::var_os("BROWSER").filter(|value| !value.is_empty()) {
        Command::new(browser)
            .arg(url)
            .spawn()
            .context("failed to open browser from BROWSER")?;
        return Ok(());
    }
    if cfg!(target_os = "macos") {
        Command::new("open")
            .arg(url)
            .spawn()
            .context("failed to open browser with open")?;
        return Ok(());
    }
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("failed to open browser with start")?;
        return Ok(());
    }
    if cfg!(all(unix, not(target_os = "macos"))) {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("failed to open browser with xdg-open")?;
        return Ok(());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn gemini_oauth_authorize_url_includes_scopes_and_state() {
        let url = gemini_oauth_authorize_url("http://127.0.0.1:1234/oauth2callback", "state-test")
            .expect("authorize URL should build");

        assert!(url.contains("client_id="));
        assert!(url.contains("state=state-test"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("cloud-platform"));
    }

    #[test]
    fn fetch_gemini_quota_uses_code_assist_retrieve_user_quota() {
        let server = TinyServer::http("127.0.0.1:0").expect("quota test server should bind");
        let listen_addr = server.server_addr().to_ip().unwrap();
        let endpoint = format!("http://{listen_addr}/v1internal");
        let _endpoint_guard =
            crate::TestEnvVarGuard::set("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT", endpoint.as_str());
        let root = temp_codex_home("quota");
        let secret = GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-123".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: Some(now_ms() + 3_600_000),
            email: "gemini-user@example.com".to_string(),
            project_id: Some("gemini-project".to_string()),
        };
        write_gemini_oauth_secret(&root, &secret).expect("Gemini OAuth secret should write");

        let handle = thread::spawn(move || {
            let mut request = server.recv().expect("quota request should arrive");
            assert_eq!(request.method().as_str(), "POST");
            assert_eq!(request.url(), "/v1internal:retrieveUserQuota");
            assert!(
                request.headers().iter().any(|header| {
                    header.field.equiv("authorization")
                        && header.value.as_str() == "Bearer token-123"
                }),
                "quota request should use Gemini OAuth bearer token"
            );
            let mut body = String::new();
            request
                .as_reader()
                .read_to_string(&mut body)
                .expect("quota request body should read");
            assert!(body.contains("\"project\":\"gemini-project\""));
            assert!(!body.contains("userAgent"));
            request
                .respond(TinyResponse::from_string(
                    r#"{"buckets":[{"modelId":"models/gemini-2.5-pro","remainingAmount":"50","remainingFraction":0.5,"resetTime":"2026-05-09T00:00:00Z"}]}"#,
                ))
                .expect("quota response should send");
        });

        let quota = fetch_gemini_quota(&root, None).expect("Gemini quota should fetch");
        handle.join().expect("quota test server should finish");

        assert_eq!(quota.email.as_deref(), Some("gemini-user@example.com"));
        assert_eq!(quota.project_id.as_deref(), Some("gemini-project"));
        assert_eq!(quota.buckets.len(), 1);
        assert_eq!(
            quota.buckets[0].model_id.as_deref(),
            Some("models/gemini-2.5-pro")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_codex_home(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("prodex-gemini-auth-{name}-{stamp}"))
    }
}
