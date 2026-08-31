use super::super::super_expose::ExposeEndpointMode;
use super::support::{labeled_value_lines, text_lines};
use super::{ExposeTuiPhase, ExposeTuiState};
use ratatui::text::Line;
use terminal_ui::{tui_error_style, tui_muted_style, tui_primary_style, tui_success_style};

pub(crate) fn ready_body(state: &ExposeTuiState, width: usize) -> Vec<Line<'static>> {
    let Some(ready) = state.ready.as_ref() else {
        return vec![Line::styled("Ready", tui_success_style())];
    };
    let active_runs = ready
        .mcp
        .run_manager
        .list()
        .into_iter()
        .filter(|run| !run.state.terminal())
        .count();
    let endpoint = match &ready.endpoint {
        ExposeEndpointMode::LocalOnly => "Local only",
        ExposeEndpointMode::QuickTunnel => "Quick Tunnel",
        ExposeEndpointMode::ExistingCloudflareTunnel { .. } => "Existing Cloudflare Tunnel",
        ExposeEndpointMode::OpenAiSecureMcp { .. } => "OpenAI Secure MCP Tunnel",
    };
    let mut lines = vec![
        Line::styled(
            if ready.public_url.is_some() {
                "Ready — public MCP and browser route passed readiness"
            } else {
                "Ready — local MCP and browser route passed readiness"
            },
            tui_success_style().add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Line::from(format!(
            "Instance: {} ({})",
            ready.display_name, ready.instance_id
        )),
        Line::from(format!("Workspace: {}", ready.workspace_name)),
        Line::from(format!("Provider: {}", state.provider)),
        Line::from(format!("Model: {}", state.model)),
        Line::from(format!("Effort: {}", state.effort)),
        Line::from(format!(
            "Sub-agents: {}{}",
            if state.sub_agent {
                "enabled"
            } else {
                "disabled"
            },
            if state.sub_agent {
                format!(" · {}/{}", state.sub_agent_model, state.sub_agent_effort)
            } else {
                String::new()
            },
        )),
        Line::from(format!("Endpoint: {endpoint}")),
        Line::from(format!("Active runs: {active_runs}")),
        Line::from(""),
    ];
    if let Some(public_url) = ready.public_url.as_ref() {
        lines.extend(labeled_value_lines(
            "Public MCP URL",
            public_url.as_str(),
            width,
            tui_primary_style(),
            tui_primary_style(),
        ));
    }
    lines.extend(labeled_value_lines(
        if ready.public_url.is_some() {
            "MCP URL"
        } else {
            "Local MCP URL"
        },
        ready.local_mcp_url.as_str(),
        width,
        tui_primary_style(),
        tui_primary_style(),
    ));
    if let Some(public_browser_url) = ready.public_browser_url.as_deref() {
        lines.extend(labeled_value_lines(
            "Public Browser URL",
            public_browser_url,
            width,
            tui_muted_style(),
            tui_muted_style(),
        ));
    }
    lines.extend(labeled_value_lines(
        if ready.public_url.is_some() {
            "Browser URL"
        } else {
            "Local Browser URL"
        },
        &ready.local_url,
        width,
        tui_muted_style(),
        tui_muted_style(),
    ));
    if let ExposeEndpointMode::ExistingCloudflareTunnel {
        hostname, tunnel, ..
    } = &ready.endpoint
    {
        if let Some(tunnel) = tunnel {
            lines.extend(labeled_value_lines(
                "Tunnel",
                tunnel,
                width,
                tui_primary_style(),
                tui_primary_style(),
            ));
        }
        lines.extend(labeled_value_lines(
            "Hostname",
            hostname,
            width,
            tui_muted_style(),
            tui_muted_style(),
        ));
    }
    if let ExposeEndpointMode::OpenAiSecureMcp {
        tunnel_id,
        client_version,
    } = &ready.endpoint
    {
        lines.extend(labeled_value_lines(
            "OpenAI tunnel",
            tunnel_id,
            width,
            tui_primary_style(),
            tui_primary_style(),
        ));
        lines.push(Line::from(format!(
            "OpenAI client: {client_version} · ready"
        )));
    }
    lines.push(Line::from(""));
    lines.extend(text_lines(
        "Access: full Super authority as the current OS user; initial directory is context only.",
        width,
        tui_error_style(),
    ));
    lines.extend(text_lines(
        "The MCP URL is an ephemeral bearer capability; stop expose to revoke it.",
        width,
        tui_error_style(),
    ));
    if let Some(status) = state.status.as_deref() {
        lines.extend(text_lines(status, width, tui_success_style()));
    }
    lines
}

pub(crate) fn status_body(state: &ExposeTuiState) -> Vec<Line<'static>> {
    let style = if state.phase == ExposeTuiPhase::Failed {
        tui_error_style()
    } else {
        tui_muted_style()
    };
    vec![Line::styled(
        state.status.clone().unwrap_or_else(|| "done".to_string()),
        style,
    )]
}
