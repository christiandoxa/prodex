#![forbid(unsafe_code)]
//! Bounded async HTTP/1 front for in-process or compatibility gateway handlers.

use std::{
    convert::Infallible,
    error::Error,
    future::Future,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context as _, Result, ensure};
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full, Limited, combinators::UnsyncBoxBody};
use hyper::{
    Request, Response, StatusCode,
    body::Incoming,
    header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue},
    upgrade,
};
use hyper_util::rt::TokioIo;
use prodex_gateway_http::{
    CanonicalRequestTarget, GatewayAdminRoute, GatewayEdgeSecurityError, GatewayEdgeSecurityPolicy,
    GatewayHttpHeader, GatewayHttpPolicy, GatewayHttpRouteKind, GatewayHttpRoutePlane,
    classify_request_target, parse_gateway_admin_route, validate_gateway_edge_security,
};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    sync::{OwnedSemaphorePermit, Semaphore, watch},
    task::JoinSet,
    time::{Instant, timeout, timeout_at},
};

mod channel_body;
mod compatibility;
mod connection;
mod handler;
mod in_process_upgrade;
mod ingress;
mod security;

#[cfg(test)]
use compatibility::LoopbackBackend;
use connection::serve_connection;
use ingress::*;

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

const ROUTE_UNAVAILABLE: &[u8] =
    br#"{"error":{"code":"route_not_available","message":"route is not available"}}"#;
const INVALID_REQUEST: &[u8] =
    br#"{"error":{"code":"invalid_request","message":"request is invalid"}}"#;
const INVALID_REQUEST_TARGET: &[u8] =
    br#"{"error":{"code":"invalid_request_target","message":"request target is invalid"}}"#;
const BODY_TOO_LARGE: &[u8] = br#"{"error":{"code":"request_body_too_large","message":"request body exceeds the configured limit"}}"#;
const BACKEND_TIMEOUT: &[u8] =
    br#"{"error":{"code":"backend_timeout","message":"gateway backend timed out"}}"#;
const SERVICE_UNAVAILABLE: &[u8] =
    br#"{"error":{"code":"service_unavailable","message":"gateway backend is unavailable"}}"#;
const LOCAL_OVERLOAD: &[u8] =
    br#"{"error":{"code":"service_unavailable","message":"gateway is temporarily overloaded"}}"#;
const EDGE_REQUEST_DENIED: &[u8] =
    br#"{"error":{"code":"edge_request_denied","message":"gateway edge request is denied"}}"#;
const MAX_FORWARDED_FOR_HOPS: usize = 16;
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

async fn handle_ingress_request<H, Fut>(
    mut request: Request<Incoming>,
    peer_addr: SocketAddr,
    mtls_peer_certificate_sha256: Option<[u8; 32]>,
    state: ServerState<H>,
    permit: Arc<OwnedSemaphorePermit>,
) -> Result<Response<GatewayResponseBody>, Infallible>
where
    H: Fn(GatewayHandlerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GatewayHandlerResult> + Send + 'static,
{
    let (target, route, plane, headers) = match parse_ingress_request(&request, state.mode) {
        Ok(parsed) => parsed,
        Err(response) => return Ok(*response),
    };
    let Ok(reload_transition) = state.reload.transition.try_read() else {
        return Ok(json_error(StatusCode::SERVICE_UNAVAILABLE, LOCAL_OVERLOAD));
    };
    let edge_security = Arc::clone(&state.reload.load().edge_security);
    let (client_ip, peer_is_trusted_proxy) =
        match validate_ingress_security(&request, plane, peer_addr, &edge_security, &headers) {
            Ok(metadata) => metadata,
            Err(response) => return Ok(*response),
        };
    if let Err(response) = validate_request_size(&request, state.max_request_body_bytes) {
        return Ok(*response);
    }

    let frontend_upgrade = request
        .headers()
        .contains_key(hyper::header::UPGRADE)
        .then(|| upgrade::on(&mut request));
    strip_forwarding_headers(&mut request);
    let (parts, body) = request.into_parts();
    let request = GatewayHandlerRequest {
        peer_addr,
        client_ip,
        peer_is_trusted_proxy,
        mtls_peer_certificate_sha256,
        target,
        route,
        request: Request::from_parts(parts, Limited::new(body, state.max_request_body_bytes)),
    };
    // Reload-coupled handler state must be snapshotted while this read barrier is held.
    let handled = (state.handler.as_ref())(request);
    drop(reload_transition);
    let handled = match timeout(state.response_header_timeout, handled).await {
        Err(_) => return Ok(json_error(StatusCode::GATEWAY_TIMEOUT, BACKEND_TIMEOUT)),
        Ok(Err(error)) => return Ok(handler_error_response(error)),
        Ok(Ok(handled)) => handled,
    };
    let GatewayHandlerResponse {
        response,
        backend_upgrade,
    } = handled;
    if response.status() == StatusCode::SWITCHING_PROTOCOLS
        && let (Some(frontend_upgrade), Some(backend_upgrade)) = (frontend_upgrade, backend_upgrade)
    {
        match backend_upgrade {
            GatewayHandlerUpgrade::Backend(backend_upgrade) => {
                tokio::spawn(tunnel_upgrades(
                    frontend_upgrade,
                    backend_upgrade,
                    state.shutdown.clone(),
                    state.max_connection_age,
                    permit,
                ));
            }
            GatewayHandlerUpgrade::InProcess(upgrade) => {
                tokio::spawn(tunnel_in_process_upgrade(
                    frontend_upgrade,
                    upgrade,
                    state.shutdown.clone(),
                    state.max_connection_age,
                    permit,
                ));
            }
        }
    }
    Ok(response)
}

fn gateway_http_headers(request: &Request<Incoming>) -> Option<Vec<GatewayHttpHeader>> {
    request
        .headers()
        .iter()
        .map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| GatewayHttpHeader::new(name.as_str(), value))
        })
        .collect()
}

/// Derives client network metadata only from the authenticated transport peer.
/// The right-most untrusted address defeats caller-prepended spoofed hops.
fn derive_gateway_client_ip(
    peer_addr: SocketAddr,
    trusted_proxies: &[IpAddr],
    headers: &[GatewayHttpHeader],
) -> Result<IpAddr, GatewayEdgeSecurityError> {
    let peer_is_trusted_proxy = trusted_proxies.contains(&peer_addr.ip());
    let has_forwarding_metadata = headers.iter().any(|header| {
        matches!(
            header.normalized_name().as_str(),
            "forwarded"
                | "x-forwarded-for"
                | "x-forwarded-host"
                | "x-forwarded-proto"
                | "x-real-ip"
        )
    });
    if has_forwarding_metadata && !peer_is_trusted_proxy {
        return Err(GatewayEdgeSecurityError::ForwardedHeaderFromUntrustedPeer);
    }
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "x-forwarded-for")
        .map(|header| header.value.as_str());
    let Some(value) = values.next() else {
        return Ok(peer_addr.ip());
    };
    if values.next().is_some() {
        return Err(GatewayEdgeSecurityError::ForwardedClientAddressInvalid);
    }
    let hops = value
        .split(',')
        .map(str::trim)
        .map(str::parse::<IpAddr>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GatewayEdgeSecurityError::ForwardedClientAddressInvalid)?;
    if hops.is_empty() || hops.len() > MAX_FORWARDED_FOR_HOPS {
        return Err(GatewayEdgeSecurityError::ForwardedClientAddressInvalid);
    }
    Ok(hops
        .iter()
        .rev()
        .find(|address| !trusted_proxies.contains(address))
        .copied()
        .unwrap_or(hops[0]))
}

fn loopback_compatible_expected_host<'a>(
    configured: &'a str,
    headers: &'a [GatewayHttpHeader],
) -> &'a str {
    let Ok(expected) = configured.parse::<SocketAddr>() else {
        return configured;
    };
    if !expected.ip().is_loopback() {
        return configured;
    }
    let mut hosts = headers
        .iter()
        .filter(|header| header.normalized_name() == "host")
        .map(|header| header.value.as_str());
    let Some(host) = hosts.next() else {
        return configured;
    };
    if hosts.next().is_some() {
        return configured;
    }
    let Ok(authority) = host.parse::<hyper::http::uri::Authority>() else {
        return configured;
    };
    if authority.port_u16().unwrap_or(80) != expected.port() {
        return configured;
    }
    let name = authority.host().trim_matches(['[', ']']);
    if name.eq_ignore_ascii_case("localhost")
        || name
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
    {
        host
    } else {
        configured
    }
}

fn browser_capable_request(request: &Request<Incoming>) -> bool {
    [
        "origin",
        "cookie",
        "sec-fetch-site",
        "sec-fetch-mode",
        "sec-fetch-dest",
        "x-csrf-token",
    ]
    .into_iter()
    .any(|name| request.headers().contains_key(name))
}

fn state_changing_method(method: &hyper::Method) -> bool {
    !matches!(
        *method,
        hyper::Method::GET | hyper::Method::HEAD | hyper::Method::OPTIONS
    )
}

fn handler_error_response(error: GatewayHandlerError) -> Response<GatewayResponseBody> {
    match error {
        GatewayHandlerError::InvalidRequest => json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST),
        GatewayHandlerError::InvalidRequestTarget => {
            json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST_TARGET)
        }
        GatewayHandlerError::RequestBodyTooLarge => {
            json_error(StatusCode::PAYLOAD_TOO_LARGE, BODY_TOO_LARGE)
        }
        GatewayHandlerError::Overloaded => {
            json_error(StatusCode::SERVICE_UNAVAILABLE, LOCAL_OVERLOAD)
        }
        GatewayHandlerError::Unavailable => {
            json_error(StatusCode::SERVICE_UNAVAILABLE, SERVICE_UNAVAILABLE)
        }
    }
}

fn route_allowed(
    mode: GatewayServerMode,
    target: &CanonicalRequestTarget,
    plane: GatewayHttpRoutePlane,
) -> bool {
    matches!(plane, GatewayHttpRoutePlane::Health)
        || matches!(
            (mode, plane),
            (
                GatewayServerMode::DataPlane,
                GatewayHttpRoutePlane::DataPlane
            ) | (
                GatewayServerMode::ControlPlane,
                GatewayHttpRoutePlane::ControlPlane
            )
        )
        || (mode == GatewayServerMode::DataPlane
            && plane == GatewayHttpRoutePlane::ControlPlane
            && matches!(
                gateway_admin_route(target),
                Some(GatewayAdminRoute::Metrics)
            ))
}

fn gateway_admin_route(target: &CanonicalRequestTarget) -> Option<GatewayAdminRoute<'_>> {
    parse_gateway_admin_route("", target.path())
        .or_else(|| parse_gateway_admin_route("/v1", target.path()))
}

fn content_length(request: &Request<Incoming>) -> Result<Option<u64>, ()> {
    request
        .headers()
        .get(CONTENT_LENGTH)
        .map(|value| value.to_str().map_err(|_| ())?.parse().map_err(|_| ()))
        .transpose()
}

fn caused_by_length_limit(error: &(dyn Error + 'static)) -> bool {
    let mut source = Some(error);
    while let Some(error) = source {
        if error.is::<http_body_util::LengthLimitError>() {
            return true;
        }
        source = error.source();
    }
    false
}

fn json_error(status: StatusCode, body: &'static [u8]) -> Response<GatewayResponseBody> {
    let mut response = Response::new(
        Full::new(Bytes::from_static(body))
            .map_err(|error: Infallible| -> GatewayBoxError { match error {} })
            .boxed_unsync(),
    );
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn tunnel_upgrades(
    frontend: upgrade::OnUpgrade,
    backend: upgrade::OnUpgrade,
    mut shutdown: watch::Receiver<bool>,
    max_connection_age: Duration,
    _permit: Arc<OwnedSemaphorePermit>,
) {
    let upgrades = async {
        let frontend = frontend.await?;
        let backend = backend.await?;
        Ok::<_, hyper::Error>((frontend, backend))
    };
    let Ok((frontend, backend)) = (tokio::select! {
        _ = shutdown.changed() => return,
        upgrades = upgrades => upgrades,
    }) else {
        return;
    };
    let mut frontend = TokioIo::new(frontend);
    let mut backend = TokioIo::new(backend);
    tokio::select! {
        _ = shutdown.changed() => {}
        _ = tokio::time::sleep(max_connection_age) => {}
        _ = tokio::io::copy_bidirectional(&mut frontend, &mut backend) => {}
    }
}

async fn tunnel_in_process_upgrade(
    frontend: upgrade::OnUpgrade,
    handoff: GatewayInProcessUpgradeHandoff,
    mut shutdown: watch::Receiver<bool>,
    max_connection_age: Duration,
    _permit: Arc<OwnedSemaphorePermit>,
) {
    let Ok(frontend) = (tokio::select! {
        _ = shutdown.changed() => return,
        frontend = frontend => frontend,
    }) else {
        return;
    };
    let (to_application, mut from_application, _request_guard) = handoff.into_channels();
    let frontend = TokioIo::new(frontend);
    let (mut frontend_read, mut frontend_write) = tokio::io::split(frontend);
    let upload = async move {
        let mut buffer = [0_u8; 8192];
        loop {
            let read = frontend_read.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            if to_application
                .send(Bytes::copy_from_slice(&buffer[..read]))
                .await
                .is_err()
            {
                break;
            }
        }
        Result::<(), std::io::Error>::Ok(())
    };
    let download = async move {
        while let Some(bytes) = from_application.recv().await {
            frontend_write.write_all(&bytes).await?;
        }
        frontend_write.shutdown().await
    };
    tokio::select! {
        _ = shutdown.changed() => {}
        _ = tokio::time::sleep(max_connection_age) => {}
        _ = async { let _ = tokio::try_join!(upload, download); } => {}
    }
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
