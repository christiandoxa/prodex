use anyhow::{Context, Result, anyhow};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, IsTerminal};
use std::net::SocketAddr;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::dashboard_html::DASHBOARD_HTML;
use crate::{
    AppPaths, AppState, AppStateIoExt, DashboardArgs, ProfileEntry, ProfileProvider,
    ProviderQuotaSnapshot, QuotaProviderFilter, collect_profile_summaries,
    collect_quota_reports_with_filters, create_codex_home_if_missing, ensure_path_is_unique,
    format_copilot_main_quota, format_copilot_quota_status, format_copilot_reset_summary,
    format_gemini_main_quota, format_gemini_quota_status, format_gemini_reset_summary,
    format_main_windows, managed_profile_home_path, prepare_managed_codex_home,
};

#[derive(Debug)]
struct DashboardServer {
    paths: AppPaths,
    base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActiveProfileRequest {
    profile: String,
}

#[derive(Debug, Deserialize)]
struct AddProfileRequest {
    name: String,
    #[serde(default)]
    activate: bool,
}

pub(crate) fn serve_dashboard(paths: AppPaths, args: DashboardArgs) -> Result<()> {
    let bind = format!("{}:{}", args.host.trim(), args.port);
    let server = Server::http(&bind).map_err(|err| anyhow!("failed to bind {bind}: {err}"))?;
    let addr = server.server_addr();
    let url = dashboard_url(addr);
    let warning = (!bind.starts_with("127.0.0.1:")
        && !bind.starts_with("localhost:")
        && !bind.starts_with("[::1]:"))
    .then_some("dashboard has no password auth; bind localhost unless the network is trusted");
    print_dashboard_status(&url, warning)?;

    let dashboard = DashboardServer {
        paths,
        base_url: args.base_url,
    };
    for request in server.incoming_requests() {
        if let Err(err) = dashboard.handle(request) {
            eprintln!("dashboard request failed: {err:#}");
        }
    }
    Ok(())
}

fn print_dashboard_status(url: &str, warning: Option<&str>) -> Result<()> {
    if !io::stdout().is_terminal() {
        println!("Prodex dashboard: {url}");
        if let Some(warning) = warning {
            println!("Warning: {warning}.");
        }
        return Ok(());
    }

    let height = if warning.is_some() { 8 } else { 7 };
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        println!("Prodex dashboard: {url}");
        if let Some(warning) = warning {
            println!("Warning: {warning}.");
        }
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "Prodex Dashboard",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("running", Style::default().fg(Color::Green)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(dashboard_status_tui_text(url, warning))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn dashboard_status_tui_text(url: &str, warning: Option<&str>) -> Text<'static> {
    let mut lines = vec![Line::from(vec![
        Span::styled("URL     ", Style::default().fg(Color::DarkGray)),
        Span::styled(url.to_string(), Style::default().fg(Color::Green)),
    ])];
    if let Some(warning) = warning {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Warning ", Style::default().fg(Color::Yellow)),
            Span::styled(warning.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    Text::from(lines)
}

fn dashboard_url(addr: tiny_http::ListenAddr) -> String {
    match addr.to_ip() {
        Some(SocketAddr::V4(addr)) => format!("http://{}:{}", addr.ip(), addr.port()),
        Some(SocketAddr::V6(addr)) => format!("http://[{}]:{}", addr.ip(), addr.port()),
        None => "http://127.0.0.1:8765".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_status_tui_text_contains_url_and_warning() {
        let text = dashboard_status_tui_text(
            "http://127.0.0.1:8765",
            Some("dashboard has no password auth"),
        );
        let rendered = text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("http://127.0.0.1:8765"));
        assert!(rendered.contains("Warning"));
        assert!(rendered.contains("dashboard has no password auth"));
    }
}

impl DashboardServer {
    fn handle(&self, request: Request) -> Result<()> {
        let method = request.method().clone();
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        match (method, path.as_str()) {
            (Method::Get, "/") | (Method::Get, "/dashboard") => {
                respond_html(request, DASHBOARD_HTML)
            }
            (Method::Get, "/api/state") => respond_json_result(request, self.state_json()),
            (Method::Get, "/api/accounts") => respond_json_result(request, self.accounts_json()),
            (Method::Get, "/api/usage") => respond_json_result(request, self.usage_json()),
            (Method::Post, "/api/profile") => self.handle_add_profile(request),
            (Method::Post, "/api/profile/active") => self.handle_set_active(request),
            (Method::Delete, path) if path.starts_with("/api/profile/") => {
                let name = path.trim_start_matches("/api/profile/").to_string();
                self.handle_remove_profile(request, name)
            }
            _ => respond_status(
                request,
                StatusCode(404),
                "application/json",
                br#"{"error":"not_found"}"#.to_vec(),
            ),
        }
    }

    fn state_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        Ok(json!({
            "activeProfile": state.active_profile,
            "profileCount": state.profiles.len(),
            "paths": {
                "stateFile": self.paths.state_file.display().to_string(),
                "managedProfilesRoot": self.paths.managed_profiles_root.display().to_string(),
            },
            "commands": {
                "open": "prodex dashboard",
                "login": "prodex login --profile <name>",
                "addManagedProfile": "prodex profile add <name> --activate",
                "importCurrent": "prodex profile import-current <name>",
                "quota": "prodex quota --all --once",
            }
        }))
    }

    fn accounts_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        let accounts = collect_profile_summaries(&state)
            .into_iter()
            .map(|summary| {
                json!({
                    "name": summary.name,
                    "active": summary.active,
                    "managed": summary.managed,
                    "email": summary.email,
                    "provider": summary.provider.label(),
                    "providerName": summary.provider.display_name(),
                    "auth": {
                        "label": summary.auth.label,
                        "quotaCompatible": summary.auth.quota_compatible,
                    },
                    "codexHome": summary.codex_home.display().to_string(),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "accounts": accounts }))
    }

    fn usage_json(&self) -> Result<Value> {
        let state = AppState::load(&self.paths)?;
        let reports = collect_quota_reports_with_filters(
            &state,
            self.base_url.as_deref(),
            &prodex_quota::QuotaAuthFilter::All,
            QuotaProviderFilter::All,
        );
        let mut ready = 0usize;
        let mut blocked = 0usize;
        let mut errors = 0usize;
        let mut profiles = Vec::new();

        for report in reports {
            let quota = match report.result {
                Ok(snapshot) => {
                    let summary = quota_summary(&snapshot);
                    if summary["status"]
                        .as_str()
                        .is_some_and(|status| status == "Ready")
                    {
                        ready += 1;
                    } else {
                        blocked += 1;
                    }
                    summary
                }
                Err(error) => {
                    errors += 1;
                    json!({
                        "status": "Error",
                        "main": "-",
                        "reset": null,
                        "error": error.lines().find(|line| !line.trim().is_empty()).unwrap_or("quota fetch failed"),
                    })
                }
            };
            profiles.push(json!({
                "name": report.name,
                "active": report.active,
                "provider": report.provider.label(),
                "providerName": report.provider.display_name(),
                "auth": report.auth.label,
                "workspaceId": report.workspace_id,
                "fetchedAt": report.fetched_at,
                "quota": quota,
            }));
        }

        Ok(json!({
            "summary": {
                "ready": ready,
                "blocked": blocked,
                "errors": errors,
                "total": profiles.len(),
            },
            "profiles": profiles,
        }))
    }

    fn handle_set_active(&self, mut request: Request) -> Result<()> {
        let payload: ActiveProfileRequest = match read_json_body(&mut request) {
            Ok(payload) => payload,
            Err(err) => return respond_error(request, StatusCode(400), err),
        };
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if !state.profiles.contains_key(&payload.profile) {
            return respond_error(
                request,
                StatusCode(404),
                anyhow!("profile '{}' is missing", payload.profile),
            );
        }
        state.active_profile = Some(payload.profile);
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        respond_json(
            request,
            json!({ "status": "ok", "activeProfile": state.active_profile }),
        )
    }

    fn handle_add_profile(&self, mut request: Request) -> Result<()> {
        let payload: AddProfileRequest = match read_json_body(&mut request) {
            Ok(payload) => payload,
            Err(err) => return respond_error(request, StatusCode(400), err),
        };
        let name = payload.name.trim().to_string();
        if let Err(err) = prodex_profile_identity::validate_profile_name(&name) {
            return respond_error(request, StatusCode(400), err);
        }
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if state.profiles.contains_key(&name) {
            return respond_error(
                request,
                StatusCode(409),
                anyhow!("profile '{}' already exists", name),
            );
        }
        let codex_home = match managed_profile_home_path(&self.paths, &name) {
            Ok(path) => path,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        for result in [
            create_codex_home_if_missing(&codex_home),
            prepare_managed_codex_home(&self.paths, &codex_home),
            ensure_path_is_unique(&state, &codex_home),
        ] {
            if let Err(err) = result {
                return respond_error(request, StatusCode(500), err);
            }
        }
        state.profiles.insert(
            name.clone(),
            ProfileEntry {
                codex_home: codex_home.clone(),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        );
        if payload.activate || state.active_profile.is_none() {
            state.active_profile = Some(name.clone());
        }
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        respond_json(
            request,
            json!({
                "status": "ok",
                "profile": name,
                "activeProfile": state.active_profile,
                "codexHome": codex_home.display().to_string(),
            }),
        )
    }

    fn handle_remove_profile(&self, request: Request, raw_name: String) -> Result<()> {
        let name = percent_decode(&raw_name);
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if state.profiles.remove(&name).is_none() {
            return respond_status(
                request,
                StatusCode(404),
                "application/json",
                br#"{"error":"profile_not_found"}"#.to_vec(),
            );
        }
        if state.active_profile.as_deref() == Some(name.as_str()) {
            state.active_profile = state.profiles.keys().next().cloned();
        }
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        respond_json(
            request,
            json!({ "status": "ok", "activeProfile": state.active_profile }),
        )
    }
}

fn quota_summary(snapshot: &ProviderQuotaSnapshot) -> Value {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            let blocked = crate::collect_blocked_limits(usage, false);
            json!({
                "account": usage.email,
                "plan": usage.plan_type,
                "status": if blocked.is_empty() { "Ready".to_string() } else { format!("Blocked ({})", crate::format_blocked_limits(&blocked)) },
                "main": format_main_windows(usage),
                "reset": prodex_quota::format_main_reset_summary(usage),
                "windows": {
                    "fiveHour": usage.rate_limit.as_ref().and_then(|rate| rate.primary_window.as_ref()).map(window_json),
                    "weekly": usage.rate_limit.as_ref().and_then(|rate| rate.secondary_window.as_ref()).map(window_json),
                }
            })
        }
        ProviderQuotaSnapshot::Copilot(info) => json!({
            "account": info.login,
            "plan": info.copilot_plan.as_ref().or(info.access_type_sku.as_ref()),
            "status": format_copilot_quota_status(info),
            "main": format_copilot_main_quota(info),
            "reset": format_copilot_reset_summary(info),
        }),
        ProviderQuotaSnapshot::Gemini(info) => json!({
            "account": info.email,
            "plan": info.plan,
            "project": info.project_id,
            "status": format_gemini_quota_status(info),
            "main": format_gemini_main_quota(info),
            "reset": format_gemini_reset_summary(info),
        }),
        ProviderQuotaSnapshot::External(info) => json!({
            "account": info.account,
            "plan": info.plan,
            "status": info.status,
            "main": info.main,
            "reset": info.reset,
            "details": info.details,
        }),
    }
}

fn window_json(window: &prodex_quota::UsageWindow) -> Value {
    json!({
        "usedPercent": window.used_percent,
        "remainingPercent": prodex_quota::remaining_percent(window.used_percent),
        "resetAt": window.reset_at,
        "windowSeconds": window.limit_window_seconds,
    })
}

fn read_json_body<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T> {
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .context("failed to read dashboard request body")?;
    serde_json::from_str(&body).context("invalid JSON request body")
}

fn respond_json(request: Request, value: Value) -> Result<()> {
    let body = serde_json::to_vec(&value).context("failed to serialize dashboard JSON")?;
    respond_status(request, StatusCode(200), "application/json", body)
}

fn respond_json_result(request: Request, result: Result<Value>) -> Result<()> {
    match result {
        Ok(value) => respond_json(request, value),
        Err(err) => respond_error(request, StatusCode(500), err),
    }
}

fn respond_error(request: Request, status: StatusCode, err: anyhow::Error) -> Result<()> {
    respond_status(
        request,
        status,
        "application/json",
        serde_json::to_vec(&json!({ "error": err.to_string() }))
            .context("failed to serialize dashboard error")?,
    )
}

fn respond_html(request: Request, html: &str) -> Result<()> {
    respond_status(
        request,
        StatusCode(200),
        "text/html; charset=utf-8",
        html.as_bytes().to_vec(),
    )
}

fn respond_status(
    request: Request,
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> Result<()> {
    let response = Response::from_data(body)
        .with_status_code(status)
        .with_header(Header::from_bytes("content-type", content_type).unwrap())
        .with_header(Header::from_bytes("cache-control", "no-store").unwrap());
    request
        .respond(response)
        .map_err(|err| anyhow!("failed to send dashboard response: {err}"))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push((high << 4) | low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
