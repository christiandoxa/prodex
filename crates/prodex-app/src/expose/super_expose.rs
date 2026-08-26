use super::EXPOSE_MAX_CLIENTS_LIMIT;
use super::mcp::{
    ExposeMcpEndpoint, expose_instance_id, expose_main_provider, mcp_public_url, verify_public_mcp,
};
use super::runtime::{
    ExposeHttpServer, ExposePty, ExposeShared, expose_access_url, expose_public_host,
    start_cloudflared_tunnel,
};
use super::session::{ExposeSessionStore, expose_random_token};
use crate::ExposeArgs;
use anyhow::{Context, bail};
use prodex_cli::SuperArgs;
use std::collections::BTreeSet;
use std::env;
use std::io::{self, IsTerminal};
use std::net::TcpListener;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use terminal_ui::print_panel;

pub(super) fn handle_super_expose(mut args: ExposeArgs) -> anyhow::Result<()> {
    let mut super_args = args
        .super_args
        .take()
        .context("Super expose configuration is unavailable")?;
    super_args
        .extract_provider_overrides_from_codex_args()
        .map_err(anyhow::Error::msg)?;
    super_args.validate_urls().map_err(anyhow::Error::msg)?;
    let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
    crate::resolve_super_expose_configuration(&mut super_args, interactive)?;
    let workspace_root = env::current_dir()
        .context("failed to capture expose workspace")?
        .canonicalize()
        .context("failed to resolve expose workspace")?;
    let workspace_name = expose_display_name(None, &workspace_root)?;
    let display_name = expose_display_name(args.name.as_deref(), &workspace_root)?;
    print_super_expose_configuration(&super_args, &workspace_name)?;
    ensure_cloudflared_available()?;
    #[cfg(unix)]
    let _sigint = crate::InteractiveSigintGuard::install()
        .context("failed to install expose signal handler")?;
    let bootstrap = zeroize::Zeroizing::new(expose_random_token()?);
    let capability = zeroize::Zeroizing::new(expose_random_token()?);
    let instance_id = expose_instance_id()?;
    let pty = ExposePty::spawn_in_cwd(&args, Some(&workspace_root))?;
    let listener =
        TcpListener::bind("127.0.0.1:0").context("failed to bind expose server on 127.0.0.1:0")?;
    let listen_addr = listener
        .local_addr()
        .context("failed to inspect expose listen address")?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let workspace_name = display_name.clone();
    let mcp = ExposeMcpEndpoint::new(
        &capability,
        instance_id.clone(),
        workspace_root,
        workspace_name.clone(),
        display_name.clone(),
        super_args,
    );
    let shared = Arc::new(ExposeShared {
        sessions: Mutex::new(ExposeSessionStore::new(
            &bootstrap,
            usize::from(args.max_clients),
            Instant::now(),
        )),
        allowed_hosts: Mutex::new(BTreeSet::from([listen_addr.to_string()])),
        mcp_only_hosts: Mutex::new(BTreeSet::new()),
        mcp: Some(Arc::clone(&mcp)),
        pty,
        shutdown: Arc::clone(&shutdown),
        active_clients: AtomicUsize::new(0),
        active_requests: AtomicUsize::new(0),
        peak_requests: AtomicUsize::new(0),
        next_client_id: AtomicU64::new(1),
        max_clients: usize::from(args.max_clients).clamp(1, EXPOSE_MAX_CLIENTS_LIMIT),
    });
    let mut http = ExposeHttpServer::start(listener, Arc::clone(&shared))
        .context("failed to start expose HTTP server")?;
    let local_url = zeroize::Zeroizing::new(expose_access_url(
        &format!("http://{listen_addr}"),
        &bootstrap,
    ));
    let mut tunnel = match start_cloudflared_tunnel(&format!("http://{listen_addr}")) {
        Ok(tunnel) => tunnel,
        Err(error) => {
            shared.shutdown.store(true, Ordering::SeqCst);
            http.shutdown();
            return Err(error);
        }
    };
    let tunnel_origin = match tunnel.url.as_deref() {
        Some(url) => url.to_string(),
        None => {
            tunnel.shutdown();
            shared.shutdown.store(true, Ordering::SeqCst);
            http.shutdown();
            bail!("cloudflared did not report a Quick Tunnel hostname")
        }
    };
    let Some(public_host) = expose_public_host(&tunnel_origin) else {
        tunnel.shutdown();
        shared.shutdown.store(true, Ordering::SeqCst);
        http.shutdown();
        bail!("cloudflared reported an invalid Quick Tunnel hostname")
    };
    shared.allow_host(public_host.clone());
    shared.allow_mcp_only_host(public_host);
    let public_url = mcp_public_url(&tunnel_origin, &capability);
    if let Err(error) = verify_public_mcp(&public_url) {
        tunnel.shutdown();
        shared.shutdown.store(true, Ordering::SeqCst);
        mcp.run_manager.shutdown();
        shared.pty.shutdown();
        http.shutdown();
        return Err(error);
    }
    #[cfg(unix)]
    if crate::InteractiveSigintGuard::count() > 0 {
        shared.shutdown.store(true, Ordering::SeqCst);
        mcp.run_manager.shutdown();
        tunnel.shutdown();
        shared.pty.shutdown();
        http.shutdown();
        return Ok(());
    }
    print_super_expose_status(
        &local_url,
        &public_url,
        &instance_id,
        &workspace_name,
        &display_name,
        &mcp,
    )?;
    drop(local_url);
    drop(public_url);
    drop(capability);
    let mut tunnel_lost = false;
    while !shared.shutdown.load(Ordering::SeqCst) {
        #[cfg(unix)]
        if crate::InteractiveSigintGuard::count() > 0 {
            break;
        }
        if tunnel.exited().is_some() {
            tunnel_lost = true;
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    shared.shutdown.store(true, Ordering::SeqCst);
    mcp.run_manager.shutdown();
    tunnel.shutdown();
    shared.pty.shutdown();
    http.shutdown();
    if tunnel_lost {
        bail!("cloudflared exited; public access is unavailable; rerun `prodex s expose`")
    }
    Ok(())
}

pub(super) fn ensure_cloudflared_available() -> anyhow::Result<()> {
    let mut command = Command::new("cloudflared");
    command.arg("--version");
    let output = crate::command_probe_output(&mut command, "cloudflared version probe");
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => bail!(
            "cloudflared --version failed; install cloudflared (no account or init is required)"
        ),
        Err(_) => bail!(
            "cloudflared is required for Quick Tunnel mode; install it from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/ (no account or init is required)"
        ),
    }
}

fn expose_display_name(
    requested: Option<&str>,
    workspace_root: &std::path::Path,
) -> anyhow::Result<String> {
    let name = requested
        .or_else(|| workspace_root.file_name().and_then(|name| name.to_str()))
        .unwrap_or("workspace");
    if name.is_empty()
        || name.len() > 64
        || name.as_bytes().contains(&0)
        || name.chars().any(|ch| ch.is_control())
    {
        bail!("expose name must be 1-64 characters without control characters")
    }
    Ok(name.to_string())
}

pub(super) fn print_super_expose_status(
    local_url: &str,
    public_url: &str,
    instance_id: &str,
    workspace_name: &str,
    display_name: &str,
    mcp: &ExposeMcpEndpoint,
) -> anyhow::Result<()> {
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
            "FULL ACCESS: this URL controls Prodex Super in the current workspace".to_string(),
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
            "Cloudflare".to_string(),
            "Quick Tunnel connected".to_string(),
        ),
        (
            "Access".to_string(),
            "Ephemeral Capability Authentication".to_string(),
        ),
        ("ChatGPT MCP URL".to_string(), public_url.to_string()),
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
    print_panel("Prodex Super for ChatGPT", &fields)?;
    Ok(())
}

pub(super) fn print_super_expose_configuration(
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
        "full Super capability; workspace is the captured initial directory".to_string(),
    ));
    print_panel("Prodex Super Configuration", &fields)?;
    Ok(())
}
