use super::{EndpointChoice, EndpointField, ExposeTuiState};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use terminal_ui::{tui_error_style, tui_muted_style, tui_primary_style, tui_success_style};

pub(super) fn endpoint_body(state: &ExposeTuiState) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled(
            format!("Workspace: {}", state.workspace_name),
            tui_primary_style(),
        ),
        Line::from(format!("Instance name: {}", state.display_name)),
        Line::from(format!(
            "Super: {} · {} · {} · sub-agents {}",
            state.provider,
            state.model,
            state.effort,
            if state.sub_agent { "on" } else { "off" },
        )),
        Line::from(""),
        option_line(
            state.endpoint_choice == EndpointChoice::LocalOnly,
            "Local only",
            "loopback only; no external tunnel",
        ),
        option_line(
            state.endpoint_choice == EndpointChoice::QuickTunnel,
            "Cloudflare Quick Tunnel",
            "random trycloudflare.com hostname",
        ),
        option_line(
            state.endpoint_choice == EndpointChoice::ExistingCloudflareTunnel,
            "Existing Cloudflare Tunnel",
            "use a configured hostname and loopback origin",
        ),
        option_line(
            state.endpoint_choice == EndpointChoice::OpenAiSecureMcp,
            "OpenAI Secure MCP Tunnel",
            "remote MCP; browser stays local",
        ),
    ];
    if state.endpoint_choice == EndpointChoice::ExistingCloudflareTunnel {
        lines.extend([
            Line::from(format!(
                "Tunnel: {}",
                state
                    .existing_cloudflare
                    .as_ref()
                    .and_then(|selection| selection.tunnel.as_deref())
                    .unwrap_or("<detected config identity unavailable>"),
            )),
            Line::from(format!(
                "Hostname{}: {}",
                if state.endpoint_field == EndpointField::Hostname {
                    "*"
                } else {
                    ""
                },
                if state.hostname.is_empty() {
                    "<type a public DNS name>"
                } else {
                    &state.hostname
                },
            )),
            Line::from(format!(
                "Origin port{}: {}",
                if state.endpoint_field == EndpointField::OriginPort {
                    "*"
                } else {
                    ""
                },
                state.origin_port,
            )),
        ]);
    }
    if state.endpoint_choice == EndpointChoice::OpenAiSecureMcp {
        lines.extend([
            Line::from(format!(
                "Tunnel ID: {}",
                state
                    .openai_tunnel_id
                    .as_deref()
                    .unwrap_or("<CONTROL_PLANE_TUNNEL_ID not set>")
            )),
            Line::from("Browser: local only"),
            Line::from("MCP: OpenAI Secure MCP Tunnel"),
        ]);
    }
    if let Some(status) = state.status.as_deref() {
        lines.push(Line::styled(status.to_string(), tui_error_style()));
    }
    lines
}

fn option_line(selected: bool, name: &str, description: &str) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let style = if selected {
        tui_success_style().add_modifier(Modifier::BOLD)
    } else {
        tui_muted_style()
    };
    Line::from(vec![
        Span::styled(format!("{marker} {name}"), style),
        Span::raw(format!(" — {description}")),
    ])
}
