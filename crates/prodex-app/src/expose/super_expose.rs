use super::EXPOSE_MAX_CLIENTS_LIMIT;
use super::mcp::{
    ExposeMcpEndpoint, expose_instance_id, expose_main_provider, mcp_public_url, verify_local_mcp,
    verify_public_mcp,
};
use super::runtime::{
    CloudflaredTransport, ExposeHttpServer, ExposePty, ExposeShared, cloudflared_command,
    expose_access_url, expose_public_host, start_cloudflared_tunnel,
};
use super::session::{ExposeSessionStore, expose_random_token};
use crate::ExposeArgs;
use crate::app_state::AppStateIoExt;
use crate::print_launch_status;
use anyhow::{Context, bail};
use prodex_cli::SuperArgs;
use std::collections::BTreeSet;
use std::env;
use std::io::{self, IsTerminal};
use std::net::IpAddr;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use terminal_ui::{print_panel, print_stderr_line, print_stderr_prompt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExposeEndpointMode {
    QuickTunnel,
    ExistingCloudflareTunnel { hostname: String, origin_port: u16 },
}

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
    let endpoint = select_expose_endpoint(interactive)?;
    confirm_expose_start(interactive)?;
    remember_expose_model_preference(&super_args)?;
    if matches!(endpoint, ExposeEndpointMode::QuickTunnel) {
        ensure_cloudflared_available()?;
    }
    #[cfg(unix)]
    let _sigint = crate::InteractiveSigintGuard::install()
        .context("failed to install expose signal handler")?;
    let bootstrap = zeroize::Zeroizing::new(expose_random_token()?);
    let capability = zeroize::Zeroizing::new(expose_random_token()?);
    let instance_id = expose_instance_id()?;
    let listener = bind_expose_listener(&endpoint)?;
    let pty = ExposePty::spawn_in_cwd(&args, Some(&workspace_root))?;
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
    print_launch_status("starting Prodex Super Remote...");
    print_launch_status("waiting for local MCP server...");
    let local_mcp_url = mcp_public_url(&format!("http://{listen_addr}"), &capability);
    if let Err(error) = verify_local_mcp(&local_mcp_url) {
        cleanup_super_expose(&shared, &mcp, &mut http, None);
        return Err(error);
    }
    print_launch_status("Local MCP server ready.");
    drop(local_mcp_url);
    let local_url = zeroize::Zeroizing::new(expose_access_url(
        &format!("http://{listen_addr}"),
        &bootstrap,
    ));
    let (public_origin, public_host, mut tunnel) =
        match start_public_endpoint(&endpoint, &format!("http://{listen_addr}")) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                cleanup_super_expose(&shared, &mcp, &mut http, None);
                return Err(error);
            }
        };
    shared.allow_host(public_host.clone());
    shared.allow_mcp_only_host(public_host);
    let public_url = mcp_public_url(&public_origin, &capability);
    print_launch_status("waiting for public MCP readiness...");
    if let Err(error) = verify_public_mcp(&public_url) {
        cleanup_super_expose(&shared, &mcp, &mut http, tunnel.as_mut());
        return Err(error);
    }
    print_launch_status("Public MCP endpoint ready.");
    #[cfg(unix)]
    if crate::InteractiveSigintGuard::count() > 0 {
        cleanup_super_expose(&shared, &mcp, &mut http, tunnel.as_mut());
        return Ok(());
    }
    print_super_expose_status(
        &local_url,
        &public_url,
        &instance_id,
        &workspace_name,
        &display_name,
        &mcp,
        &endpoint,
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
        if tunnel
            .as_mut()
            .is_some_and(|tunnel| tunnel.exited().is_some())
        {
            tunnel_lost = true;
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    cleanup_super_expose(&shared, &mcp, &mut http, tunnel.as_mut());
    if tunnel_lost {
        bail!("Cloudflare Quick Tunnel exited after readiness; public access is unavailable")
    }
    Ok(())
}

fn start_public_endpoint(
    endpoint: &ExposeEndpointMode,
    local_url: &str,
) -> anyhow::Result<(String, String, Option<super::runtime::CloudflaredTunnel>)> {
    match endpoint {
        ExposeEndpointMode::QuickTunnel => {
            let mut tunnel = start_cloudflared_tunnel(local_url)?;
            if let Some(failure) = tunnel.startup_failure.take() {
                return Err(anyhow::anyhow!(failure));
            }
            let Some(transport) = tunnel.effective_transport else {
                return Err(cloudflared_start_failure(&mut tunnel));
            };
            let label = match transport {
                CloudflaredTransport::Auto => "auto",
                CloudflaredTransport::Quic => "QUIC",
                CloudflaredTransport::Http2 => "HTTP/2",
            };
            print_launch_status(&format!("Cloudflare {label} transport connected."));
            let origin = tunnel
                .url
                .clone()
                .context("Cloudflare Quick Tunnel did not report a public hostname")?;
            let host = expose_public_host(&origin)
                .context("Cloudflare Quick Tunnel reported an invalid hostname")?;
            print_launch_status("Cloudflare Quick Tunnel allocated.");
            Ok((origin, host, Some(tunnel)))
        }
        ExposeEndpointMode::ExistingCloudflareTunnel { hostname, .. } => {
            print_launch_status("Using existing Cloudflare Tunnel hostname.");
            Ok((format!("https://{hostname}"), hostname.clone(), None))
        }
    }
}

fn select_expose_endpoint(interactive: bool) -> anyhow::Result<ExposeEndpointMode> {
    if !interactive {
        return Ok(ExposeEndpointMode::QuickTunnel);
    }
    print_stderr_line("Public endpoint:")?;
    print_stderr_line("  1) Quick Tunnel (random trycloudflare.com hostname)")?;
    print_stderr_line("  2) Existing Cloudflare Tunnel (configured hostname)")?;
    loop {
        print_stderr_prompt("Choose endpoint [1]: ")?;
        let mut choice = String::new();
        if io::stdin().read_line(&mut choice)? == 0 {
            bail!("public endpoint selection cancelled");
        }
        match choice.trim() {
            "" | "1" => return Ok(ExposeEndpointMode::QuickTunnel),
            "2" => return prompt_existing_cloudflare_endpoint(),
            _ => print_stderr_line("Choose 1 for Quick Tunnel or 2 for an existing tunnel.")?,
        }
    }
}

fn confirm_expose_start(interactive: bool) -> anyhow::Result<()> {
    if !interactive {
        return Ok(());
    }
    print_stderr_prompt("Press Enter to start expose, or type q to cancel: ")?;
    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 {
        bail!("expose startup cancelled");
    }
    if input.trim().eq_ignore_ascii_case("q") || input.trim().eq_ignore_ascii_case("n") {
        bail!("expose startup cancelled");
    }
    Ok(())
}

fn prompt_existing_cloudflare_endpoint() -> anyhow::Result<ExposeEndpointMode> {
    let hostname = loop {
        print_stderr_prompt("Public hostname: ")?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            bail!("public endpoint selection cancelled");
        }
        match validate_existing_cloudflare_hostname(input.trim()) {
            Ok(hostname) => break hostname,
            Err(error) => print_stderr_line(&error.to_string())?,
        }
    };
    let origin_port = loop {
        print_stderr_prompt("Local origin port [8765]: ")?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            bail!("public endpoint selection cancelled");
        }
        let value = if input.trim().is_empty() {
            Ok(8765)
        } else {
            input
                .trim()
                .parse::<u16>()
                .map_err(|_| "origin port must be 1-65535".to_string())
        };
        match value {
            Ok(port) if port > 0 => break port,
            Ok(_) | Err(_) => print_stderr_line("origin port must be 1-65535")?,
        }
    };
    Ok(ExposeEndpointMode::ExistingCloudflareTunnel {
        hostname,
        origin_port,
    })
}

pub(super) fn validate_existing_cloudflare_hostname(value: &str) -> anyhow::Result<String> {
    if value.is_empty()
        || value.contains("://")
        || value.contains(['/', '\\', ':', '@', '?', '#'])
        || !value.contains('.')
        || value.parse::<IpAddr>().is_ok()
        || !super::runtime::expose_valid_dns_hostname(value)
    {
        bail!("hostname must be a public DNS name such as prodex.example.com")
    }
    Ok(value.to_ascii_lowercase())
}

pub(super) fn bind_expose_listener(endpoint: &ExposeEndpointMode) -> anyhow::Result<TcpListener> {
    let address = match endpoint {
        ExposeEndpointMode::QuickTunnel => "127.0.0.1:0".to_string(),
        ExposeEndpointMode::ExistingCloudflareTunnel { origin_port, .. } => {
            format!("127.0.0.1:{origin_port}")
        }
    };
    TcpListener::bind(&address).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!("LocalOriginPortInUse: {address} is already in use")
        } else {
            anyhow::Error::new(error).context(format!("failed to bind expose origin on {address}"))
        }
    })
}

fn remember_expose_model_preference(args: &SuperArgs) -> anyhow::Result<()> {
    let paths = crate::AppPaths::discover()?;
    let state = crate::AppState::load_and_repair(&paths)?;
    let codex_home = match crate::resolve_profile_name(&state, args.profile.as_deref()) {
        Ok(profile_name) => state
            .profiles
            .get(&profile_name)
            .map(|profile| profile.codex_home.clone())
            .context("selected expose profile is unavailable")?,
        Err(_error) if args.profile.is_none() => prodex_core::default_codex_home(&paths)?,
        Err(error) => return Err(error),
    };
    let runtime_args = args.clone().into_runtime_tool_args_with_presidio(false);
    let model = args
        .local_model
        .clone()
        .or_else(|| crate::codex_cli_config_override_value(&runtime_args.codex_args, "model"));
    let effort =
        crate::codex_cli_config_override_value(&runtime_args.codex_args, "model_reasoning_effort");
    crate::remember_model_preference_for_launch(
        &paths,
        &codex_home,
        &runtime_args.codex_args,
        model.as_deref(),
        effort.as_deref(),
        "expose-selection",
    )
}

fn cleanup_super_expose(
    shared: &ExposeShared,
    mcp: &ExposeMcpEndpoint,
    http: &mut ExposeHttpServer,
    tunnel: Option<&mut super::runtime::CloudflaredTunnel>,
) {
    shared.shutdown.store(true, Ordering::SeqCst);
    mcp.run_manager.shutdown();
    if let Some(tunnel) = tunnel {
        tunnel.shutdown();
    }
    shared.pty.shutdown();
    http.shutdown();
}

fn cloudflared_start_failure(tunnel: &mut super::runtime::CloudflaredTunnel) -> anyhow::Error {
    if let Some(status) = tunnel.exited() {
        return anyhow::anyhow!(
            "Cloudflare Quick Tunnel exited before obtaining a public hostname (status {})",
            status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        );
    }
    if tunnel.startup_timed_out {
        return anyhow::anyhow!(
            "Cloudflare Quick Tunnel did not complete transport negotiation; check outbound UDP/TCP 7844 connectivity"
        );
    }
    anyhow::anyhow!("Cloudflare Quick Tunnel did not report a public hostname")
}

pub(super) fn ensure_cloudflared_available() -> anyhow::Result<()> {
    let mut command = cloudflared_command();
    command.arg("--version");
    let output = crate::command_probe_output(&mut command, "cloudflared version probe");
    match output {
        Ok(output) if output.status.success() && cloudflared_version_is_parseable(&output) => {}
        Ok(_) => bail!(
            "cloudflared --version failed or reported an unsupported version; install a current cloudflared (no account or init is required)"
        ),
        Err(_) => bail!(
            "cloudflared is required for Quick Tunnel mode; install it from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/ (no account or init is required)"
        ),
    }
    let mut help = cloudflared_command();
    help.args(["tunnel", "--help"]);
    let help = crate::command_probe_output(&mut help, "cloudflared Quick Tunnel capability probe")
        .context("cloudflared Quick Tunnel capability probe failed; upgrade cloudflared")?;
    let help = String::from_utf8_lossy(&help.stdout);
    if !help.contains("--config") || !help.contains("--url") {
        bail!(
            "installed cloudflared does not support isolated Quick Tunnel configuration; upgrade cloudflared"
        );
    }
    Ok(())
}

fn cloudflared_version_is_parseable(output: &std::process::Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    [stdout.as_ref(), stderr.as_ref()].into_iter().any(|text| {
        text.split_whitespace().any(|token| {
            let token = token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '.'
            });
            let token = token.strip_prefix('v').unwrap_or(token);
            let token = token.split(['-', '+']).next().unwrap_or_default();
            let parts = token.split('.').collect::<Vec<_>>();
            parts.len() >= 2
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
    })
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

fn print_super_expose_status(
    local_url: &str,
    public_url: &str,
    instance_id: &str,
    workspace_name: &str,
    display_name: &str,
    mcp: &ExposeMcpEndpoint,
    endpoint: &ExposeEndpointMode,
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
            match endpoint {
                ExposeEndpointMode::QuickTunnel => "Quick Tunnel connected".to_string(),
                ExposeEndpointMode::ExistingCloudflareTunnel {
                    hostname,
                    origin_port,
                } => {
                    format!("Existing Tunnel · {hostname} · 127.0.0.1:{origin_port}")
                }
            },
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
