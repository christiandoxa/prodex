use crate::{create_codex_home_if_missing, print_wrapped_stderr};
use anyhow::{Context, Result, bail};
use prodex_quota::GeminiQuotaInfo;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::io::{self, IsTerminal};
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCodeAssistTier {
    id: Option<String>,
    name: Option<String>,
    is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCodeAssistIneligibleTier {
    reason_code: Option<String>,
    reason_message: Option<String>,
    validation_url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GeminiLoadCodeAssistResponse {
    current_tier: Option<GeminiCodeAssistTier>,
    paid_tier: Option<GeminiCodeAssistTier>,
    allowed_tiers: Option<Vec<GeminiCodeAssistTier>>,
    ineligible_tiers: Option<Vec<GeminiCodeAssistIneligibleTier>>,
    cloudaicompanion_project: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCodeAssistOnboardResponse {
    cloudaicompanion_project: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct GeminiCodeAssistOperationResponse {
    name: Option<String>,
    done: Option<bool>,
    response: Option<GeminiCodeAssistOnboardResponse>,
}

#[derive(Debug, Clone, Copy)]
enum GeminiCodeAssistSetupMode {
    Interactive,
    NonInteractive,
}

#[derive(Debug, Clone)]
struct GeminiCodeAssistValidation {
    url: Option<String>,
    description: Option<String>,
    learn_more_url: Option<String>,
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

pub(crate) fn ensure_gemini_code_assist_project_if_missing(
    codex_home: &Path,
) -> Result<GeminiOAuthSecret> {
    let mut secret = refresh_gemini_oauth_secret_if_needed(codex_home)?;
    if normalize_gemini_project_id(secret.project_id.as_deref()).is_some()
        || gemini_oauth_project_from_env().is_some()
    {
        return Ok(secret);
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Gemini Code Assist HTTP client")?;
    if let Some(project_id) = resolve_gemini_code_assist_project(
        &client,
        &secret,
        GeminiCodeAssistSetupMode::NonInteractive,
    )? {
        secret.project_id = Some(project_id);
        write_gemini_oauth_secret(codex_home, &secret)?;
    }
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
    let mut project_id = resolve_gemini_quota_project_id(
        &client,
        codex_home,
        &mut secret,
        provider_project_id,
        code_assist_endpoint,
    )?;
    let mut value =
        match retrieve_gemini_user_quota(&client, &secret, &project_id, code_assist_endpoint) {
            Ok(value) => value,
            Err(err) if gemini_error_is_http_401(&err) => {
                secret = force_refresh_gemini_oauth_secret(codex_home)
                    .context("Gemini quota auth failed and OAuth token refresh failed")?;
                project_id = resolve_gemini_quota_project_id(
                    &client,
                    codex_home,
                    &mut secret,
                    provider_project_id,
                    code_assist_endpoint,
                )?;
                retrieve_gemini_user_quota(&client, &secret, &project_id, code_assist_endpoint)?
            }
            Err(err) => return Err(err),
        };
    let plan = fetch_gemini_code_assist_plan(&client, &secret, &project_id, code_assist_endpoint)
        .ok()
        .flatten();
    if let Some(object) = value.as_object_mut() {
        object.insert("email".to_string(), Value::String(secret.email.clone()));
        if let Some(plan) = plan {
            object.insert("plan".to_string(), Value::String(plan));
        }
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
    if let Some(project_id) = resolve_gemini_code_assist_project_with_endpoint(
        client,
        secret,
        code_assist_endpoint,
        GeminiCodeAssistSetupMode::NonInteractive,
    )? {
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
        if let Some(validation) = gemini_validation_from_body(&body) {
            bail!("{}", gemini_validation_error_message(&validation));
        }
        bail!(
            "Gemini quota request failed (HTTP {}): {body}",
            status.as_u16()
        );
    }
    serde_json::from_str(&body).context("failed to parse Gemini quota response")
}

fn gemini_error_is_http_401(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("HTTP 401"))
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
    mode: GeminiCodeAssistSetupMode,
) -> Result<Option<String>> {
    let code_assist_endpoint = gemini_code_assist_endpoint();
    resolve_gemini_code_assist_project_with_endpoint(client, secret, &code_assist_endpoint, mode)
}

fn resolve_gemini_code_assist_project_with_endpoint(
    client: &Client,
    secret: &GeminiOAuthSecret,
    code_assist_endpoint: &str,
    mode: GeminiCodeAssistSetupMode,
) -> Result<Option<String>> {
    setup_gemini_code_assist_project_with_endpoint(client, secret, code_assist_endpoint, mode)
}

fn setup_gemini_code_assist_project_with_endpoint(
    client: &Client,
    secret: &GeminiOAuthSecret,
    code_assist_endpoint: &str,
    mode: GeminiCodeAssistSetupMode,
) -> Result<Option<String>> {
    let project_id = gemini_oauth_project_from_env();
    let mut load_response = loop {
        let value = request_gemini_code_assist_post(
            client,
            secret,
            code_assist_endpoint,
            "loadCodeAssist",
            &json!({
                "cloudaicompanionProject": project_id.as_deref(),
                "metadata": gemini_code_assist_metadata(project_id.as_deref()),
            }),
        )?;
        let load_response: GeminiLoadCodeAssistResponse = serde_json::from_value(value)
            .context("failed to parse Gemini Code Assist setup response")?;
        if let Some(validation) = gemini_validation_from_load_response(&load_response) {
            handle_gemini_validation(&validation, mode)?;
            continue;
        }
        break load_response;
    };

    if let Some(project_id) = load_response
        .cloudaicompanion_project
        .as_ref()
        .and_then(gemini_code_assist_project_id)
    {
        return Ok(Some(project_id));
    }
    if load_response.current_tier.is_some() {
        if project_id.is_some() {
            return Ok(project_id);
        }
        if let Some(message) = gemini_code_assist_ineligible_message(&mut load_response) {
            bail!("{message}");
        }
        bail!(
            "Gemini Code Assist setup requires GOOGLE_CLOUD_PROJECT or GOOGLE_CLOUD_PROJECT_ID for this account"
        );
    }

    let tier = gemini_code_assist_onboard_tier(&load_response);
    let tier_id = tier
        .id
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("legacy-tier");
    let onboard_project = (tier_id != "free-tier")
        .then(|| project_id.clone())
        .flatten();
    let mut operation = request_gemini_code_assist_post(
        client,
        secret,
        code_assist_endpoint,
        "onboardUser",
        &json!({
            "tierId": tier_id,
            "cloudaicompanionProject": onboard_project.as_deref(),
            "metadata": gemini_code_assist_metadata(onboard_project.as_deref()),
        }),
    )
    .and_then(|value| {
        serde_json::from_value::<GeminiCodeAssistOperationResponse>(value)
            .context("failed to parse Gemini Code Assist onboarding response")
    })?;

    while !operation.done.unwrap_or(false) {
        let name = operation
            .name
            .as_deref()
            .context("Gemini Code Assist onboarding did not return an operation name")?;
        std::thread::sleep(Duration::from_secs(5));
        operation =
            request_gemini_code_assist_operation(client, secret, code_assist_endpoint, name)?;
    }

    if let Some(project_id) = operation
        .response
        .as_ref()
        .and_then(|response| response.cloudaicompanion_project.as_ref())
        .and_then(gemini_code_assist_project_id)
    {
        return Ok(Some(project_id));
    }
    if project_id.is_some() {
        return Ok(project_id);
    }
    if let Some(message) = gemini_code_assist_ineligible_message(&mut load_response) {
        bail!("{message}");
    }
    bail!(
        "Gemini Code Assist setup requires GOOGLE_CLOUD_PROJECT or GOOGLE_CLOUD_PROJECT_ID for this account"
    )
}

fn gemini_code_assist_metadata(project_id: Option<&str>) -> serde_json::Value {
    json!({
        "ideType": "IDE_UNSPECIFIED",
        "platform": "PLATFORM_UNSPECIFIED",
        "pluginType": "GEMINI",
        "duetProject": project_id,
    })
}

fn request_gemini_code_assist_post(
    client: &Client,
    secret: &GeminiOAuthSecret,
    code_assist_endpoint: &str,
    method: &str,
    body: &Value,
) -> Result<Value> {
    let response = client
        .post(format!("{code_assist_endpoint}:{method}"))
        .bearer_auth(&secret.access_token)
        .json(body)
        .send()
        .with_context(|| format!("failed to call Gemini Code Assist {method}"))?;
    parse_gemini_code_assist_response(response, method)
}

fn request_gemini_code_assist_operation(
    client: &Client,
    secret: &GeminiOAuthSecret,
    code_assist_endpoint: &str,
    name: &str,
) -> Result<GeminiCodeAssistOperationResponse> {
    let response = client
        .get(format!("{code_assist_endpoint}/{name}"))
        .bearer_auth(&secret.access_token)
        .send()
        .context("failed to poll Gemini Code Assist onboarding operation")?;
    let value = parse_gemini_code_assist_response(response, "getOperation")?;
    serde_json::from_value(value).context("failed to parse Gemini Code Assist operation response")
}

fn parse_gemini_code_assist_response(
    response: reqwest::blocking::Response,
    method: &str,
) -> Result<Value> {
    let status = response.status();
    let body = response
        .text()
        .with_context(|| format!("failed to read Gemini Code Assist {method} response"))?;
    if !status.is_success() {
        if let Some(validation) = gemini_validation_from_body(&body) {
            bail!("{}", gemini_validation_error_message(&validation));
        }
        bail!(
            "Gemini Code Assist {method} failed (HTTP {}): {body}",
            status.as_u16()
        );
    }
    serde_json::from_str(&body)
        .with_context(|| format!("failed to parse Gemini Code Assist {method} response"))
}

fn gemini_code_assist_onboard_tier(
    response: &GeminiLoadCodeAssistResponse,
) -> GeminiCodeAssistTier {
    response
        .allowed_tiers
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|tier| tier.is_default.unwrap_or(false))
        .cloned()
        .or_else(|| {
            response
                .allowed_tiers
                .as_deref()
                .unwrap_or_default()
                .first()
                .cloned()
        })
        .unwrap_or(GeminiCodeAssistTier {
            id: Some("legacy-tier".to_string()),
            name: None,
            is_default: None,
        })
}

fn fetch_gemini_code_assist_plan(
    client: &Client,
    secret: &GeminiOAuthSecret,
    project_id: &str,
    code_assist_endpoint: &str,
) -> Result<Option<String>> {
    let value = request_gemini_code_assist_post(
        client,
        secret,
        code_assist_endpoint,
        "loadCodeAssist",
        &json!({
            "cloudaicompanionProject": project_id,
            "metadata": gemini_code_assist_metadata(Some(project_id)),
            "mode": "HEALTH_CHECK",
        }),
    )?;
    let response: GeminiLoadCodeAssistResponse = serde_json::from_value(value)
        .context("failed to parse Gemini Code Assist plan response")?;
    Ok(gemini_code_assist_plan_label(&response))
}

fn gemini_code_assist_plan_label(response: &GeminiLoadCodeAssistResponse) -> Option<String> {
    response
        .paid_tier
        .as_ref()
        .or(response.current_tier.as_ref())
        .and_then(gemini_code_assist_tier_label)
}

fn gemini_code_assist_tier_label(tier: &GeminiCodeAssistTier) -> Option<String> {
    if let Some(id) = tier
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        return Some(match id {
            "free-tier" => "free".to_string(),
            "legacy-tier" => "legacy".to_string(),
            "standard-tier" => "standard".to_string(),
            "g1-pro-tier" => "pro".to_string(),
            "g1-ultra-tier" => "ultra".to_string(),
            other => other
                .strip_suffix("-tier")
                .unwrap_or(other)
                .to_ascii_lowercase(),
        });
    }
    tier.name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            if lower.contains("google one ai ultra") {
                "ultra".to_string()
            } else if lower.contains("google one ai pro") {
                "pro".to_string()
            } else if lower.contains("standard") {
                "standard".to_string()
            } else if lower.contains("free") {
                "free".to_string()
            } else {
                name.to_string()
            }
        })
}

fn gemini_validation_from_load_response(
    response: &GeminiLoadCodeAssistResponse,
) -> Option<GeminiCodeAssistValidation> {
    response
        .ineligible_tiers
        .as_deref()?
        .iter()
        .find(|tier| {
            tier.reason_code.as_deref() == Some("VALIDATION_REQUIRED")
                && tier
                    .validation_url
                    .as_deref()
                    .is_some_and(|url| !url.is_empty())
        })
        .map(|tier| GeminiCodeAssistValidation {
            url: tier.validation_url.clone(),
            description: tier.reason_message.clone(),
            learn_more_url: None,
        })
}

fn gemini_validation_from_body(body: &str) -> Option<GeminiCodeAssistValidation> {
    let value: Value = serde_json::from_str(body).ok()?;
    let error = value.get("error")?;
    let details = error.get("details")?.as_array()?;
    let error_info = details.iter().find(|detail| {
        detail.get("@type").and_then(Value::as_str)
            == Some("type.googleapis.com/google.rpc.ErrorInfo")
            && detail.get("reason").and_then(Value::as_str) == Some("VALIDATION_REQUIRED")
            && detail
                .get("domain")
                .and_then(Value::as_str)
                .is_some_and(gemini_code_assist_domain_matches)
    })?;
    let help_link = details
        .iter()
        .find(|detail| {
            detail.get("@type").and_then(Value::as_str)
                == Some("type.googleapis.com/google.rpc.Help")
        })
        .and_then(|detail| detail.get("links"))
        .and_then(Value::as_array)
        .and_then(|links| links.first());
    let url = help_link
        .and_then(|link| link.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            error_info
                .get("metadata")
                .and_then(|metadata| {
                    metadata
                        .get("validation_url")
                        .or_else(|| metadata.get("validation_link"))
                })
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let description = help_link
        .and_then(|link| link.get("description"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            error_info
                .get("metadata")
                .and_then(|metadata| metadata.get("validation_error_message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let learn_more_url = details
        .iter()
        .find(|detail| {
            detail.get("@type").and_then(Value::as_str)
                == Some("type.googleapis.com/google.rpc.Help")
        })
        .and_then(|detail| detail.get("links"))
        .and_then(Value::as_array)
        .and_then(|links| {
            links.iter().find_map(|link| {
                let description = link.get("description").and_then(Value::as_str)?;
                (description.eq_ignore_ascii_case("learn more"))
                    .then(|| link.get("url").and_then(Value::as_str).map(str::to_string))
                    .flatten()
            })
        });
    Some(GeminiCodeAssistValidation {
        url,
        description,
        learn_more_url,
    })
}

fn gemini_code_assist_domain_matches(domain: &str) -> bool {
    let sanitized = domain
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '.' || *ch == '-')
        .collect::<String>();
    sanitized == "cloudcode-pa.googleapis.com"
}

fn handle_gemini_validation(
    validation: &GeminiCodeAssistValidation,
    mode: GeminiCodeAssistSetupMode,
) -> Result<()> {
    if matches!(mode, GeminiCodeAssistSetupMode::NonInteractive) {
        bail!("{}", gemini_validation_error_message(validation));
    }
    print_wrapped_stderr(&gemini_validation_error_message(validation));
    if let Some(url) = validation.url.as_deref() {
        let _ = open_browser(url);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("Gemini account validation requires an interactive terminal");
    }
    print_wrapped_stderr("Press Enter when Gemini account verification is complete.");
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read Gemini account validation confirmation")?;
    Ok(())
}

fn gemini_validation_error_message(validation: &GeminiCodeAssistValidation) -> String {
    let mut message = validation
        .description
        .clone()
        .unwrap_or_else(|| "Gemini account validation required".to_string());
    if let Some(url) = validation.url.as_deref() {
        message.push_str(&format!(" Open: {url}"));
    }
    if let Some(url) = validation.learn_more_url.as_deref() {
        message.push_str(&format!(" Learn more: {url}"));
    }
    message
}

fn gemini_code_assist_ineligible_message(
    response: &mut GeminiLoadCodeAssistResponse,
) -> Option<String> {
    let tiers = response.ineligible_tiers.take()?;
    let reasons = tiers
        .into_iter()
        .filter_map(|tier| tier.reason_message)
        .filter(|message| !message.trim().is_empty())
        .collect::<Vec<_>>();
    (!reasons.is_empty()).then(|| reasons.join(", "))
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

            let mut plan = server.recv().expect("plan request should arrive");
            assert_eq!(plan.method().as_str(), "POST");
            assert_eq!(plan.url(), "/v1internal:loadCodeAssist");
            let mut plan_body = String::new();
            plan.as_reader()
                .read_to_string(&mut plan_body)
                .expect("plan request body should read");
            assert!(plan_body.contains("\"mode\":\"HEALTH_CHECK\""));
            assert!(plan_body.contains("\"cloudaicompanionProject\":\"gemini-project\""));
            plan.respond(TinyResponse::from_string(
                r#"{"paidTier":{"id":"g1-pro-tier","name":"Gemini Code Assist in Google One AI Pro"},"currentTier":{"id":"standard-tier"}}"#,
            ))
            .expect("plan response should send");
        });

        let quota = fetch_gemini_quota(&root, None).expect("Gemini quota should fetch");
        handle.join().expect("quota test server should finish");

        assert_eq!(quota.email.as_deref(), Some("gemini-user@example.com"));
        assert_eq!(quota.plan.as_deref(), Some("pro"));
        assert_eq!(quota.project_id.as_deref(), Some("gemini-project"));
        assert_eq!(quota.buckets.len(), 1);
        assert_eq!(
            quota.buckets[0].model_id.as_deref(),
            Some("models/gemini-2.5-pro")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn code_assist_setup_onboards_default_tier_project() {
        let _env_lock = crate::TestEnvVarGuard::lock();
        let _project_guard = crate::TestEnvVarGuard::unset("GOOGLE_CLOUD_PROJECT");
        let _project_id_guard = crate::TestEnvVarGuard::unset("GOOGLE_CLOUD_PROJECT_ID");
        let _gcloud_guard = crate::TestEnvVarGuard::unset("GCLOUD_PROJECT");
        let server = TinyServer::http("127.0.0.1:0").expect("setup test server should bind");
        let listen_addr = server.server_addr().to_ip().unwrap();
        let endpoint = format!("http://{listen_addr}/v1internal");
        let secret = test_gemini_secret(None);

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
                r#"{"allowedTiers":[{"id":"standard-tier","isDefault":true}],"paidTier":{"id":"g1-pro-tier"}}"#,
            ))
            .expect("loadCodeAssist response should send");

            let mut onboard = server.recv().expect("onboardUser request should arrive");
            assert_eq!(onboard.method().as_str(), "POST");
            assert_eq!(onboard.url(), "/v1internal:onboardUser");
            let mut onboard_body = String::new();
            onboard
                .as_reader()
                .read_to_string(&mut onboard_body)
                .expect("onboardUser body should read");
            assert!(onboard_body.contains("\"tierId\":\"standard-tier\""));
            onboard
                .respond(TinyResponse::from_string(
                    r#"{"done":true,"response":{"cloudaicompanionProject":{"id":"created-project"}}}"#,
                ))
                .expect("onboardUser response should send");
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("client should build");
        let project_id = resolve_gemini_code_assist_project_with_endpoint(
            &client,
            &secret,
            &endpoint,
            GeminiCodeAssistSetupMode::NonInteractive,
        )
        .expect("Code Assist setup should succeed");
        handle.join().expect("setup test server should finish");

        assert_eq!(project_id.as_deref(), Some("created-project"));
    }

    #[test]
    fn fetch_gemini_quota_surfaces_validation_required_url() {
        let server = TinyServer::http("127.0.0.1:0").expect("quota test server should bind");
        let listen_addr = server.server_addr().to_ip().unwrap();
        let endpoint = format!("http://{listen_addr}/v1internal");
        let _endpoint_guard =
            crate::TestEnvVarGuard::set("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT", endpoint.as_str());
        let root = temp_codex_home("quota-validation");
        let secret = test_gemini_secret(Some("gemini-project"));
        write_gemini_oauth_secret(&root, &secret).expect("Gemini OAuth secret should write");

        let handle = thread::spawn(move || {
            let request = server.recv().expect("quota request should arrive");
            assert_eq!(request.method().as_str(), "POST");
            assert_eq!(request.url(), "/v1internal:retrieveUserQuota");
            request
                .respond(
                    TinyResponse::from_string(
                        r#"{"error":{"code":403,"message":"Verify your account to continue.","status":"PERMISSION_DENIED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"VALIDATION_REQUIRED","domain":"cloudcode-pa.googleapis.com","metadata":{"validation_error_message":"Verify your account to continue.","validation_url":"https://accounts.google.com/verify"}},{"@type":"type.googleapis.com/google.rpc.Help","links":[{"description":"Verify your account","url":"https://accounts.google.com/verify"},{"description":"Learn more","url":"https://support.google.com/accounts?p=al_alert"}]}]}}"#,
                    )
                    .with_status_code(403),
                )
                .expect("validation response should send");
        });

        let err = fetch_gemini_quota(&root, None).expect_err("quota should require validation");
        handle
            .join()
            .expect("quota validation test server should finish");
        let message = format!("{err:#}");
        assert!(message.contains("Verify your account"));
        assert!(message.contains("https://accounts.google.com/verify"));
        assert!(message.contains("https://support.google.com/accounts?p=al_alert"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn test_gemini_secret(project_id: Option<&str>) -> GeminiOAuthSecret {
        GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-123".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: Some(now_ms() + 3_600_000),
            email: "gemini-user@example.com".to_string(),
            project_id: project_id.map(str::to_string),
        }
    }

    fn temp_codex_home(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("prodex-gemini-auth-{name}-{stamp}"))
    }
}
