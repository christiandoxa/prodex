#![forbid(unsafe_code)]
//! Bounded async HTTP/1 front for in-process or compatibility gateway handlers.

use std::{error::Error, future::Future, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context as _, Result, ensure};
use bytes::Bytes;
use http_body_util::{Limited, combinators::UnsyncBoxBody};
use hyper::body::Incoming;
use prodex_gateway_http::GatewayHttpPolicy;
use sha2::{Digest, Sha256};
use tokio::{
    net::TcpListener,
    sync::{Semaphore, watch},
    task::JoinSet,
    time::{Instant, timeout, timeout_at},
};

mod channel_body;
mod compatibility;
mod connection;
mod handler;
mod in_process_upgrade;
mod ingress;
mod request;
mod security;

#[cfg(test)]
use compatibility::LoopbackBackend;
use connection::serve_connection;

pub use channel_body::{
    GatewayResponseBodySender, bounded_response_body, bounded_response_body_with_guard,
};
pub use compatibility::{serve, serve_with_handler, serve_with_handler_reloadable};
pub use handler::{
    GatewayHandlerError, GatewayHandlerRequest, GatewayHandlerResponse, GatewayHandlerResult,
    GatewayHandlerUpgrade,
};
pub use in_process_upgrade::{
    GatewayInProcessUpgrade, GatewayInProcessUpgradeHandoff, bounded_in_process_upgrade,
};
pub use security::{
    GatewayServerBrowserSecurity, GatewayServerEdgeSecurity, GatewayServerReloadHandle,
    GatewayServerTlsConfig,
};

pub type GatewayBoxError = Box<dyn Error + Send + Sync>;
pub type GatewayRequestBody = Limited<Incoming>;
pub type GatewayResponseBody = UnsyncBoxBody<Bytes, GatewayBoxError>;

const DEFAULT_REQUEST_HEADER_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_CONNECTION_AGE: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayServerMode {
    DataPlane,
    ControlPlane,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayServerConfig {
    pub listen_addr: SocketAddr,
    pub mode: GatewayServerMode,
    pub max_connections: usize,
    pub max_request_body_bytes: usize,
    pub request_header_timeout: Duration,
    pub response_header_timeout: Duration,
    pub max_connection_age: Duration,
    pub drain_timeout: Duration,
    pub edge_security: GatewayServerEdgeSecurity,
    pub tls: Option<GatewayServerTlsConfig>,
}

impl GatewayServerConfig {
    pub fn production(listen_addr: SocketAddr, mode: GatewayServerMode) -> Self {
        let policy = GatewayHttpPolicy::production_default();
        Self {
            listen_addr,
            mode,
            max_connections: policy.max_concurrent_streams as usize,
            max_request_body_bytes: policy.max_body_bytes,
            request_header_timeout: DEFAULT_REQUEST_HEADER_TIMEOUT,
            response_header_timeout: Duration::from_millis(policy.request_timeout_ms),
            max_connection_age: DEFAULT_MAX_CONNECTION_AGE,
            drain_timeout: Duration::from_millis(policy.connection_drain_timeout_ms),
            edge_security: GatewayServerEdgeSecurity {
                trusted_proxies: Vec::new(),
                expected_host: if listen_addr.ip().is_loopback() {
                    listen_addr.to_string()
                } else {
                    String::new()
                },
                browser: None,
            },
            tls: None,
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.max_connections > 0 && self.max_connections <= u32::MAX as usize,
            "gateway max_connections must be between 1 and u32::MAX"
        );
        ensure!(
            self.max_request_body_bytes > 0,
            "gateway max_request_body_bytes must be non-zero"
        );
        ensure!(
            !self.request_header_timeout.is_zero(),
            "gateway request_header_timeout must be non-zero"
        );
        ensure!(
            !self.response_header_timeout.is_zero(),
            "gateway response_header_timeout must be non-zero"
        );
        ensure!(
            !self.max_connection_age.is_zero(),
            "gateway max_connection_age must be non-zero"
        );
        ensure!(
            !self.drain_timeout.is_zero(),
            "gateway drain_timeout must be non-zero"
        );
        ensure!(
            !self.edge_security.expected_host.is_empty()
                && self.edge_security.expected_host.len() <= 263
                && !self
                    .edge_security
                    .expected_host
                    .chars()
                    .any(char::is_whitespace)
                && self
                    .edge_security
                    .expected_host
                    .parse::<hyper::http::uri::Authority>()
                    .is_ok(),
            "gateway expected_host must be a bounded exact HTTP authority"
        );
        if let Some(browser) = &self.edge_security.browser {
            ensure!(
                !browser.expected_origin.is_empty()
                    && browser
                        .expected_csrf_token
                        .as_deref()
                        .is_none_or(|token| !token.is_empty()),
                "gateway browser edge policy must be complete"
            );
        }
        if let Some(tls) = &self.tls {
            tls.server_config()?;
        }
        Ok(())
    }
}

struct ServerState<H> {
    mode: GatewayServerMode,
    handler: Arc<H>,
    reload: GatewayServerReloadHandle,
    max_request_body_bytes: usize,
    request_header_timeout: Duration,
    response_header_timeout: Duration,
    max_connection_age: Duration,
    shutdown: watch::Receiver<bool>,
}

impl<H> Clone for ServerState<H> {
    fn clone(&self) -> Self {
        Self {
            mode: self.mode,
            handler: Arc::clone(&self.handler),
            reload: self.reload.clone(),
            max_request_body_bytes: self.max_request_body_bytes,
            request_header_timeout: self.request_header_timeout,
            response_header_timeout: self.response_header_timeout,
            max_connection_age: self.max_connection_age,
            shutdown: self.shutdown.clone(),
        }
    }
}

async fn run_with_handler<F, H, Fut>(
    listener: TcpListener,
    config: GatewayServerConfig,
    handler: H,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = Result<()>>,
    H: Fn(GatewayHandlerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GatewayHandlerResult> + Send + 'static,
{
    let reload = GatewayServerReloadHandle::new(&config)?;
    run_with_handler_reloadable(listener, config, reload, handler, shutdown).await
}

async fn run_with_handler_reloadable<F, H, Fut>(
    listener: TcpListener,
    config: GatewayServerConfig,
    reload: GatewayServerReloadHandle,
    handler: H,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = Result<()>>,
    H: Fn(GatewayHandlerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GatewayHandlerResult> + Send + 'static,
{
    config.validate()?;
    let handler = Arc::new(handler);
    let connections = Arc::new(Semaphore::new(config.max_connections));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut tasks = JoinSet::new();
    tokio::pin!(shutdown);

    let stop_result = loop {
        let permit = tokio::select! {
            result = shutdown.as_mut() => break result,
            permit = Arc::clone(&connections).acquire_owned() => {
                permit.context("gateway connection limiter closed")?
            }
        };
        let accepted = tokio::select! {
            result = shutdown.as_mut() => {
                drop(permit);
                break result;
            }
            accepted = listener.accept() => accepted,
        };
        let (stream, peer_addr) = match accepted {
            Ok(accepted) => accepted,
            Err(_) => {
                drop(permit);
                tokio::select! {
                    result = shutdown.as_mut() => break result,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
                }
            }
        };
        let security = reload.load();
        let state = ServerState {
            mode: config.mode,
            handler: Arc::clone(&handler),
            reload: reload.clone(),
            max_request_body_bytes: config.max_request_body_bytes,
            request_header_timeout: config.request_header_timeout,
            response_header_timeout: config.response_header_timeout,
            max_connection_age: config.max_connection_age,
            shutdown: shutdown_rx.clone(),
        };
        if let Some(tls_acceptor) = security.tls_acceptor.clone() {
            tasks.spawn(async move {
                let handshake =
                    timeout(state.request_header_timeout, tls_acceptor.accept(stream)).await;
                let Ok(Ok(stream)) = handshake else {
                    return;
                };
                let mtls_peer_certificate_sha256 = stream
                    .get_ref()
                    .1
                    .peer_certificates()
                    .and_then(|certificates| certificates.first())
                    .map(|certificate| Sha256::digest(certificate.as_ref()).into());
                serve_connection(
                    stream,
                    peer_addr,
                    mtls_peer_certificate_sha256,
                    state,
                    permit,
                )
                .await;
            });
        } else {
            tasks.spawn(serve_connection(stream, peer_addr, None, state, permit));
        }
    };

    drop(listener);
    let _ = shutdown_tx.send(true);
    let deadline = Instant::now() + config.drain_timeout;
    let max_connections = config.max_connections as u32;
    let drain = async {
        while tasks.join_next().await.is_some() {}
        let _all_permits = Arc::clone(&connections)
            .acquire_many_owned(max_connections)
            .await
            .context("gateway connection limiter closed")?;
        Result::<()>::Ok(())
    };
    match timeout_at(deadline, drain).await {
        Ok(result) => result?,
        Err(_) => {
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
            return Err(anyhow::anyhow!("gateway server drain timed out"));
        }
    }
    stop_result
}

#[cfg(unix)]
async fn shutdown_signal() -> Result<()> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("failed to register gateway SIGTERM handler")?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result.context("failed to wait for gateway SIGINT"),
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("failed to wait for gateway shutdown signal")
}

#[cfg(test)]
mod tests;
