use std::{net::SocketAddr, sync::Arc, time::Duration};

use prodex_gateway_server::{GatewayServerConfig, GatewayServerMode, serve_with_handler};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DedicatedServerMode {
    DataPlane,
    ControlPlane,
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
    let listen_addr = parse_listen_addr(mode, args)?;
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
        prodex_app::start_policy_gateway_application_for_mode(policy_mode)
            .map_err(|error| format!("failed to start gateway application: {error}"))?,
    );
    let handler_application = Arc::clone(&application);
    let mut server_config = GatewayServerConfig::production(listen_addr, server_mode);
    let gateway_policy = prodex_app::runtime_policy_gateway().unwrap_or_default();
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
    let server_result = serve_with_handler(server_config, move |request| {
        let application = Arc::clone(&handler_application);
        async move { application.handle(request).await }
    })
    .map_err(|_| "gateway server failed".to_string());
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

fn parse_listen_addr(
    mode: DedicatedServerMode,
    mut args: impl Iterator<Item = String>,
) -> Result<SocketAddr, String> {
    let default = match mode {
        DedicatedServerMode::DataPlane => "127.0.0.1:4000",
        DedicatedServerMode::ControlPlane => "127.0.0.1:4100",
    };
    let mut listen = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" if listen.is_none() => {
                listen = Some(
                    args.next()
                        .ok_or_else(|| "serve requires a value after --listen".to_string())?,
                );
            }
            "--listen" => return Err("serve accepts --listen only once".to_string()),
            other => return Err(format!("unknown serve argument: {other}")),
        }
    }
    listen
        .as_deref()
        .unwrap_or(default)
        .parse()
        .map_err(|_| "invalid serve listen address".to_string())
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
}
