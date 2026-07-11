use super::{
    GeminiOAuthSecret, gemini_oauth_project_from_env, normalize_gemini_project_id,
    refresh_gemini_oauth_secret_if_needed, write_gemini_oauth_secret,
};
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_text_with_limit};
use anyhow::{Context, Result, bail};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use redaction::redaction_redact_secret_like_text;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use std::io::{self, IsTerminal};
use std::path::Path;
use std::time::Duration;
use terminal_ui::{
    tui_border_style, tui_connected_footer_block, tui_connected_header_block, tui_hint_style,
    tui_primary_style, tui_secondary_style, tui_success_style, tui_title_style,
};

const GEMINI_CODE_ASSIST_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com/v1internal";

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
pub(super) enum GeminiCodeAssistSetupMode {
    Interactive,
    NonInteractive,
}

#[derive(Debug, Clone)]
pub(super) struct GeminiCodeAssistValidation {
    url: Option<String>,
    description: Option<String>,
    learn_more_url: Option<String>,
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

pub(crate) fn gemini_code_assist_endpoint() -> String {
    env::var("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT")
        .or_else(|_| env::var("GEMINI_CODE_ASSIST_ENDPOINT"))
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| GEMINI_CODE_ASSIST_ENDPOINT.to_string())
}

pub(super) fn resolve_gemini_code_assist_project(
    client: &Client,
    secret: &GeminiOAuthSecret,
    mode: GeminiCodeAssistSetupMode,
) -> Result<Option<String>> {
    let code_assist_endpoint = gemini_code_assist_endpoint();
    resolve_gemini_code_assist_project_with_endpoint(client, secret, &code_assist_endpoint, mode)
}

pub(super) fn resolve_gemini_code_assist_project_with_endpoint(
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
    let body = read_blocking_response_text_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        &format!("failed to read Gemini Code Assist {method} response"),
    )?;
    if !status.is_success() {
        if let Some(validation) = gemini_validation_from_body(&body) {
            bail!("{}", gemini_validation_error_message(&validation));
        }
        let body = gemini_code_assist_redacted_error_body(&body);
        bail!(
            "Gemini Code Assist {method} failed (HTTP {}): {body}",
            status.as_u16()
        );
    }
    serde_json::from_str(&body)
        .with_context(|| format!("failed to parse Gemini Code Assist {method} response"))
}

fn gemini_code_assist_redacted_error_body(body: &str) -> String {
    redaction_redact_secret_like_text(body)
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

pub(super) fn fetch_gemini_code_assist_plan(
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

pub(super) fn gemini_validation_from_body(body: &str) -> Option<GeminiCodeAssistValidation> {
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

pub(super) fn handle_gemini_validation(
    validation: &GeminiCodeAssistValidation,
    mode: GeminiCodeAssistSetupMode,
) -> Result<()> {
    if matches!(mode, GeminiCodeAssistSetupMode::NonInteractive) {
        bail!("{}", gemini_validation_error_message(validation));
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("Gemini account validation requires an interactive terminal");
    }
    if let Some(url) = validation.url.as_deref() {
        let _ = super::oauth::open_browser(url);
    }
    prompt_gemini_validation_tui(validation)?;
    Ok(())
}

struct GeminiValidationTui {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
}

impl GeminiValidationTui {
    fn new() -> Result<Self> {
        enable_raw_mode().context("failed to enable Gemini validation TUI raw mode")?;
        let mut stderr = io::stderr();
        if let Err(err) = crossterm::execute!(stderr, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err).context("failed to enter Gemini validation TUI alternate screen");
        }
        let backend = CrosstermBackend::new(stderr);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let mut stderr = io::stderr();
                let _ = crossterm::execute!(stderr, Show, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err).context("failed to initialize Gemini validation TUI terminal");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for GeminiValidationTui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn prompt_gemini_validation_tui(validation: &GeminiCodeAssistValidation) -> Result<()> {
    let message = gemini_validation_error_message(validation);
    let mut tui = GeminiValidationTui::new()?;
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Gemini Account Validation", tui_title_style()),
                Span::raw("  "),
                Span::styled("action required", tui_hint_style()),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);

            let mut lines = vec![
                Line::from(Span::styled(message.clone(), tui_primary_style())),
                Line::raw(""),
            ];
            if let Some(url) = validation.url.as_deref() {
                lines.push(Line::from(vec![
                    Span::styled("Open ", tui_secondary_style()),
                    Span::styled(url.to_string(), tui_success_style()),
                ]));
            }
            if let Some(url) = validation.learn_more_url.as_deref() {
                lines.push(Line::from(vec![
                    Span::styled("Learn ", tui_secondary_style()),
                    Span::styled(url.to_string(), tui_success_style()),
                ]));
            }
            let body = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::LEFT | Borders::RIGHT)
                        .border_style(tui_border_style()),
                )
                .wrap(Wrap { trim: false });
            frame.render_widget(body, chunks[1]);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("enter", tui_success_style()),
                Span::raw(" continue after verification  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(" continue"),
            ]))
            .block(tui_connected_footer_block(tui_border_style()));
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && (matches!(key.code, KeyCode::Enter | KeyCode::Esc)
                || (key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('z'))))
        {
            return Ok(());
        }
    }
}

pub(super) fn gemini_validation_error_message(validation: &GeminiCodeAssistValidation) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tiny_http::{Response as TinyResponse, Server as TinyServer};

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
    fn code_assist_error_body_redacts_secret_like_material() {
        let server = TinyServer::http("127.0.0.1:0").expect("setup test server should bind");
        let listen_addr = server.server_addr().to_ip().unwrap();
        let endpoint = format!("http://{listen_addr}/v1internal");
        let secret = test_gemini_secret(None);

        let handle = thread::spawn(move || {
            let request = server.recv().expect("Code Assist request should arrive");
            assert_eq!(request.method().as_str(), "POST");
            assert_eq!(request.url(), "/v1internal:loadCodeAssist");
            request
                .respond(
                    TinyResponse::from_string(
                        r#"{"error":"Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123"}"#,
                    )
                    .with_status_code(500),
                )
                .expect("error response should send");
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("client should build");
        let err = request_gemini_code_assist_post(
            &client,
            &secret,
            &endpoint,
            "loadCodeAssist",
            &json!({}),
        )
        .expect_err("Code Assist request should fail");
        handle.join().expect("setup test server should finish");
        let message = format!("{err:#}");

        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("fixture-token-123"));
        assert!(!message.contains("sk-fixture-123"));
    }

    fn test_gemini_secret(project_id: Option<&str>) -> GeminiOAuthSecret {
        GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-123".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: Some(super::super::now_ms() + 3_600_000),
            email: "gemini-user@example.com".to_string(),
            project_id: project_id.map(str::to_string),
        }
    }
}
