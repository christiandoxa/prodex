use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use arc_swap::ArcSwap;
use prodex_gateway_server::{
    GatewayServerBrowserSecurity, GatewayServerConfig, GatewayServerMode,
    GatewayServerReloadHandle, serve_with_handler_reloadable,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DedicatedServerMode {
    DataPlane,
    ControlPlane,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LiveConfigPublication {
    transport: PathBuf,
    replica: String,
    root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EnterpriseServeOptions {
    listen_addr: Option<SocketAddr>,
    publication: Option<LiveConfigPublication>,
}

pub fn run_enterprise_serve_or_exit(
    mode: DedicatedServerMode,
    args: impl Iterator<Item = String>,
    help: &str,
) {
    if let Err(error) = run_enterprise_serve(mode, args) {
        eprintln!("{error}\n\n{help}");
        std::process::exit(2);
    }
}

fn run_enterprise_serve(
    mode: DedicatedServerMode,
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let options = parse_serve_options(mode, args)?;
    let gateway_policy = prodex_app::runtime_policy_gateway().unwrap_or_default();
    let listen_addr = resolve_serve_listen_addr(
        mode,
        options.listen_addr,
        gateway_policy.listen_addr.as_deref(),
    )?;
    let (policy_mode, server_mode) = match mode {
        DedicatedServerMode::DataPlane => (
            prodex_app::RuntimePolicyServiceMode::Gateway,
            GatewayServerMode::DataPlane,
        ),
        DedicatedServerMode::ControlPlane => (
            prodex_app::RuntimePolicyServiceMode::ControlPlane,
            GatewayServerMode::ControlPlane,
        ),
    };
    let application = Arc::new(
        prodex_app::start_policy_gateway_application_for_mode_at(policy_mode, listen_addr)
            .map_err(|error| format!("failed to start gateway application: {error}"))?,
    );
    let applications = Arc::new(ArcSwap::from(application));
    let server_config = gateway_server_config(mode, listen_addr, server_mode)?;
    let server_reload = GatewayServerReloadHandle::new(&server_config)
        .map_err(|error| format!("failed to prepare reloadable gateway server: {error}"))?;
    if let Some(publication) = options.publication.as_ref() {
        deliver_live_config_publications(
            publication,
            mode,
            policy_mode,
            listen_addr,
            server_mode,
            &applications,
            &server_reload,
        )?;
    }
    let watcher_shutdown = Arc::new(AtomicBool::new(false));
    let watcher = options.publication.map(|publication| {
        spawn_live_config_publication_watcher(
            publication,
            mode,
            policy_mode,
            listen_addr,
            server_mode,
            Arc::clone(&applications),
            server_reload.clone(),
            Arc::clone(&watcher_shutdown),
        )
    });
    let handler_applications = Arc::clone(&applications);
    let server_result =
        serve_with_handler_reloadable(server_config, server_reload, move |request| {
            let application = handler_applications.load_full();
            async move { application.handle(request).await }
        })
        .map_err(|error| format!("gateway server failed: {error}"));
    watcher_shutdown.store(true, Ordering::SeqCst);
    if let Some(watcher) = watcher {
        let _ = watcher.join();
    }
    let application = applications.load_full();
    let drain_timeout = Duration::from_millis(
        prodex_gateway_http::GatewayHttpPolicy::production_default().connection_drain_timeout_ms,
    );
    let drained = application.shutdown_and_drain(drain_timeout);
    server_result?;
    if !drained {
        return Err("gateway application drain timed out".to_string());
    }
    Ok(())
}

fn gateway_server_config(
    mode: DedicatedServerMode,
    listen_addr: SocketAddr,
    server_mode: GatewayServerMode,
) -> Result<GatewayServerConfig, String> {
    let mut server_config = GatewayServerConfig::production(listen_addr, server_mode);
    let gateway_policy = prodex_app::runtime_policy_gateway().unwrap_or_default();
    if mode == DedicatedServerMode::ControlPlane {
        server_config.edge_security.browser = gateway_browser_security(
            gateway_policy.sso.browser_flow == Some(true),
            gateway_policy.sso.oidc_redirect_uri.as_deref(),
        )?;
    }
    server_config.edge_security.expected_host =
        gateway_expected_host(listen_addr, gateway_policy.expected_host)?;
    server_config.edge_security.trusted_proxies = gateway_policy
        .trusted_proxies
        .into_iter()
        .map(|proxy| {
            proxy
                .parse()
                .map_err(|_| "gateway trusted proxy must be an exact IP address".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if mode == DedicatedServerMode::DataPlane {
        server_config.tls = prodex_app::runtime_policy_gateway_tls_config()
            .map_err(|error| format!("failed to configure gateway TLS: {error}"))?;
    }
    Ok(server_config)
}

#[allow(clippy::too_many_arguments)]
fn deliver_live_config_publications(
    publication: &LiveConfigPublication,
    mode: DedicatedServerMode,
    policy_mode: prodex_app::RuntimePolicyServiceMode,
    listen_addr: SocketAddr,
    server_mode: GatewayServerMode,
    applications: &Arc<ArcSwap<prodex_app::GatewayApplication>>,
    server_reload: &GatewayServerReloadHandle,
) -> Result<(), String> {
    prodex_app::deliver_pending_config_publication_events_with_activation(
        &publication.transport,
        &publication.replica,
        &publication.root,
        |_| {
            activate_live_gateway_configuration(
                mode,
                policy_mode,
                listen_addr,
                server_mode,
                applications,
                server_reload,
            )
        },
    )
    .map(|_| ())
    .map_err(|error| format!("failed to consume live configuration publication: {error}"))
}

fn activate_live_gateway_configuration(
    mode: DedicatedServerMode,
    policy_mode: prodex_app::RuntimePolicyServiceMode,
    listen_addr: SocketAddr,
    server_mode: GatewayServerMode,
    applications: &Arc<ArcSwap<prodex_app::GatewayApplication>>,
    server_reload: &GatewayServerReloadHandle,
) -> anyhow::Result<()> {
    let candidate = Arc::new(prodex_app::start_policy_gateway_application_for_mode_at(
        policy_mode,
        listen_addr,
    )?);
    let server_config = match gateway_server_config(mode, listen_addr, server_mode) {
        Ok(config) => config,
        Err(error) => {
            let _ = candidate.shutdown_and_drain(Duration::from_secs(1));
            anyhow::bail!(error);
        }
    };
    let activated = Arc::clone(&candidate);
    let previous = match server_reload
        .reload_with_activation(&server_config, || applications.swap(activated))
    {
        Ok(previous) => previous,
        Err(error) => {
            let _ = candidate.shutdown_and_drain(Duration::from_secs(1));
            return Err(error);
        }
    };
    let drain_timeout = Duration::from_millis(
        prodex_gateway_http::GatewayHttpPolicy::production_default().connection_drain_timeout_ms,
    );
    let _ = previous.shutdown_and_drain(drain_timeout);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn spawn_live_config_publication_watcher(
    publication: LiveConfigPublication,
    mode: DedicatedServerMode,
    policy_mode: prodex_app::RuntimePolicyServiceMode,
    listen_addr: SocketAddr,
    server_mode: GatewayServerMode,
    applications: Arc<ArcSwap<prodex_app::GatewayApplication>>,
    server_reload: GatewayServerReloadHandle,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut last_error = None;
        while !shutdown.load(Ordering::SeqCst) {
            match deliver_live_config_publications(
                &publication,
                mode,
                policy_mode,
                listen_addr,
                server_mode,
                &applications,
                &server_reload,
            ) {
                Ok(()) => last_error = None,
                Err(error) => {
                    if last_error.as_deref() != Some(error.as_str()) {
                        eprintln!("config publication watcher: {error}");
                    }
                    last_error = Some(error);
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    })
}

fn gateway_browser_security(
    enabled: bool,
    redirect: Option<&str>,
) -> Result<Option<GatewayServerBrowserSecurity>, String> {
    if !enabled {
        return Ok(None);
    }
    let redirect = redirect.ok_or_else(|| "browser OIDC redirect URI is required".to_string())?;
    let redirect = reqwest::Url::parse(redirect)
        .map_err(|_| "browser OIDC redirect URI is invalid".to_string())?;
    let origin = redirect.origin().ascii_serialization();
    if origin == "null" {
        return Err("browser OIDC redirect origin is invalid".to_string());
    }
    Ok(Some(GatewayServerBrowserSecurity {
        expected_origin: origin,
        expected_csrf_token: None,
    }))
}

fn gateway_expected_host(
    listen_addr: SocketAddr,
    configured: Option<String>,
) -> Result<String, String> {
    match configured {
        Some(expected_host) => Ok(expected_host),
        None if listen_addr.ip().is_loopback() => Ok(listen_addr.to_string()),
        None => Err("non-loopback gateway serve requires gateway.expected_host".to_string()),
    }
}

#[cfg(test)]
fn parse_listen_addr(
    mode: DedicatedServerMode,
    args: impl Iterator<Item = String>,
) -> Result<SocketAddr, String> {
    let options = parse_serve_options(mode, args)?;
    resolve_serve_listen_addr(mode, options.listen_addr, None)
}

fn resolve_serve_listen_addr(
    mode: DedicatedServerMode,
    explicit: Option<SocketAddr>,
    configured: Option<&str>,
) -> Result<SocketAddr, String> {
    let default = match mode {
        DedicatedServerMode::DataPlane => "127.0.0.1:4000",
        DedicatedServerMode::ControlPlane => "127.0.0.1:4100",
    };
    explicit
        .map(Ok)
        .unwrap_or_else(|| configured.unwrap_or(default).parse())
        .map_err(|_| "invalid gateway listen address".to_string())
}

fn parse_serve_options(
    _mode: DedicatedServerMode,
    mut args: impl Iterator<Item = String>,
) -> Result<EnterpriseServeOptions, String> {
    let mut listen = None;
    let mut publication_transport = None;
    let mut publication_replica = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" if listen.is_none() => {
                listen = Some(
                    args.next()
                        .ok_or_else(|| "serve requires a value after --listen".to_string())?,
                );
            }
            "--listen" => return Err("serve accepts --listen only once".to_string()),
            "--config-publication-transport" if publication_transport.is_none() => {
                publication_transport = Some(PathBuf::from(args.next().ok_or_else(|| {
                    "serve requires a value after --config-publication-transport".to_string()
                })?));
            }
            "--config-publication-transport" => {
                return Err("serve accepts --config-publication-transport only once".to_string());
            }
            "--config-publication-replica" if publication_replica.is_none() => {
                publication_replica = Some(args.next().ok_or_else(|| {
                    "serve requires a value after --config-publication-replica".to_string()
                })?);
            }
            "--config-publication-replica" => {
                return Err("serve accepts --config-publication-replica only once".to_string());
            }
            other => return Err(format!("unknown serve argument: {other}")),
        }
    }
    let listen_addr = listen
        .as_deref()
        .map(str::parse)
        .transpose()
        .map_err(|_| "invalid serve listen address".to_string())?;
    let publication = match (publication_transport, publication_replica) {
        (None, None) => None,
        (Some(transport), Some(replica))
            if !replica.is_empty()
                && replica.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                }) =>
        {
            Some(LiveConfigPublication {
                transport,
                replica,
                root: prodex_app::runtime_policy_root()
                    .map_err(|error| format!("failed to resolve runtime policy root: {error}"))?,
            })
        }
        (Some(_), Some(_)) => {
            return Err("serve config publication replica is invalid".to_string());
        }
        _ => {
            return Err(
                "serve config publication requires both --config-publication-transport and --config-publication-replica"
                    .to_string(),
            );
        }
    };
    Ok(EnterpriseServeOptions {
        listen_addr,
        publication,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_listen_defaults_are_loopback_and_overrides_are_validated() {
        assert_eq!(
            parse_listen_addr(DedicatedServerMode::DataPlane, std::iter::empty()).unwrap(),
            "127.0.0.1:4000".parse().unwrap()
        );
        assert_eq!(
            parse_listen_addr(
                DedicatedServerMode::ControlPlane,
                ["--listen".to_string(), "0.0.0.0:4100".to_string()].into_iter(),
            )
            .unwrap(),
            "0.0.0.0:4100".parse().unwrap()
        );
        assert_eq!(
            parse_listen_addr(
                DedicatedServerMode::DataPlane,
                ["--listen".to_string(), "invalid".to_string()].into_iter(),
            ),
            Err("invalid serve listen address".to_string())
        );
        assert_eq!(
            resolve_serve_listen_addr(DedicatedServerMode::DataPlane, None, Some("0.0.0.0:4400"))
                .unwrap(),
            "0.0.0.0:4400".parse().unwrap()
        );
    }

    #[test]
    fn expected_host_defaults_only_for_loopback() {
        assert_eq!(
            gateway_expected_host("127.0.0.1:4000".parse().unwrap(), None).unwrap(),
            "127.0.0.1:4000"
        );
        assert!(gateway_expected_host("0.0.0.0:4000".parse().unwrap(), None).is_err());
        assert_eq!(
            gateway_expected_host(
                "0.0.0.0:4000".parse().unwrap(),
                Some("gateway.example.com".to_string()),
            )
            .unwrap(),
            "gateway.example.com"
        );
    }

    #[test]
    fn serve_config_publication_requires_a_complete_live_target() {
        assert!(
            parse_serve_options(
                DedicatedServerMode::DataPlane,
                [
                    "--config-publication-transport".to_string(),
                    "/tmp/config-events".to_string(),
                ]
                .into_iter(),
            )
            .is_err()
        );
        let options = parse_serve_options(
            DedicatedServerMode::DataPlane,
            [
                "--config-publication-transport".to_string(),
                "/tmp/config-events".to_string(),
                "--config-publication-replica".to_string(),
                "gateway-a".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();
        let publication = options.publication.unwrap();
        assert_eq!(publication.transport, PathBuf::from("/tmp/config-events"));
        assert_eq!(publication.replica, "gateway-a");
        assert_eq!(publication.root, prodex_app::runtime_policy_root().unwrap());
    }
}
