use std::future::Future;

use anyhow::{Context as _, Result};
use hyper::{Request, Response, StatusCode, Uri, header::HOST, upgrade};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use tokio::net::TcpListener;

use super::{
    GatewayHandlerError, GatewayHandlerRequest, GatewayHandlerResponse, GatewayHandlerResult,
    GatewayRequestBody, GatewayServerConfig, GatewayServerReloadHandle,
    request::caused_by_length_limit, run_with_handler, run_with_handler_reloadable,
    shutdown_signal,
};

type ProxyClient = Client<HttpConnector, GatewayRequestBody>;

/// Runs the compatibility front until SIGINT or SIGTERM, then drains open connections.
pub fn serve(config: GatewayServerConfig, backend_addr: std::net::SocketAddr) -> Result<()> {
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

pub fn serve_with_handler_reloadable<H, Fut>(
    config: GatewayServerConfig,
    reload: GatewayServerReloadHandle,
    handler: H,
) -> Result<()>
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
        run_with_handler_reloadable(listener, config, reload, handler, shutdown_signal()).await
    })
}

#[derive(Clone)]
pub(super) struct LoopbackBackend {
    authority: hyper::http::uri::Authority,
    host: hyper::header::HeaderValue,
    client: ProxyClient,
}

impl LoopbackBackend {
    pub(super) fn new(backend_addr: std::net::SocketAddr) -> Result<Self> {
        let backend = backend_addr.to_string();
        Ok(Self {
            authority: backend
                .parse()
                .context("failed to prepare gateway backend authority")?,
            host: hyper::header::HeaderValue::from_str(&backend)
                .context("failed to prepare gateway backend host header")?,
            client: Client::builder(TokioExecutor::new()).build_http(),
        })
    }

    pub(super) async fn handle(&self, request: GatewayHandlerRequest) -> GatewayHandlerResult {
        let GatewayHandlerRequest {
            peer_addr: _,
            client_ip: _,
            peer_is_trusted_proxy: _,
            mtls_peer_certificate_sha256: _,
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
