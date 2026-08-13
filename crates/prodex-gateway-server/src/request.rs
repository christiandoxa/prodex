use std::{
    convert::Infallible, error::Error, future::Future, net::SocketAddr, sync::Arc, time::Duration,
};

use bytes::Bytes;
use http_body_util::Limited;
use hyper::{Request, Response, StatusCode, body::Incoming, upgrade};
use hyper_util::rt::TokioIo;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    sync::{OwnedSemaphorePermit, watch},
    time::timeout,
};

use super::{
    GatewayHandlerError, GatewayHandlerRequest, GatewayHandlerResponse, GatewayHandlerResult,
    GatewayHandlerUpgrade, GatewayInProcessUpgradeHandoff, GatewayResponseBody, ServerState,
    ingress::{
        BACKEND_TIMEOUT, BODY_TOO_LARGE, INVALID_REQUEST, INVALID_REQUEST_TARGET, LOCAL_OVERLOAD,
        SERVICE_UNAVAILABLE, json_error, parse_ingress_request, strip_forwarding_headers,
        validate_ingress_security, validate_request_size,
    },
};

pub(super) async fn handle_ingress_request<H, Fut>(
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

pub(super) fn caused_by_length_limit(error: &(dyn Error + 'static)) -> bool {
    let mut source = Some(error);
    while let Some(error) = source {
        if error.is::<http_body_util::LengthLimitError>() {
            return true;
        }
        source = error.source();
    }
    false
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
