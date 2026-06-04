use super::{GeminiOAuthSecret, gemini_oauth_expiry_date_ms};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::env;
use std::process::{Command, Stdio};
use tiny_http::{Response as TinyResponse, Server as TinyServer};

const GEMINI_OAUTH_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GEMINI_OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GEMINI_OAUTH_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const GEMINI_OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
];

#[derive(Debug, Deserialize)]
pub(super) struct GeminiTokenResponse {
    pub(super) access_token: String,
    #[serde(default)]
    pub(super) refresh_token: Option<String>,
    #[serde(default)]
    pub(super) token_type: Option<String>,
    #[serde(default)]
    pub(super) scope: Option<String>,
    #[serde(default)]
    pub(super) expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GeminiUserInfoResponse {
    email: String,
}

pub(super) fn gemini_oauth_authorize_url(redirect_uri: &str, state: &str) -> Result<String> {
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

pub(super) fn wait_for_google_oauth_code(
    server: &TinyServer,
    expected_state: &str,
) -> Result<String> {
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

pub(super) fn exchange_google_oauth_code(
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

pub(super) fn refresh_google_oauth_token(
    client: &Client,
    refresh_token: &str,
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

pub(super) fn fetch_google_user_email(client: &Client, access_token: &str) -> Result<String> {
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

pub(super) fn gemini_oauth_secret_from_token(
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

pub(super) fn random_hex(byte_count: usize) -> Result<String> {
    let mut bytes = vec![0u8; byte_count];
    getrandom::fill(&mut bytes).context("failed to generate Google OAuth state")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(super) fn open_browser(url: &str) -> Result<()> {
    if let Some(browser) = env::var_os("BROWSER").filter(|value| !value.is_empty()) {
        let mut command = Command::new(browser);
        command.arg(url);
        spawn_quiet_browser(command).context("failed to open browser from BROWSER")?;
        return Ok(());
    }
    if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg(url);
        spawn_quiet_browser(command).context("failed to open browser with open")?;
        return Ok(());
    }
    if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        spawn_quiet_browser(command).context("failed to open browser with start")?;
        return Ok(());
    }
    if cfg!(all(unix, not(target_os = "macos"))) {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        spawn_quiet_browser(command).context("failed to open browser with xdg-open")?;
        return Ok(());
    }
    Ok(())
}

fn spawn_quiet_browser(mut command: Command) -> Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_oauth_authorize_url_includes_scopes_and_state() {
        let url = gemini_oauth_authorize_url("http://127.0.0.1:1234/oauth2callback", "state-test")
            .expect("authorize URL should build");

        assert!(url.contains("client_id="));
        assert!(url.contains("state=state-test"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("cloud-platform"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn open_browser_suppresses_browser_stdio() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::thread;
        use std::time::Duration;

        let _env_lock = crate::TestEnvVarGuard::lock();
        let root = env::temp_dir().join(format!(
            "prodex-browser-stdio-{}-{}",
            std::process::id(),
            random_hex(4).expect("random suffix")
        ));
        fs::create_dir_all(&root).expect("test dir should be created");
        let script = root.join("browser");
        let report = root.join("stdio-report");
        fs::write(
            &script,
            "#!/bin/sh\npid=$$\nout=$(readlink \"/proc/$pid/fd/1\")\nerr=$(readlink \"/proc/$pid/fd/2\")\nprintf '%s\\n%s\\n' \"$out\" \"$err\" > \"$PRODEX_BROWSER_STDIO_REPORT\"\n",
        )
        .expect("browser script should be written");
        let mut permissions = fs::metadata(&script)
            .expect("browser script metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("browser script should be executable");

        let _browser_guard =
            crate::TestEnvVarGuard::set("BROWSER", script.to_str().expect("script path utf-8"));
        let _report_guard = crate::TestEnvVarGuard::set(
            "PRODEX_BROWSER_STDIO_REPORT",
            report.to_str().expect("report path utf-8"),
        );

        open_browser("https://example.test").expect("fake browser should spawn");
        for _ in 0..50 {
            if report.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        let report = fs::read_to_string(&report).expect("stdio report should be written");
        assert_eq!(report, "/dev/null\n/dev/null\n");
        let _ = fs::remove_dir_all(root);
    }
}
