#![forbid(unsafe_code)]
//! Async HTTP/1 compatibility front for a loopback Prodex gateway backend.

use std::{
    convert::Infallible, error::Error, future::Future, net::SocketAddr, sync::Arc, time::Duration,
};

use anyhow::{Context as _, Result, ensure};
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full, Limited, combinators::UnsyncBoxBody};
use hyper::{
    Request, Response, StatusCode, Uri,
    body::Incoming,
    header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderValue},
    server::conn::http1,
    service::service_fn,
    upgrade,
};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::{TokioExecutor, TokioIo},
};
use prodex_gateway_http::{
    CanonicalRequestTarget, GatewayHttpPolicy, GatewayHttpRoutePlane, classify_request_target,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore, watch},
    task::JoinSet,
    time::{Instant, timeout, timeout_at},
};

type BoxError = Box<dyn Error + Send + Sync>;
type ProxyClient = Client<HttpConnector, Limited<Incoming>>;
type ProxyBody = UnsyncBoxBody<Bytes, BoxError>;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayServerMode {
    DataPlane,
    ControlPlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayServerConfig {
    pub listen_addr: SocketAddr,
    pub mode: GatewayServerMode,
    pub max_connections: usize,
    pub max_request_body_bytes: usize,
    pub response_header_timeout: Duration,
    pub drain_timeout: Duration,
}

impl GatewayServerConfig {
    pub fn production(listen_addr: SocketAddr, mode: GatewayServerMode) -> Self {
        let policy = GatewayHttpPolicy::production_default();
        Self {
            listen_addr,
            mode,
            max_connections: policy.max_concurrent_streams as usize,
            max_request_body_bytes: policy.max_body_bytes,
            response_header_timeout: Duration::from_millis(policy.request_timeout_ms),
            drain_timeout: Duration::from_millis(policy.connection_drain_timeout_ms),
        }
    }

    fn validate(self) -> Result<()> {
        ensure!(
            self.max_connections > 0 && self.max_connections <= u32::MAX as usize,
            "gateway max_connections must be between 1 and u32::MAX"
        );
        ensure!(
            self.max_request_body_bytes > 0,
            "gateway max_request_body_bytes must be non-zero"
        );
        ensure!(
            !self.response_header_timeout.is_zero(),
            "gateway response_header_timeout must be non-zero"
        );
        ensure!(
            !self.drain_timeout.is_zero(),
            "gateway drain_timeout must be non-zero"
        );
        Ok(())
    }
}

/// Runs the compatibility front until SIGINT or SIGTERM, then drains open connections.
pub fn serve(config: GatewayServerConfig, backend_addr: SocketAddr) -> Result<()> {
    config.validate()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to initialize gateway server runtime")?;
    runtime.block_on(async move {
        let listener = TcpListener::bind(config.listen_addr)
            .await
            .context("failed to bind gateway server listener")?;
        run(listener, config, backend_addr, shutdown_signal()).await
    })
}

#[derive(Clone)]
struct ProxyState {
    mode: GatewayServerMode,
    backend_authority: hyper::http::uri::Authority,
    backend_host: HeaderValue,
    client: ProxyClient,
    max_request_body_bytes: usize,
    response_header_timeout: Duration,
    shutdown: watch::Receiver<bool>,
}

async fn run<F>(
    listener: TcpListener,
    config: GatewayServerConfig,
    backend_addr: SocketAddr,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = Result<()>>,
{
    config.validate()?;
    let backend = backend_addr.to_string();
    let backend_authority: hyper::http::uri::Authority = backend
        .parse()
        .context("failed to prepare gateway backend authority")?;
    let backend_host =
        HeaderValue::from_str(&backend).context("failed to prepare gateway backend host header")?;
    let client = Client::builder(TokioExecutor::new()).build_http();
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
        let (stream, _) = match accepted {
            Ok(accepted) => accepted,
            Err(_) => {
                drop(permit);
                tokio::select! {
                    result = shutdown.as_mut() => break result,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
                }
            }
        };
        let state = ProxyState {
            mode: config.mode,
            backend_authority: backend_authority.clone(),
            backend_host: backend_host.clone(),
            client: client.clone(),
            max_request_body_bytes: config.max_request_body_bytes,
            response_header_timeout: config.response_header_timeout,
            shutdown: shutdown_rx.clone(),
        };
        tasks.spawn(serve_connection(stream, state, permit));
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

async fn serve_connection(stream: TcpStream, state: ProxyState, permit: OwnedSemaphorePermit) {
    let permit = Arc::new(permit);
    let mut shutdown = state.shutdown.clone();
    let service =
        service_fn(move |request| proxy_request(request, state.clone(), Arc::clone(&permit)));
    let connection = http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades();
    tokio::pin!(connection);
    tokio::select! {
        _ = shutdown.changed() => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
        _ = connection.as_mut() => {}
    }
}

async fn proxy_request(
    mut request: Request<Incoming>,
    state: ProxyState,
    permit: Arc<OwnedSemaphorePermit>,
) -> Result<Response<ProxyBody>, Infallible> {
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
    let (mut parts, body) = request.into_parts();
    let mut uri_parts = parts.uri.into_parts();
    uri_parts.scheme = Some(hyper::http::uri::Scheme::HTTP);
    uri_parts.authority = Some(state.backend_authority.clone());
    let Ok(path_and_query) = target.path_and_query().parse() else {
        return Ok(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST_TARGET));
    };
    uri_parts.path_and_query = Some(path_and_query);
    let Ok(uri) = Uri::from_parts(uri_parts) else {
        return Ok(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST));
    };
    parts.uri = uri;
    parts.headers.insert(HOST, state.backend_host.clone());
    let request = Request::from_parts(parts, Limited::new(body, state.max_request_body_bytes));

    let mut response =
        match timeout(state.response_header_timeout, state.client.request(request)).await {
            Err(_) => return Ok(json_error(StatusCode::GATEWAY_TIMEOUT, BACKEND_TIMEOUT)),
            Ok(Err(error)) if caused_by_length_limit(&error) => {
                return Ok(json_error(StatusCode::PAYLOAD_TOO_LARGE, BODY_TOO_LARGE));
            }
            Ok(Err(_)) => {
                return Ok(json_error(StatusCode::BAD_GATEWAY, BACKEND_UNAVAILABLE));
            }
            Ok(Ok(response)) => response,
        };

    if response.status() == StatusCode::SWITCHING_PROTOCOLS
        && let Some(frontend_upgrade) = frontend_upgrade
    {
        let backend_upgrade = upgrade::on(&mut response);
        tokio::spawn(tunnel_upgrades(
            frontend_upgrade,
            backend_upgrade,
            state.shutdown.clone(),
            permit,
        ));
    }
    let (parts, body) = response.into_parts();
    Ok(Response::from_parts(
        parts,
        body.map_err(|error| Box::new(error) as BoxError)
            .boxed_unsync(),
    ))
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

fn json_error(status: StatusCode, body: &'static [u8]) -> Response<ProxyBody> {
    let mut response = Response::new(
        Full::new(Bytes::from_static(body))
            .map_err(|error: Infallible| -> BoxError { match error {} })
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
