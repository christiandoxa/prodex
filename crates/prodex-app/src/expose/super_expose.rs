use super::EXPOSE_MAX_CLIENTS_LIMIT;
use super::mcp::{
    ExposeMcpEndpoint, PublicMcpEndpoint, expose_instance_id, verify_local_browser_with_progress,
    verify_local_mcp_with_progress, verify_public_browser_with_progress,
    verify_public_mcp_with_progress,
};
pub(super) use super::runtime::ensure_cloudflared_available;
use super::runtime::{
    CloudflaredTransport, CloudflaredTunnel, ExposeHttpServer, ExposePty, ExposeShared,
    OpenAiTunnel, OpenAiTunnelCredentials, cloudflared_start_failure,
    ensure_openai_tunnel_available, expose_access_url, expose_display_name, expose_public_host,
    openai_tunnel_credentials_from_env, resolve_existing_cloudflare_selection,
    resolve_openai_tunnel_id, start_existing_cloudflared_tunnel, start_openai_tunnel,
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
pub(super) use status::{
    display_local_mcp_url, print_expose_dry_run, print_super_expose_configuration,
    print_super_expose_status,
};
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
        tunnel: Option<String>,
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
    LocalBrowser,
    Cloudflare,
    OpenAiTunnel,
    PublicMcpInitialize,
    PublicMcpTools,
    PublicBrowser,
}

impl ExposeLifecyclePhase {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Preparing => "Preparing expose",
            Self::CheckingCloudflared => "Checking Cloudflare",
            Self::StartingSuper => "Starting Prodex Super",
            Self::LocalMcpInitialize => "Local MCP initialize",
            Self::LocalMcpTools => "Local MCP tools/list",
            Self::LocalBrowser => "Local browser route",
            Self::Cloudflare => "Cloudflare tunnel",
            Self::OpenAiTunnel => "OpenAI Secure MCP Tunnel",
            Self::PublicMcpInitialize => "Public MCP initialize",
            Self::PublicMcpTools => "Public MCP tools/list",
            Self::PublicBrowser => "Public browser route",
        }
    }

    pub(super) const fn order(self) -> usize {
        match self {
            Self::Preparing => 0,
            Self::CheckingCloudflared => 1,
            Self::StartingSuper => 2,
            Self::LocalMcpInitialize => 3,
            Self::LocalMcpTools => 4,
            Self::LocalBrowser => 5,
            Self::Cloudflare => 6,
            Self::OpenAiTunnel => 7,
            Self::PublicMcpInitialize => 8,
            Self::PublicMcpTools => 9,
            Self::PublicBrowser => 10,
        }
    }
}

pub(super) struct ExposeReadyState {
    pub(super) local_url: String,
    pub(super) local_mcp_url: PublicMcpEndpoint,
    pub(super) public_browser_url: Option<String>,
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
            .field(
                "public_browser_url",
                &self.public_browser_url.as_ref().map(|_| "<redacted>"),
            )
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
    Ready(Box<ExposeReadyState>),
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
    let interactive =
        io::stdin().is_terminal() && io::stdout().is_terminal() && io::stderr().is_terminal();
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
    let explicit_endpoint =
        super_args.dry_run || args.no_tunnel || args.tunnel || args.tunnel_provider.is_some();
    let interactive_openai_setup = interactive
        && !super_args.dry_run
        && args.tunnel_provider == Some(ExposeTunnelProvider::OpenAi);
    if interactive && (!explicit_endpoint || interactive_openai_setup) {
        return super::super_expose_ui::run(
            args,
            super_args,
            workspace_root,
            workspace_name,
            display_name,
        );
    }
    print_super_expose_configuration(&super_args, &workspace_name)?;
    let endpoint = select_expose_endpoint(&args, !super_args.dry_run)?;
    let openai_credentials = if super_args.dry_run {
        None
    } else {
        match &endpoint {
            ExposeEndpointMode::OpenAiSecureMcp { tunnel_id, .. } => {
                Some(openai_tunnel_credentials_from_env(tunnel_id)?)
            }
            _ => None,
        }
    };
    if super_args.dry_run {
        print_expose_dry_run(&args, &endpoint)?;
        return Ok(());
    }
    confirm_expose_start(interactive)?;
    run_super_expose_engine(
        ExposeEngineRequest {
            args,
            super_args,
            workspace_root,
            workspace_name,
            display_name,
            endpoint,
            openai_credentials,
        },
        None,
        Arc::new(AtomicBool::new(false)),
    )
}

pub(super) fn validate_expose_launch_args(args: &SuperArgs) -> anyhow::Result<()> {
    let _ = args;
    Ok(())
}

pub(super) struct ExposeEngineRequest {
    pub(super) args: ExposeArgs,
    pub(super) super_args: SuperArgs,
    pub(super) workspace_root: PathBuf,
    pub(super) workspace_name: String,
    pub(super) display_name: String,
    pub(super) endpoint: ExposeEndpointMode,
    pub(super) openai_credentials: Option<OpenAiTunnelCredentials>,
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
        openai_credentials,
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
    if let Err(error) = verify_local_browser_with_progress(&local_url, &mut progress, &cancelled) {
        cleanup_super_expose(&shared, &mcp, &mut http, None, None);
        return Err(error);
    }
    let ExposeEndpointState {
        mut cloudflared,
        mut openai_tunnel,
        public_browser_url,
        public_url,
    } = start_expose_endpoint(ExposeEndpointStartup {
        args: &args,
        endpoint: &endpoint,
        openai_credentials,
        local_origin: &local_origin,
        local_mcp_url: &local_mcp_url,
        capability: &capability,
        bootstrap: &bootstrap,
        lifecycle_tx,
        cancelled: &cancelled,
        shared: &shared,
        mcp: &mcp,
        http: &mut http,
    })?;
    let ready = ExposeReadyState {
        local_url: local_url.to_string(),
        local_mcp_url,
        public_browser_url,
        public_url,
        instance_id,
        workspace_name,
        display_name,
        endpoint,
        mcp: Arc::clone(&mcp),
    };
    drop(capability);
    if let Some(lifecycle_tx) = lifecycle_tx {
        emit_lifecycle(
            Some(lifecycle_tx),
            ExposeLifecycleEvent::Ready(Box::new(ready)),
        );
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

struct ExposeEndpointState {
    cloudflared: Option<CloudflaredTunnel>,
    openai_tunnel: Option<OpenAiTunnel>,
    public_browser_url: Option<String>,
    public_url: Option<PublicMcpEndpoint>,
}

struct ExposeEndpointStartup<'a> {
    args: &'a ExposeArgs,
    endpoint: &'a ExposeEndpointMode,
    openai_credentials: Option<OpenAiTunnelCredentials>,
    local_origin: &'a str,
    local_mcp_url: &'a PublicMcpEndpoint,
    capability: &'a str,
    bootstrap: &'a str,
    lifecycle_tx: Option<&'a SyncSender<ExposeLifecycleEvent>>,
    cancelled: &'a dyn Fn() -> bool,
    shared: &'a ExposeShared,
    mcp: &'a ExposeMcpEndpoint,
    http: &'a mut ExposeHttpServer,
}

fn start_expose_endpoint(
    startup: ExposeEndpointStartup<'_>,
) -> anyhow::Result<ExposeEndpointState> {
    let ExposeEndpointStartup {
        args,
        endpoint,
        openai_credentials,
        local_origin,
        local_mcp_url,
        capability,
        bootstrap,
        lifecycle_tx,
        cancelled,
        shared,
        mcp,
        http,
    } = startup;
    match endpoint {
        ExposeEndpointMode::LocalOnly => Ok(ExposeEndpointState {
            cloudflared: None,
            openai_tunnel: None,
            public_browser_url: None,
            public_url: None,
        }),
        ExposeEndpointMode::OpenAiSecureMcp {
            tunnel_id,
            client_version,
        } => {
            report_phase(
                lifecycle_tx,
                ExposeLifecyclePhase::OpenAiTunnel,
                "starting OpenAI Secure MCP Tunnel...",
            );
            let credentials = match openai_credentials {
                Some(credentials) if credentials.tunnel_id() == tunnel_id => credentials,
                Some(_) => {
                    cleanup_super_expose(shared, mcp, http, None, None);
                    bail!("OpenAI tunnel credentials changed before startup")
                }
                None => {
                    cleanup_super_expose(shared, mcp, http, None, None);
                    bail!("OpenAI tunnel credentials are unavailable")
                }
            };
            let client_mcp_url = mcp
                .install_openai_relay(local_mcp_url)
                .inspect_err(|_| cleanup_super_expose(shared, mcp, http, None, None))?;
            match start_openai_tunnel(
                &client_mcp_url,
                credentials,
                client_version.clone(),
                cancelled,
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
                    Ok(ExposeEndpointState {
                        cloudflared: None,
                        openai_tunnel: Some(tunnel),
                        public_browser_url: None,
                        public_url: None,
                    })
                }
                Err(error) => {
                    cleanup_super_expose(shared, mcp, http, None, None);
                    Err(error)
                }
            }
        }
        ExposeEndpointMode::QuickTunnel | ExposeEndpointMode::ExistingCloudflareTunnel { .. } => {
            let (public_origin, public_host, mut tunnel) = match start_public_endpoint(
                endpoint,
                args,
                local_origin,
                lifecycle_tx,
                cancelled,
            ) {
                Ok(endpoint) => endpoint,
                Err(error) => {
                    cleanup_super_expose(shared, mcp, http, None, None);
                    return Err(error);
                }
            };
            shared.allow_host(public_host);
            let public_url = match PublicMcpEndpoint::new(&public_origin, capability) {
                Ok(url) => url,
                Err(error) => {
                    cleanup_super_expose(shared, mcp, http, tunnel.as_mut(), None);
                    return Err(error);
                }
            };
            let mut progress = |label: &str| report_probe_phase(lifecycle_tx, label);
            if let Err(error) =
                verify_public_mcp_with_progress(public_url.as_str(), &mut progress, cancelled)
            {
                cleanup_super_expose(shared, mcp, http, tunnel.as_mut(), None);
                return Err(error);
            }
            let public_browser_url = expose_access_url(&public_origin, bootstrap);
            if let Err(error) =
                verify_public_browser_with_progress(&public_browser_url, &mut progress, cancelled)
            {
                cleanup_super_expose(shared, mcp, http, tunnel.as_mut(), None);
                return Err(error);
            }
            Ok(ExposeEndpointState {
                cloudflared: tunnel,
                openai_tunnel: None,
                public_browser_url: Some(public_browser_url),
                public_url: Some(public_url),
            })
        }
    }
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
        "local browser route" => ExposeLifecyclePhase::LocalBrowser,
        "public MCP initialize" => ExposeLifecyclePhase::PublicMcpInitialize,
        "public MCP tools/list" => ExposeLifecyclePhase::PublicMcpTools,
        "public browser route" => ExposeLifecyclePhase::PublicBrowser,
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
    args: &ExposeArgs,
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
        ExposeEndpointMode::ExistingCloudflareTunnel {
            hostname,
            origin_port,
            tunnel: expected_tunnel,
        } => {
            report_phase(
                lifecycle_tx,
                ExposeLifecyclePhase::Cloudflare,
                "Using existing Cloudflare Tunnel hostname.",
            );
            let selection = resolve_existing_cloudflare_selection(args)?;
            if selection.hostname != *hostname
                || selection.origin_port != *origin_port
                || selection.tunnel != *expected_tunnel
            {
                bail!(
                    "existing Cloudflare selection changed before startup; reselect the configured route"
                )
            }
            let tunnel = start_existing_cloudflared_tunnel(&selection, cancelled)?;
            Ok((
                format!("https://{hostname}"),
                hostname.clone(),
                Some(tunnel),
            ))
        }
        ExposeEndpointMode::LocalOnly | ExposeEndpointMode::OpenAiSecureMcp { .. } => {
            unreachable!("non-Cloudflare endpoint passed to public endpoint startup")
        }
    }
}

pub(super) fn select_expose_endpoint(
    args: &ExposeArgs,
    probe_external_clients: bool,
) -> anyhow::Result<ExposeEndpointMode> {
    if args.openai_tunnel_id.is_some() && args.tunnel_provider != Some(ExposeTunnelProvider::OpenAi)
    {
        bail!("--openai-tunnel-id requires --tunnel-provider openai")
    }
    if (args.cloudflare_config.is_some()
        || args.cloudflare_tunnel.is_some()
        || args.cloudflare_hostname.is_some()
        || args.cloudflare_origin_port.is_some()
        || args.cloudflare_token_file.is_some())
        && args.tunnel_provider != Some(ExposeTunnelProvider::CloudflareExisting)
    {
        bail!("Cloudflare existing options require --tunnel-provider cloudflare-existing")
    }
    if args.no_tunnel {
        return Ok(ExposeEndpointMode::LocalOnly);
    }
    match args.tunnel_provider {
        Some(ExposeTunnelProvider::OpenAi) => {
            let tunnel_id = resolve_openai_tunnel_id(args.openai_tunnel_id.as_deref())?;
            let client_version = if probe_external_clients {
                let _ = openai_tunnel_credentials_from_env(&tunnel_id)?;
                ensure_openai_tunnel_available(&tunnel_id)?
            } else {
                "not probed (dry run)".to_string()
            };
            return Ok(ExposeEndpointMode::OpenAiSecureMcp {
                tunnel_id,
                client_version,
            });
        }
        Some(ExposeTunnelProvider::CloudflareQuick) => return Ok(ExposeEndpointMode::QuickTunnel),
        Some(ExposeTunnelProvider::CloudflareExisting) => {
            let selection = resolve_existing_cloudflare_selection(args)?;
            return Ok(ExposeEndpointMode::ExistingCloudflareTunnel {
                hostname: selection.hostname,
                origin_port: selection.origin_port,
                tunnel: selection.tunnel,
            });
        }
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
    mcp.clear_openai_relay();
    shared.pty.shutdown();
    http.shutdown();
}
