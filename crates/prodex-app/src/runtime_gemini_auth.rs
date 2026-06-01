mod code_assist;
mod oauth;
mod quota;

use crate::{create_codex_home_if_missing, print_wrapped_stderr};
use anyhow::{Context, Result};
use code_assist::{GeminiCodeAssistSetupMode, resolve_gemini_code_assist_project};
pub(crate) use code_assist::{
    ensure_gemini_code_assist_project_if_missing, gemini_code_assist_endpoint,
};
use oauth::{
    exchange_google_oauth_code, fetch_google_user_email, gemini_oauth_authorize_url,
    gemini_oauth_secret_from_token, open_browser, random_hex, refresh_google_oauth_token,
    wait_for_google_oauth_code,
};
pub(crate) use quota::{
    fetch_gemini_quota, fetch_gemini_quota_json, fetch_gemini_quota_with_code_assist_endpoint,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::Server as TinyServer;

pub(crate) const GEMINI_OAUTH_SECRET_FILE: &str = "gemini_oauth.json";
const GEMINI_OAUTH_EXPIRY_SKEW_MS: i64 = 60_000;

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
    match resolve_gemini_code_assist_project(
        &client,
        &secret,
        GeminiCodeAssistSetupMode::Interactive,
    ) {
        Ok(project_id) => {
            secret.project_id = project_id;
        }
        Err(err) => {
            print_wrapped_stderr(&format!(
                "Google sign-in succeeded, but Gemini Code Assist setup is incomplete: {err:#}"
            ));
        }
    }
    write_gemini_oauth_secret(codex_home, &secret)?;
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
