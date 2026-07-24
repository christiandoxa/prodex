//! Gemini Live upstream connection and local websocket upgrade helpers.

use super::super::gemini_rewrite::RuntimeGeminiAuth;
use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_gateway_admission::RUNTIME_GATEWAY_REALTIME_FRAME_MAX_BYTES;
use super::super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::super::local_rewrite_transport::runtime_local_rewrite_with_projected_provider_secret;
use super::GEMINI_LIVE_WEBSOCKET_URL;
use crate::{
    RuntimeUpstreamWebSocket, TinyHeader, TinyResponse, TinyStatusCode, derive_accept_key,
    runtime_set_upstream_websocket_io_timeout,
};
use anyhow::{Context, Result};
use std::time::Duration;
use tungstenite::client::{IntoClientRequest, connect_with_config};

pub(super) fn runtime_gemini_live_websocket_config() -> tungstenite::protocol::WebSocketConfig {
    tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(RUNTIME_GATEWAY_REALTIME_FRAME_MAX_BYTES))
        .max_frame_size(Some(RUNTIME_GATEWAY_REALTIME_FRAME_MAX_BYTES))
}

pub(super) fn runtime_gemini_live_connect(
    auth: &RuntimeGeminiAuth,
    configured_endpoint: Option<&str>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeUpstreamWebSocket> {
    let endpoint = configured_endpoint.unwrap_or(GEMINI_LIVE_WEBSOCKET_URL);
    if matches!(auth, RuntimeGeminiAuth::Projected) {
        return runtime_local_rewrite_with_projected_provider_secret(
            shared.provider_credential.as_ref(),
            |api_key| runtime_gemini_live_connect_api_key(endpoint, api_key),
        );
    }
    let mut request = endpoint
        .into_client_request()
        .context("failed to build Gemini Live websocket request")?;
    match auth {
        RuntimeGeminiAuth::ApiKey { api_key } => {
            runtime_gemini_live_insert_api_key(&mut request, api_key)?;
        }
        RuntimeGeminiAuth::OAuth { access_token, .. } => {
            request.headers_mut().insert(
                tungstenite::http::header::AUTHORIZATION,
                format!("Bearer {access_token}")
                    .parse()
                    .context("failed to encode Gemini Live OAuth header")?,
            );
        }
        RuntimeGeminiAuth::Projected => unreachable!("projected auth is handled above"),
    }
    let (mut socket, _) =
        connect_with_config(request, Some(runtime_gemini_live_websocket_config()), 3)
            .context("failed to connect Gemini Live websocket")?;
    runtime_set_upstream_websocket_io_timeout(&mut socket, Some(Duration::from_secs(30)))
        .context("failed to configure Gemini Live websocket timeout")?;
    Ok(socket)
}

fn runtime_gemini_live_connect_api_key(
    endpoint: &str,
    api_key: &str,
) -> Result<RuntimeUpstreamWebSocket> {
    let mut request = endpoint
        .into_client_request()
        .context("failed to build Gemini Live websocket request")?;
    runtime_gemini_live_insert_api_key(&mut request, api_key)?;
    let (mut socket, _) =
        connect_with_config(request, Some(runtime_gemini_live_websocket_config()), 3)
            .context("failed to connect Gemini Live websocket")?;
    runtime_set_upstream_websocket_io_timeout(&mut socket, Some(Duration::from_secs(30)))
        .context("failed to configure Gemini Live websocket timeout")?;
    Ok(socket)
}

fn runtime_gemini_live_insert_api_key(
    request: &mut tungstenite::http::Request<()>,
    api_key: &str,
) -> Result<()> {
    request.headers_mut().insert(
        "x-goog-api-key",
        api_key
            .parse()
            .context("failed to encode Gemini Live API key header")?,
    );
    Ok(())
}

pub(super) fn runtime_gemini_live_websocket_key(
    request: &RuntimeLocalRewriteRequest,
) -> Option<String> {
    request.headers().iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("Sec-WebSocket-Key")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn runtime_gemini_live_upgrade_response(
    key: &str,
) -> Result<TinyResponse<std::io::Empty>> {
    let accept = derive_accept_key(key.as_bytes());
    let upgrade = TinyHeader::from_bytes("Upgrade", "websocket")
        .map_err(|err| anyhow::anyhow!("invalid websocket Upgrade header: {err:?}"))?;
    let connection = TinyHeader::from_bytes("Connection", "Upgrade")
        .map_err(|err| anyhow::anyhow!("invalid websocket Connection header: {err:?}"))?;
    let accept = TinyHeader::from_bytes("Sec-WebSocket-Accept", accept.as_bytes())
        .map_err(|err| anyhow::anyhow!("invalid websocket accept header: {err:?}"))?;
    Ok(TinyResponse::new_empty(TinyStatusCode(101))
        .with_header(upgrade)
        .with_header(connection)
        .with_header(accept))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_is_carried_in_header_not_websocket_target() {
        let endpoint = "wss://generativelanguage.googleapis.com/ws/live?mode=audio";
        let api_key = "gemini-live-query-secret-sentinel";
        let mut request = endpoint.into_client_request().unwrap();

        runtime_gemini_live_insert_api_key(&mut request, api_key).unwrap();

        assert_eq!(request.uri().to_string(), endpoint);
        assert!(!request.uri().to_string().contains(api_key));
        assert_eq!(request.headers()["x-goog-api-key"], api_key);
    }
}
