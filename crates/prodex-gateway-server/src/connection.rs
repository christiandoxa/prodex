use super::{GatewayHandlerRequest, GatewayHandlerResult, ServerState, handle_ingress_request};
use std::{future::Future, net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::{net::TcpStream, sync::OwnedSemaphorePermit};

pub(super) async fn serve_connection<H, Fut>(
    stream: TcpStream,
    peer_addr: SocketAddr,
    state: ServerState<H>,
    permit: OwnedSemaphorePermit,
) where
    H: Fn(GatewayHandlerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GatewayHandlerResult> + Send + 'static,
{
    let permit = Arc::new(permit);
    let mut shutdown = state.shutdown.clone();
    let request_header_timeout = state.request_header_timeout;
    let max_connection_age = state.max_connection_age;
    let service = service_fn(move |request| {
        handle_ingress_request(request, peer_addr, state.clone(), Arc::clone(&permit))
    });
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(request_header_timeout);
    let connection = builder
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades();
    tokio::pin!(connection);
    let connection_age = tokio::time::sleep(max_connection_age);
    tokio::pin!(connection_age);
    tokio::select! {
        _ = shutdown.changed() => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
        _ = &mut connection_age => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
        _ = connection.as_mut() => {}
    }
}
