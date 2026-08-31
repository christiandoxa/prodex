use super::EXPOSE_MAX_CLIENTS_LIMIT;
use super::mcp::{
    ExposeMcpEndpoint, PublicMcpEndpoint, expose_instance_id, verify_local_mcp_with_progress,
    verify_public_mcp_with_progress,
};
use super::runtime::{
    CloudflaredTransport, ExposeHttpServer, ExposePty, ExposeShared, OpenAiTunnel,
    cloudflared_command, ensure_openai_tunnel_available, expose_access_url, expose_public_host,
    resolve_openai_tunnel_id, start_openai_tunnel,
};
use super::session::{ExposeSessionStore, expose_random_token};
#[path = "super_expose_status.rs"]
mod status;
use crate::ExposeArgs;
use crate::app_state::AppStateIoExt;
use crate::print_launch_status;
use anyhow::{Context, bail};
use prodex_cli::{ExposeTunnelProvider, SuperArgs};
use redaction::redaction_redact_secret_like_text;
pub(super) use status::{print_super_expose_configuration, print_super_expose_status};
use std::collections::BTreeSet;
use std::env;
use std::fmt;
use std::io::{self, IsTerminal};
use std::net::IpAddr;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use terminal_ui::print_stderr_prompt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExposeEndpointMode {
    LocalOnly,
    QuickTunnel,
    ExistingCloudflareTunnel {
        hostname: String,
        origin_port: u16,
    },
    OpenAiSecureMcp {
        tunnel_id: String,
        client_version: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExposeLifecyclePhase {
    Preparing,
    CheckingCloudflared,
    StartingSuper,
    LocalMcpInitialize,
    LocalMcpTools,
    Cloudflare,
    OpenAiTunnel,
    PublicMcpInitialize,
    PublicMcpTools,
}

impl ExposeLifecyclePhase {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Preparing => "Preparing expose",
            Self::CheckingCloudflared => "Checking Cloudflare",
            Self::StartingSuper => "Starting Prodex Super",
            Self::LocalMcpInitialize => "Local MCP initialize",
            Self::LocalMcpTools => "Local MCP tools/list",
            Self::Cloudflare => "Cloudflare tunnel",
            Self::OpenAiTunnel => "OpenAI Secure MCP Tunnel",
            Self::PublicMcpInitialize => "Public MCP initialize",
            Self::PublicMcpTools => "Public MCP tools/list",
        }
    }

    pub(super) const fn order(self) -> usize {
        match self {
            Self::Preparing => 0,
            Self::CheckingCloudflared => 1,
            Self::StartingSuper => 2,
            Self::LocalMcpInitialize => 3,
            Self::LocalMcpTools => 4,
            Self::Cloudflare => 5,
            Self::OpenAiTunnel => 6,
            Self::PublicMcpInitialize => 7,
            Self::PublicMcpTools => 8,
        }
    }
}

pub(super) struct ExposeReadyState {
    pub(super) local_url: String,
    pub(super) local_mcp_url: PublicMcpEndpoint,
    pub(super) public_url: Option<PublicMcpEndpoint>,
    pub(super) instance_id: String,
    pub(super) workspace_name: String,
    pub(super) display_name: String,
    pub(super) endpoint: ExposeEndpointMode,
    pub(super) mcp: Arc<ExposeMcpEndpoint>,
}

impl fmt::Debug for ExposeReadyState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExposeReadyState")
            .field("local_url", &"<redacted>")
            .field("local_mcp_url", &self.local_mcp_url)
            .field("public_url", &self.public_url)
            .field("instance_id", &self.instance_id)
            .field("workspace_name", &self.workspace_name)
            .field("display_name", &self.display_name)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

pub(super) enum ExposeLifecycleEvent {
    Phase(ExposeLifecyclePhase),
    Ready(ExposeReadyState),
    Stopped,
    Failed(String),
}

impl fmt::Debug for ExposeLifecycleEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Phase(phase) => formatter.debug_tuple("Phase").field(phase).finish(),
            Self::Ready(_) => formatter.debug_tuple("Ready").field(&"<redacted>").finish(),
            Self::Stopped => formatter.write_str("Stopped"),
            Self::Failed(error) => formatter
                .debug_tuple("Failed")
                .field(&redaction_redact_secret_like_text(error))
                .finish(),
        }
    }
}

pub(super) fn handle_super_expose(mut args: ExposeArgs) -> anyhow::Result<()> {
    let mut super_args = args
        .super_args
        .take()
        .context("Super expose configuration is unavailable")?;
    validate_expose_launch_args(&super_args)?;
    super_args
        .extract_provider_overrides_from_codex_args()
        .map_err(anyhow::Error::msg)?;
    crate::runtime_gemini_cli::validate_super_native_cli_preflight(&super_args)?;
    super_args.validate_urls().map_err(anyhow::Error::msg)?;
    let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
    crate::resolve_super_expose_configuration(&mut super_args, interactive)?;
    let workspace_root = env::current_dir()
        .context("failed to capture expose workspace")?
        .canonicalize()
        .context("failed to resolve expose workspace")?;
    let workspace_name = expose_display_name(None, &workspace_root)?;
    let display_name = expose_display_name(args.name.as_deref(), &workspace_root)?;
    #[cfg(unix)]
    let _sigint = crate::InteractiveSigintGuard::install()
        .context("failed to install expose signal handler")?;
    if interactive
        && (args.tunnel || args.tunnel_provider == Some(ExposeTunnelProvider::Cloudflare))
    {
        return super::super_expose_ui::run(
            args,
            super_args,
            workspace_root,
            workspace_name,
            display_name,
        );
    }
    print_super_expose_configuration(&super_args, &workspace_name)?;
    let endpoint = select_expose_endpoint(&args)?;
    confirm_expose_start(interactive)?;
    run_super_expose_engine(
        ExposeEngineRequest {
            args,
            super_args,
            workspace_root,
            workspace_name,
            display_name,
            endpoint,
        },
        None,
        Arc::new(AtomicBool::new(false)),
    )
}

pub(super) fn validate_expose_launch_args(args: &SuperArgs) -> anyhow::Result<()> {
    if args.dry_run {
        bail!("--dry-run is not supported with `prodex s expose`; use `prodex s --dry-run`");
    }
    Ok(())
}

pub(super) struct ExposeEngineRequest {
    pub(super) args: ExposeArgs,
    pub(super) super_args: SuperArgs,
    pub(super) workspace_root: PathBuf,
    pub(super) workspace_name: String,
    pub(super) display_name: String,
    pub(super) endpoint: ExposeEndpointMode,
}

pub(super) fn run_super_expose_engine(
    request: ExposeEngineRequest,
    lifecycle_tx: Option<SyncSender<ExposeLifecycleEvent>>,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let result = run_super_expose_engine_inner(request, lifecycle_tx.as_ref(), &cancel);
    match result {
        Ok(()) => {
            emit_lifecycle(lifecycle_tx.as_ref(), ExposeLifecycleEvent::Stopped);
            Ok(())
        }
        Err(_error) if expose_cancelled(&cancel) => {
            emit_lifecycle(lifecycle_tx.as_ref(), ExposeLifecycleEvent::Stopped);
            Ok(())
        }
        Err(error) => {
            emit_lifecycle(
                lifecycle_tx.as_ref(),
                ExposeLifecycleEvent::Failed(redaction_redact_secret_like_text(&format!(
                    "{error:#}"
                ))),
            );
            Err(error)
        }
    }
}

fn run_super_expose_engine_inner(
    request: ExposeEngineRequest,
    lifecycle_tx: Option<&SyncSender<ExposeLifecycleEvent>>,
    cancel: &AtomicBool,
) -> anyhow::Result<()> {
    let ExposeEngineRequest {
        args,
        super_args,
        workspace_root,
        workspace_name,
        display_name,
        endpoint,
    } = request;
    report_phase(
        lifecycle_tx,
        ExposeLifecyclePhase::Preparing,
        "preparing Prodex Super expose...",
    );
    check_cancelled(cancel)?;
    remember_expose_model_preference(&super_args)?;
    if matches!(endpoint, ExposeEndpointMode::QuickTunnel) {
        report_phase(
            lifecycle_tx,
            ExposeLifecyclePhase::CheckingCloudflared,
            "checking Cloudflare Quick Tunnel...",
        );
        ensure_cloudflared_available()?;
    }
    check_cancelled(cancel)?;
    report_phase(
        lifecycle_tx,
        ExposeLifecyclePhase::StartingSuper,
        "starting Prodex Super Remote...",
    );
    let bootstrap = zeroize::Zeroizing::new(expose_random_token()?);
    let capability = zeroize::Zeroizing::new(expose_random_token()?);
    let instance_id = expose_instance_id()?;
    let listener = bind_expose_listener(&endpoint)?;
    let pty = ExposePty::spawn_in_cwd(&args, Some(&workspace_root))?;
    let listen_addr = listener
        .local_addr()
        .context("failed to inspect expose listen address")?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let mcp_workspace_name = display_name.clone();
    let mcp = ExposeMcpEndpoint::new(
        &capability,
        instance_id.clone(),
        workspace_root,
        mcp_workspace_name,
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
    check_cancelled(cancel)?;
    let local_origin = format!("http://{listen_addr}");
    let local_mcp_url = match PublicMcpEndpoint::new(&local_origin, &capability) {
        Ok(url) => url,
        Err(error) => {
            cleanup_super_expose(&shared, &mcp, &mut http, None, None);
            return Err(error);
        }
    };
    let cancelled = || expose_cancelled(cancel);
    let mut progress = |label: &str| report_probe_phase(lifecycle_tx, label);
    if let Err(error) =
        verify_local_mcp_with_progress(local_mcp_url.as_str(), &mut progress, &cancelled)
    {
        cleanup_super_expose(&shared, &mcp, &mut http, None, None);
        return Err(error);
    }
    let local_url = zeroize::Zeroizing::new(expose_access_url(&local_origin, &bootstrap));
    let mut cloudflared = None;
    let mut openai_tunnel = None;
    let public_url = match &endpoint {
        ExposeEndpointMode::LocalOnly => None,
        ExposeEndpointMode::OpenAiSecureMcp {
            tunnel_id,
            client_version,
        } => {
            report_phase(
                lifecycle_tx,
                ExposeLifecyclePhase::OpenAiTunnel,
                "starting OpenAI Secure MCP Tunnel...",
            );
            match start_openai_tunnel(
                local_mcp_url.as_str(),
                tunnel_id.clone(),
                client_version.clone(),
                &cancelled,
            ) {
                Ok(tunnel) => {
                    report_phase(
                        lifecycle_tx,
                        ExposeLifecyclePhase::OpenAiTunnel,
                        &format!(
                            "OpenAI tunnel-client ready ({})",
                            tunnel.status.client_version
                        ),
                    );
                    openai_tunnel = Some(tunnel);
                    None
                }
                Err(error) => {
                    cleanup_super_expose(&shared, &mcp, &mut http, None, None);
                    return Err(error);
                }
            }
        }
        ExposeEndpointMode::QuickTunnel | ExposeEndpointMode::ExistingCloudflareTunnel { .. } => {
            let (public_origin, public_host, tunnel) =
                match start_public_endpoint(&endpoint, &local_origin, lifecycle_tx, &cancelled) {
                    Ok(endpoint) => endpoint,
                    Err(error) => {
                        cleanup_super_expose(&shared, &mcp, &mut http, None, None);
                        return Err(error);
                    }
                };
            shared.allow_host(public_host.clone());
            shared.allow_mcp_only_host(public_host);
            let public_url = match PublicMcpEndpoint::new(&public_origin, &capability) {
                Ok(url) => url,
                Err(error) => {
                    let mut tunnel = tunnel;
                    cleanup_super_expose(&shared, &mcp, &mut http, tunnel.as_mut(), None);
                    return Err(error);
                }
            };
            let mut progress = |label: &str| report_probe_phase(lifecycle_tx, label);
            if let Err(error) =
                verify_public_mcp_with_progress(public_url.as_str(), &mut progress, &cancelled)
            {
                let mut tunnel = tunnel;
                cleanup_super_expose(&shared, &mcp, &mut http, tunnel.as_mut(), None);
                return Err(error);
            }
            cloudflared = tunnel;
            Some(public_url)
        }
    };
    let ready = ExposeReadyState {
        local_url: local_url.to_string(),
        local_mcp_url,
        public_url,
        instance_id,
        workspace_name,
        display_name,
        endpoint,
        mcp: Arc::clone(&mcp),
    };
    drop(capability);
    if let Some(lifecycle_tx) = lifecycle_tx {
        emit_lifecycle(Some(lifecycle_tx), ExposeLifecycleEvent::Ready(ready));
    } else {
        print_super_expose_status(&ready)?;
    }
    let mut tunnel_lost = false;
    while !shared.shutdown.load(Ordering::SeqCst) && !expose_cancelled(cancel) {
        if cloudflared
            .as_mut()
            .is_some_and(|tunnel| tunnel.exited().is_some())
            || openai_tunnel
                .as_mut()
                .is_some_and(|tunnel| tunnel.exited().is_some())
        {
            tunnel_lost = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    cleanup_super_expose(
        &shared,
        &mcp,
        &mut http,
        cloudflared.as_mut(),
        openai_tunnel.as_mut(),
    );
    if tunnel_lost {
        bail!("expose tunnel process exited after readiness; remote access is unavailable")
    }
    Ok(())
}

fn expose_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::SeqCst) || {
        #[cfg(unix)]
        {
            crate::InteractiveSigintGuard::count() > 0
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
}

fn check_cancelled(cancel: &AtomicBool) -> anyhow::Result<()> {
    if expose_cancelled(cancel) {
        bail!("expose startup cancelled")
    }
    Ok(())
}

fn emit_lifecycle(
    lifecycle_tx: Option<&SyncSender<ExposeLifecycleEvent>>,
    event: ExposeLifecycleEvent,
) {
    if let Some(lifecycle_tx) = lifecycle_tx {
        let _ = lifecycle_tx.try_send(event);
    }
}

fn report_phase(
    lifecycle_tx: Option<&SyncSender<ExposeLifecycleEvent>>,
    phase: ExposeLifecyclePhase,
    message: &str,
) {
    if lifecycle_tx.is_some() {
        emit_lifecycle(lifecycle_tx, ExposeLifecycleEvent::Phase(phase));
    } else {
        print_launch_status(message);
    }
}

fn report_probe_phase(lifecycle_tx: Option<&SyncSender<ExposeLifecycleEvent>>, label: &str) {
    let phase = match label {
        "local MCP initialize" => ExposeLifecyclePhase::LocalMcpInitialize,
        "local MCP tools/list" => ExposeLifecyclePhase::LocalMcpTools,
        "public MCP initialize" => ExposeLifecyclePhase::PublicMcpInitialize,
        "public MCP tools/list" => ExposeLifecyclePhase::PublicMcpTools,
        _ => return,
    };
    if lifecycle_tx.is_some() {
        emit_lifecycle(lifecycle_tx, ExposeLifecycleEvent::Phase(phase));
    } else {
        print_launch_status(&format!("Validating {label}..."));
    }
}

fn start_public_endpoint(
    endpoint: &ExposeEndpointMode,
    local_url: &str,
    lifecycle_tx: Option<&SyncSender<ExposeLifecycleEvent>>,
    cancelled: &dyn Fn() -> bool,
) -> anyhow::Result<(String, String, Option<super::runtime::CloudflaredTunnel>)> {
    match endpoint {
        ExposeEndpointMode::QuickTunnel => {
            let mut progress = |message: &str| {
                report_phase(lifecycle_tx, ExposeLifecyclePhase::Cloudflare, message)
            };
            let mut tunnel = super::runtime::start_cloudflared_tunnel_with_cancel_and_progress(
                local_url,
                cancelled,
                &mut progress,
            )?;
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
            report_phase(
                lifecycle_tx,
                ExposeLifecyclePhase::Cloudflare,
                &format!("Cloudflare {label} transport connected."),
            );
            let origin = tunnel
                .url
                .clone()
                .context("Cloudflare Quick Tunnel did not report a public hostname")?;
            let host = expose_public_host(&origin)
                .context("Cloudflare Quick Tunnel reported an invalid hostname")?;
            report_phase(
                lifecycle_tx,
                ExposeLifecyclePhase::Cloudflare,
                &format!("Cloudflare hostname allocated: {host}"),
            );
            Ok((origin, host, Some(tunnel)))
        }
        ExposeEndpointMode::ExistingCloudflareTunnel { hostname, .. } => {
            report_phase(
                lifecycle_tx,
                ExposeLifecyclePhase::Cloudflare,
                "Using existing Cloudflare Tunnel hostname.",
            );
            Ok((format!("https://{hostname}"), hostname.clone(), None))
        }
        ExposeEndpointMode::LocalOnly | ExposeEndpointMode::OpenAiSecureMcp { .. } => {
            unreachable!("non-Cloudflare endpoint passed to public endpoint startup")
        }
    }
}

fn select_expose_endpoint(args: &ExposeArgs) -> anyhow::Result<ExposeEndpointMode> {
    if args.openai_tunnel_id.is_some() && args.tunnel_provider != Some(ExposeTunnelProvider::OpenAi)
    {
        bail!("--openai-tunnel-id requires --tunnel-provider openai")
    }
    if args.no_tunnel {
        return Ok(ExposeEndpointMode::LocalOnly);
    }
    match args.tunnel_provider {
        Some(ExposeTunnelProvider::OpenAi) => {
            let tunnel_id = resolve_openai_tunnel_id(args.openai_tunnel_id.as_deref())?;
            let client_version = ensure_openai_tunnel_available(&tunnel_id)?;
            return Ok(ExposeEndpointMode::OpenAiSecureMcp {
                tunnel_id,
                client_version,
            });
        }
        Some(ExposeTunnelProvider::Cloudflare) => return Ok(ExposeEndpointMode::QuickTunnel),
        None if args.tunnel => return Ok(ExposeEndpointMode::QuickTunnel),
        None => {}
    }
    Ok(ExposeEndpointMode::LocalOnly)
}

fn confirm_expose_start(interactive: bool) -> anyhow::Result<()> {
    if !interactive {
        return Ok(());
    }
    print_stderr_prompt("Press Enter to start expose, or type q to cancel: ")?;
    let mut choice = String::new();
    if io::stdin().read_line(&mut choice)? == 0 {
        bail!("expose startup cancelled");
    }
    if choice.trim().eq_ignore_ascii_case("q") || choice.trim().eq_ignore_ascii_case("n") {
        bail!("expose startup cancelled");
    }
    Ok(())
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
        ExposeEndpointMode::LocalOnly
        | ExposeEndpointMode::QuickTunnel
        | ExposeEndpointMode::OpenAiSecureMcp { .. } => "127.0.0.1:0".to_string(),
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
    openai_tunnel: Option<&mut OpenAiTunnel>,
) {
    shared.shutdown.store(true, Ordering::SeqCst);
    mcp.run_manager.shutdown();
    if let Some(tunnel) = tunnel {
        tunnel.shutdown();
    }
    if let Some(tunnel) = openai_tunnel {
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
