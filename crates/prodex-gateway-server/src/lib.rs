#![forbid(unsafe_code)]
//! Bounded async HTTP/1 front for in-process or compatibility gateway handlers.

use std::{
    convert::Infallible,
    error::Error,
    fmt,
    future::Future,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context as _, Result, ensure};
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full, Limited, combinators::UnsyncBoxBody};
use hyper::{
    Request, Response, StatusCode, Uri,
    body::{Body, Incoming},
    header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderName, HeaderValue},
    upgrade,
};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::{TokioExecutor, TokioIo},
};
use prodex_gateway_http::{
    CanonicalRequestTarget, GatewayEdgeSecurityError, GatewayEdgeSecurityPolicy, GatewayHttpHeader,
    GatewayHttpPolicy, GatewayHttpRouteKind, GatewayHttpRoutePlane, classify_request_target,
    validate_gateway_edge_security,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    sync::{OwnedSemaphorePermit, Semaphore, watch},
    task::JoinSet,
    time::{Instant, timeout, timeout_at},
};

mod channel_body;
mod connection;
mod in_process_upgrade;

use connection::serve_connection;

pub use channel_body::{
    GatewayResponseBodySender, bounded_response_body, bounded_response_body_with_guard,
};
pub use in_process_upgrade::{
    GatewayInProcessUpgrade, GatewayInProcessUpgradeHandoff, bounded_in_process_upgrade,
};

pub type GatewayBoxError = Box<dyn Error + Send + Sync>;
pub type GatewayRequestBody = Limited<Incoming>;
pub type GatewayResponseBody = UnsyncBoxBody<Bytes, GatewayBoxError>;

type ProxyClient = Client<HttpConnector, GatewayRequestBody>;

const ROUTE_UNAVAILABLE: &[u8] =
    br#"{"error":{"code":"route_not_available","message":"route is not available"}}"#;
const INVALID_REQUEST: &[u8] =
    br#"{"error":{"code":"invalid_request","message":"request is invalid"}}"#;
const INVALID_REQUEST_TARGET: &[u8] =
    br#"{"error":{"code":"invalid_request_target","message":"request target is invalid"}}"#;
const BODY_TOO_LARGE: &[u8] = br#"{"error":{"code":"request_body_too_large","message":"request body exceeds the configured limit"}}"#;
const BACKEND_TIMEOUT: &[u8] =
    br#"{"error":{"code":"backend_timeout","message":"gateway backend timed out"}}"#;
const BACKEND_UNAVAILABLE: &[u8] =
    br#"{"error":{"code":"backend_unavailable","message":"gateway backend is unavailable"}}"#;
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
}

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayServerEdgeSecurity {
    pub trusted_proxies: Vec<IpAddr>,
    pub expected_host: String,
    pub browser: Option<GatewayServerBrowserSecurity>,
}

impl fmt::Debug for GatewayServerEdgeSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayServerEdgeSecurity")
            .field("trusted_proxies", &self.trusted_proxies)
            .field("expected_host", &"<redacted>")
            .field("browser", &self.browser)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayServerBrowserSecurity {
    pub expected_origin: String,
    pub expected_csrf_token: String,
}

impl fmt::Debug for GatewayServerBrowserSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayServerBrowserSecurity")
            .field("expected_origin", &"<redacted>")
            .field("expected_csrf_token", &"<redacted>")
            .finish()
    }
}

/// Canonical, route-classified request delivered to an in-process gateway handler.
pub struct GatewayHandlerRequest {
    pub peer_addr: SocketAddr,
    pub client_ip: IpAddr,
    pub peer_is_trusted_proxy: bool,
    pub target: CanonicalRequestTarget,
    pub route: GatewayHttpRouteKind,
    pub request: Request<GatewayRequestBody>,
}

/// Streaming response returned by an in-process gateway handler.
pub struct GatewayHandlerResponse {
    pub response: Response<GatewayResponseBody>,
    pub backend_upgrade: Option<GatewayHandlerUpgrade>,
}

pub enum GatewayHandlerUpgrade {
    Backend(upgrade::OnUpgrade),
    InProcess(GatewayInProcessUpgradeHandoff),
}

impl GatewayHandlerResponse {
    pub fn new<B>(response: Response<B>) -> Self
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Error + Send + Sync + 'static,
    {
        Self {
            response: response.map(|body| {
                body.map_err(|error| Box::new(error) as GatewayBoxError)
                    .boxed_unsync()
            }),
            backend_upgrade: None,
        }
    }

    pub fn with_backend_upgrade<B>(
        response: Response<B>,
        backend_upgrade: upgrade::OnUpgrade,
    ) -> Self
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Error + Send + Sync + 'static,
    {
        let mut handled = Self::new(response);
        handled.backend_upgrade = Some(GatewayHandlerUpgrade::Backend(backend_upgrade));
        handled
    }

    pub fn with_in_process_upgrade<B>(
        response: Response<B>,
        upgrade: GatewayInProcessUpgradeHandoff,
    ) -> Self
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Error + Send + Sync + 'static,
    {
        let mut handled = Self::new(response);
        handled.backend_upgrade = Some(GatewayHandlerUpgrade::InProcess(upgrade));
        handled
    }

    pub fn from_parts(
        status: u16,
        headers: Vec<(String, Vec<u8>)>,
        content_length: Option<usize>,
        body: GatewayResponseBody,
    ) -> GatewayHandlerResult {
        let mut response = Response::new(body);
        *response.status_mut() =
            StatusCode::from_u16(status).map_err(|_| GatewayHandlerError::Unavailable)?;
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| GatewayHandlerError::Unavailable)?;
            let value =
                HeaderValue::from_bytes(&value).map_err(|_| GatewayHandlerError::Unavailable)?;
            response.headers_mut().append(name, value);
        }
        if let Some(content_length) = content_length {
            response.headers_mut().insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&content_length.to_string())
                    .map_err(|_| GatewayHandlerError::Unavailable)?,
            );
        }
        Ok(Self {
            response,
            backend_upgrade: None,
        })
    }

    pub fn with_in_process_upgrade_handoff(
        mut self,
        upgrade: GatewayInProcessUpgradeHandoff,
    ) -> Self {
        self.backend_upgrade = Some(GatewayHandlerUpgrade::InProcess(upgrade));
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHandlerError {
    InvalidRequest,
    InvalidRequestTarget,
    RequestBodyTooLarge,
    Overloaded,
    Unavailable,
}

impl fmt::Display for GatewayHandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gateway handler failed")
    }
}

impl Error for GatewayHandlerError {}

pub type GatewayHandlerResult = std::result::Result<GatewayHandlerResponse, GatewayHandlerError>;

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
                !browser.expected_origin.is_empty() && !browser.expected_csrf_token.is_empty(),
                "gateway browser edge policy must be complete"
            );
        }
        Ok(())
    }
}

/// Runs the compatibility front until SIGINT or SIGTERM, then drains open connections.
pub fn serve(config: GatewayServerConfig, backend_addr: SocketAddr) -> Result<()> {
    let backend = LoopbackBackend::new(backend_addr)?;
    serve_with_handler(config, move |request| {
        let backend = backend.clone();
        async move { backend.handle(request).await }
    })
}

/// Runs the gateway with an in-process request handler until SIGINT or SIGTERM.
pub fn serve_with_handler<H, Fut>(config: GatewayServerConfig, handler: H) -> Result<()>
where
    H: Fn(GatewayHandlerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GatewayHandlerResult> + Send + 'static,
{
    config.validate()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to initialize gateway server runtime")?;
    runtime.block_on(async move {
        let listener = TcpListener::bind(config.listen_addr)
            .await
            .context("failed to bind gateway server listener")?;
        run_with_handler(listener, config, handler, shutdown_signal()).await
    })
}

#[derive(Clone)]
struct LoopbackBackend {
    authority: hyper::http::uri::Authority,
    host: HeaderValue,
    client: ProxyClient,
}

impl LoopbackBackend {
    fn new(backend_addr: SocketAddr) -> Result<Self> {
        let backend = backend_addr.to_string();
        Ok(Self {
            authority: backend
                .parse()
                .context("failed to prepare gateway backend authority")?,
            host: HeaderValue::from_str(&backend)
                .context("failed to prepare gateway backend host header")?,
            client: Client::builder(TokioExecutor::new()).build_http(),
        })
    }

    async fn handle(&self, request: GatewayHandlerRequest) -> GatewayHandlerResult {
        let GatewayHandlerRequest {
            peer_addr: _,
            client_ip: _,
            peer_is_trusted_proxy: _,
            target,
            route: _,
            request,
        } = request;
        let (mut parts, body) = request.into_parts();
        let mut uri_parts = parts.uri.into_parts();
        uri_parts.scheme = Some(hyper::http::uri::Scheme::HTTP);
        uri_parts.authority = Some(self.authority.clone());
        uri_parts.path_and_query = Some(
            target
                .path_and_query()
                .parse()
                .map_err(|_| GatewayHandlerError::InvalidRequestTarget)?,
        );
        parts.uri = Uri::from_parts(uri_parts).map_err(|_| GatewayHandlerError::InvalidRequest)?;
        parts.headers.insert(HOST, self.host.clone());

        let mut response = self
            .client
            .request(Request::from_parts(parts, body))
            .await
            .map_err(|error| {
                if caused_by_length_limit(&error) {
                    GatewayHandlerError::RequestBodyTooLarge
                } else {
                    GatewayHandlerError::Unavailable
                }
            })?;
        let backend_upgrade = (response.status() == StatusCode::SWITCHING_PROTOCOLS)
            .then(|| upgrade::on(&mut response));
        let (parts, body) = response.into_parts();
        let response = Response::from_parts(parts, body);
        Ok(match backend_upgrade {
            Some(upgrade) => GatewayHandlerResponse::with_backend_upgrade(response, upgrade),
            None => GatewayHandlerResponse::new(response),
        })
    }
}

struct ServerState<H> {
    mode: GatewayServerMode,
    handler: Arc<H>,
    edge_security: Arc<GatewayServerEdgeSecurity>,
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
            edge_security: Arc::clone(&self.edge_security),
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
    config.validate()?;
    let handler = Arc::new(handler);
    let edge_security = Arc::new(config.edge_security.clone());
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
        let state = ServerState {
            mode: config.mode,
            handler: Arc::clone(&handler),
            edge_security: Arc::clone(&edge_security),
            max_request_body_bytes: config.max_request_body_bytes,
            request_header_timeout: config.request_header_timeout,
            response_header_timeout: config.response_header_timeout,
            max_connection_age: config.max_connection_age,
            shutdown: shutdown_rx.clone(),
        };
        tasks.spawn(serve_connection(stream, peer_addr, state, permit));
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
    state: ServerState<H>,
    permit: Arc<OwnedSemaphorePermit>,
) -> Result<Response<GatewayResponseBody>, Infallible>
where
    H: Fn(GatewayHandlerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GatewayHandlerResult> + Send + 'static,
{
    if request.uri().scheme().is_some() || request.uri().authority().is_some() {
        return Ok(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST_TARGET));
    }
    let raw_target = request
        .uri()
        .path_and_query()
        .map_or_else(|| request.uri().path(), |target| target.as_str());
    let Ok(target) = CanonicalRequestTarget::parse(raw_target) else {
        return Ok(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST_TARGET));
    };
    let Some(route) = classify_request_target(&target) else {
        return Ok(json_error(StatusCode::NOT_FOUND, ROUTE_UNAVAILABLE));
    };
    if !route_allowed(state.mode, route.plane) {
        return Ok(json_error(StatusCode::NOT_FOUND, ROUTE_UNAVAILABLE));
    }
    let headers = match gateway_http_headers(&request) {
        Some(headers) => headers,
        None => return Ok(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST)),
    };
    let peer_is_trusted_proxy = state
        .edge_security
        .trusted_proxies
        .contains(&peer_addr.ip());
    let client_ip =
        match derive_gateway_client_ip(peer_addr, &state.edge_security.trusted_proxies, &headers) {
            Ok(client_ip) => client_ip,
            Err(_) => return Ok(json_error(StatusCode::FORBIDDEN, EDGE_REQUEST_DENIED)),
        };
    if route.plane == GatewayHttpRoutePlane::ControlPlane {
        let browser_capable = browser_capable_request(&request);
        let browser = if browser_capable {
            match state.edge_security.browser.as_ref() {
                Some(browser) => Some(browser),
                None => return Ok(json_error(StatusCode::FORBIDDEN, EDGE_REQUEST_DENIED)),
            }
        } else {
            None
        };
        let expected_csrf_token = browser
            .filter(|_| state_changing_method(request.method()))
            .map(|browser| browser.expected_csrf_token.as_str());
        if validate_gateway_edge_security(
            GatewayEdgeSecurityPolicy {
                peer_is_trusted_proxy,
                expected_host: loopback_compatible_expected_host(
                    &state.edge_security.expected_host,
                    &headers,
                ),
                expected_origin: browser.map(|browser| browser.expected_origin.as_str()),
                expected_csrf_token,
            },
            &headers,
        )
        .is_err()
        {
            return Ok(json_error(StatusCode::FORBIDDEN, EDGE_REQUEST_DENIED));
        }
    }
    let route = route.kind;
    match content_length(&request) {
        Err(()) => return Ok(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST)),
        Ok(Some(length)) if length > state.max_request_body_bytes as u64 => {
            return Ok(json_error(StatusCode::PAYLOAD_TOO_LARGE, BODY_TOO_LARGE));
        }
        Ok(_) => {}
    }

    let frontend_upgrade = request
        .headers()
        .contains_key(hyper::header::UPGRADE)
        .then(|| upgrade::on(&mut request));
    let forwarding_headers = request
        .headers()
        .keys()
        .filter(|name| {
            name.as_str() == "forwarded"
                || name.as_str().starts_with("x-forwarded-")
                || name.as_str() == "x-real-ip"
        })
        .cloned()
        .collect::<Vec<_>>();
    for name in forwarding_headers {
        request.headers_mut().remove(name);
    }
    let (parts, body) = request.into_parts();
    let request = GatewayHandlerRequest {
        peer_addr,
        client_ip,
        peer_is_trusted_proxy,
        target,
        route,
        request: Request::from_parts(parts, Limited::new(body, state.max_request_body_bytes)),
    };
    let handled = match timeout(
        state.response_header_timeout,
        (state.handler.as_ref())(request),
    )
    .await
    {
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
                    permit,
                ));
            }
            GatewayHandlerUpgrade::InProcess(upgrade) => {
                tokio::spawn(tunnel_in_process_upgrade(
                    frontend_upgrade,
                    upgrade,
                    state.shutdown.clone(),
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
            json_error(StatusCode::BAD_GATEWAY, BACKEND_UNAVAILABLE)
        }
    }
}

fn route_allowed(mode: GatewayServerMode, plane: GatewayHttpRoutePlane) -> bool {
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
        _ = tokio::io::copy_bidirectional(&mut frontend, &mut backend) => {}
    }
}

async fn tunnel_in_process_upgrade(
    frontend: upgrade::OnUpgrade,
    handoff: GatewayInProcessUpgradeHandoff,
    mut shutdown: watch::Receiver<bool>,
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
