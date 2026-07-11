//! Gemini Live upstream connection and local websocket upgrade helpers.

use super::super::gemini_rewrite::RuntimeGeminiAuth;
use super::GEMINI_LIVE_WEBSOCKET_URL;
use crate::{
    RuntimeUpstreamWebSocket, TinyHeader, TinyResponse, TinyStatusCode, derive_accept_key,
    runtime_set_upstream_websocket_io_timeout,
};
use anyhow::{Context, Result};
use std::time::Duration;
use tungstenite::client::IntoClientRequest;

pub(super) fn runtime_gemini_live_connect(
    auth: &RuntimeGeminiAuth,
    configured_endpoint: Option<&str>,
) -> Result<RuntimeUpstreamWebSocket> {
    let endpoint = configured_endpoint.unwrap_or(GEMINI_LIVE_WEBSOCKET_URL);
    let url = match auth {
        RuntimeGeminiAuth::ApiKey { api_key } => {
            let separator = if endpoint.contains('?') { '&' } else { '?' };
            format!("{endpoint}{separator}key={api_key}")
        }
        RuntimeGeminiAuth::OAuth { .. } => endpoint.to_string(),
    };
    let mut request = url
        .into_client_request()
        .context("failed to build Gemini Live websocket request")?;
    if let RuntimeGeminiAuth::OAuth { access_token, .. } = auth {
        request.headers_mut().insert(
            tungstenite::http::header::AUTHORIZATION,
            format!("Bearer {access_token}")
                .parse()
                .context("failed to encode Gemini Live OAuth header")?,
        );
    }
    let (mut socket, _) =
        tungstenite::connect(request).context("failed to connect Gemini Live websocket")?;
    runtime_set_upstream_websocket_io_timeout(&mut socket, Some(Duration::from_secs(30)))
        .context("failed to configure Gemini Live websocket timeout")?;
    Ok(socket)
}

pub(super) fn runtime_gemini_live_websocket_key(request: &tiny_http::Request) -> Option<String> {
    request.headers().iter().find_map(|header| {
        header
            .field
            .equiv("Sec-WebSocket-Key")
            .then(|| header.value.as_str().trim().to_string())
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
