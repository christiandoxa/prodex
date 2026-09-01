use super::super::mcp::{PublicMcpEndpoint, expose_main_provider};
use super::{ExposeEndpointMode, ExposeReadyState};
use crate::ExposeArgs;
use crate::print_stdout_line;
use prodex_cli::SuperArgs;
use terminal_ui::print_panel;

pub(crate) fn display_local_mcp_url<'a>(
    endpoint: &ExposeEndpointMode,
    local_mcp_url: &'a PublicMcpEndpoint,
) -> &'a str {
    match endpoint {
        ExposeEndpointMode::OpenAiSecureMcp { .. } => "local loopback only",
        _ => local_mcp_url.as_str(),
    }
}

pub(crate) fn print_super_expose_status(ready: &ExposeReadyState) -> anyhow::Result<()> {
    let local_url = &ready.local_url;
    let local_mcp_url = &ready.local_mcp_url;
    let public_url = ready.public_url.as_ref();
    let instance_id = &ready.instance_id;
    let workspace_name = &ready.workspace_name;
    let display_name = &ready.display_name;
    let mcp = &ready.mcp;
    let endpoint = &ready.endpoint;
    let local_mcp_status = display_local_mcp_url(endpoint, local_mcp_url);
    let args = &mcp.defaults;
    let model = args
        .local_model
        .clone()
        .or_else(|| crate::codex_cli_config_override_value(&args.codex_args, "model"))
        .unwrap_or_else(|| "remembered/default".to_string());
    let effort = crate::codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort")
        .unwrap_or_else(|| "remembered/default".to_string());
    let sub_agent = args.sub_agent;
    let mut fields = vec![
        (
            "WARNING".to_string(),
            "FULL ACCESS: this URL controls Prodex Super with OS-user authority".to_string(),
        ),
        (
            "Instance".to_string(),
            format!("{display_name} ({instance_id})"),
        ),
        ("Workspace".to_string(), workspace_name.to_string()),
        ("Main agent".to_string(), "Super".to_string()),
        (
            "Main provider".to_string(),
            expose_main_provider(args).label().to_string(),
        ),
        ("Model".to_string(), model),
        ("Effort".to_string(), effort),
        (
            "Sub-agents".to_string(),
            if sub_agent { "enabled" } else { "disabled" }.to_string(),
        ),
        ("Local browser URL".to_string(), local_url.to_string()),
        (
            "Tunnel provider".to_string(),
            endpoint_provider(endpoint).to_string(),
        ),
        ("Local MCP URL".to_string(), local_mcp_status.to_string()),
        (
            "Access".to_string(),
            "Ephemeral Capability Authentication".to_string(),
        ),
        (
            "Suggested ChatGPT name".to_string(),
            format!("Prodex — {display_name}"),
        ),
        (
            "Lifetime".to_string(),
            "active only while this process is running".to_string(),
        ),
        (
            "Stop".to_string(),
            "Press Ctrl+C to revoke access and stop".to_string(),
        ),
    ];
    if sub_agent {
        fields.push((
            "Sub-agent model/effort".to_string(),
            format!(
                "{}/{}",
                args.sub_agent_model
                    .as_deref()
                    .unwrap_or("provider default"),
                args.sub_agent_model_reasoning_effort
                    .map_or("provider default", |effort| effort.as_str())
            ),
        ));
    }
    match endpoint {
        ExposeEndpointMode::LocalOnly => fields.push((
            "MCP transport".to_string(),
            "local loopback only".to_string(),
        )),
        ExposeEndpointMode::QuickTunnel => fields.push((
            "Cloudflare".to_string(),
            "Quick Tunnel connected".to_string(),
        )),
        ExposeEndpointMode::ExistingCloudflareTunnel {
            hostname,
            origin_port,
            tunnel,
        } => fields.push((
            "Cloudflare".to_string(),
            format!(
                "Existing Tunnel · {} · {hostname} · 127.0.0.1:{origin_port}",
                tunnel.as_deref().unwrap_or("configured identity")
            ),
        )),
        ExposeEndpointMode::OpenAiSecureMcp {
            tunnel_id,
            client_version,
        } => {
            fields.push((
                "MCP transport".to_string(),
                "OpenAI Secure MCP Tunnel".to_string(),
            ));
            fields.push(("Tunnel ID".to_string(), tunnel_id.clone()));
            fields.push((
                "tunnel-client".to_string(),
                format!("ready · {client_version} · ChatGPT connector unverified"),
            ));
            fields.push(("Browser".to_string(), "local loopback only".to_string()));
        }
    }
    print_panel("Prodex Super for ChatGPT", &fields)?;
    if let Some(public_url) = public_url {
        print_stdout_line(&format!("ChatGPT MCP URL: {}", public_url.as_str()))?;
    } else {
        print_stdout_line(&format!("Local MCP URL: {local_mcp_status}"))?;
    }
    if let Some(public_browser_url) = ready.public_browser_url.as_deref() {
        print_stdout_line(&format!("Public browser URL: {public_browser_url}"))?;
    }
    Ok(())
}

fn endpoint_provider(endpoint: &ExposeEndpointMode) -> &'static str {
    match endpoint {
        ExposeEndpointMode::LocalOnly => "Local only",
        ExposeEndpointMode::QuickTunnel => "Cloudflare Quick Tunnel",
        ExposeEndpointMode::ExistingCloudflareTunnel { .. } => "Existing Cloudflare Tunnel",
        ExposeEndpointMode::OpenAiSecureMcp { .. } => "OpenAI Secure MCP Tunnel",
    }
}

pub(crate) fn print_super_expose_configuration(
    args: &SuperArgs,
    workspace_name: &str,
) -> anyhow::Result<()> {
    let model = args
        .local_model
        .clone()
        .or_else(|| crate::codex_cli_config_override_value(&args.codex_args, "model"))
        .unwrap_or_else(|| "remembered/default".to_string());
    let effort = crate::codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort")
        .unwrap_or_else(|| "remembered/default".to_string());
    let mut fields = vec![
        ("Workspace".to_string(), workspace_name.to_string()),
        ("Main agent".to_string(), "Super".to_string()),
        (
            "Main provider".to_string(),
            expose_main_provider(args).label().to_string(),
        ),
        ("Main model".to_string(), model),
        ("Main effort".to_string(), effort),
        (
            "Sub-agents".to_string(),
            if args.sub_agent {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
        ),
    ];
    if args.sub_agent {
        fields.push((
            "Sub-agent model/effort".to_string(),
            format!(
                "{}/{}",
                args.sub_agent_model
                    .as_deref()
                    .unwrap_or("provider default"),
                args.sub_agent_model_reasoning_effort
                    .map_or("provider default", |effort| effort.as_str())
            ),
        ));
    }
    fields.push((
        "Access".to_string(),
        "Super · full machine access as the current OS user; workspace is initial context only"
            .to_string(),
    ));
    print_panel("Prodex Super Configuration", &fields)?;
    Ok(())
}

pub(crate) fn print_expose_dry_run(
    args: &ExposeArgs,
    endpoint: &ExposeEndpointMode,
) -> anyhow::Result<()> {
    let (mode, binary, bind, browser, mcp, source) = match endpoint {
        ExposeEndpointMode::LocalOnly => (
            "Local only",
            "none",
            "127.0.0.1:<os-assigned-port>".to_string(),
            "local only",
            "local loopback",
            "none",
        ),
        ExposeEndpointMode::QuickTunnel => (
            "Cloudflare Quick Tunnel",
            "cloudflared",
            "127.0.0.1:<os-assigned-port>".to_string(),
            "public",
            "Cloudflare public",
            "ephemeral Quick Tunnel",
        ),
        ExposeEndpointMode::ExistingCloudflareTunnel { origin_port, .. } => (
            "Existing Cloudflare Tunnel",
            "cloudflared",
            format!("127.0.0.1:{origin_port}"),
            "public",
            "Cloudflare public",
            if args.cloudflare_token_file.is_some() {
                "configured token file"
            } else {
                "configured Cloudflare config"
            },
        ),
        ExposeEndpointMode::OpenAiSecureMcp { .. } => (
            "OpenAI Secure MCP Tunnel",
            "tunnel-client",
            "127.0.0.1:<os-assigned-port>".to_string(),
            "local only",
            "OpenAI Secure MCP Tunnel",
            "configured OpenAI tunnel ID",
        ),
    };
    print_panel(
        "Expose dry run",
        &[
            ("Mode".to_string(), mode.to_string()),
            ("Tunnel provider".to_string(), mode.to_string()),
            ("Binary".to_string(), binary.to_string()),
            ("Local bind".to_string(), bind),
            ("Public browser".to_string(), browser.to_string()),
            ("MCP transport".to_string(), mcp.to_string()),
            ("Configuration".to_string(), source.to_string()),
            ("Secrets".to_string(), "not displayed".to_string()),
        ],
    )?;
    Ok(())
}
