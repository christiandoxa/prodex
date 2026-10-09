use super::*;

enum RuntimeWebsocketSessionReadAction {
    Continue,
    Break,
    Return(Result<()>),
}

pub(crate) fn run_runtime_proxy_websocket_session(
    session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    realtime_duplex: bool,
) -> Result<()> {
    let mut websocket_session = RuntimeWebsocketSessionState::with_realtime_duplex(realtime_duplex);
    loop {
        match handle_runtime_proxy_websocket_read(
            session_id,
            local_socket,
            handshake_request,
            shared,
            &mut websocket_session,
        )? {
            RuntimeWebsocketSessionReadAction::Continue => {}
            RuntimeWebsocketSessionReadAction::Break => break,
            RuntimeWebsocketSessionReadAction::Return(result) => return result,
        }
    }

    Ok(())
}

fn handle_runtime_proxy_websocket_read(
    session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<RuntimeWebsocketSessionReadAction> {
    match local_socket.read() {
        Ok(WsMessage::Text(text)) => handle_runtime_proxy_websocket_text_message(
            session_id,
            local_socket,
            handshake_request,
            shared,
            websocket_session,
            text.as_ref(),
        ),
        Ok(WsMessage::Binary(_)) => {
            runtime_proxy_log(
                shared,
                format!("websocket_session={session_id} inbound_binary_rejected"),
            );
            send_runtime_proxy_websocket_error(
                local_socket,
                400,
                "invalid_request_error",
                "Binary websocket messages are not supported by the runtime auto-rotate proxy.",
            )?;
            match runtime_websocket_read_action(WebsocketReadFrame::Binary, false, false)? {
                WebsocketReadAction::Continue => Ok(RuntimeWebsocketSessionReadAction::Continue),
                action => Err(anyhow::anyhow!(
                    "unexpected Mojo websocket binary-frame action: {action:?}"
                )),
            }
        }
        Ok(WsMessage::Ping(payload)) => {
            local_socket
                .send(WsMessage::Pong(payload))
                .context("failed to respond to runtime websocket ping")?;
            match runtime_websocket_read_action(WebsocketReadFrame::Ping, false, false)? {
                WebsocketReadAction::Continue => Ok(RuntimeWebsocketSessionReadAction::Continue),
                action => Err(anyhow::anyhow!(
                    "unexpected Mojo websocket ping-frame action: {action:?}"
                )),
            }
        }
        Ok(WsMessage::Pong(_)) => {
            match runtime_websocket_read_action(WebsocketReadFrame::Pong, false, false)? {
                WebsocketReadAction::Continue => Ok(RuntimeWebsocketSessionReadAction::Continue),
                action => Err(anyhow::anyhow!(
                    "unexpected Mojo websocket pong-frame action: {action:?}"
                )),
            }
        }
        Ok(WsMessage::Frame(_)) => {
            match runtime_websocket_read_action(WebsocketReadFrame::Raw, false, false)? {
                WebsocketReadAction::Continue => Ok(RuntimeWebsocketSessionReadAction::Continue),
                action => Err(anyhow::anyhow!(
                    "unexpected Mojo websocket raw-frame action: {action:?}"
                )),
            }
        }
        Ok(WsMessage::Close(frame)) => {
            let action = runtime_websocket_read_action(WebsocketReadFrame::Close, false, false)?;
            runtime_proxy_log(
                shared,
                format!("websocket_session={session_id} local_close"),
            );
            websocket_session.close();
            let _ = local_socket.close(frame);
            match action {
                WebsocketReadAction::Break => Ok(RuntimeWebsocketSessionReadAction::Break),
                action => Err(anyhow::anyhow!(
                    "unexpected Mojo websocket close-frame action: {action:?}"
                )),
            }
        }
        Err(err) => {
            handle_runtime_proxy_websocket_read_error(session_id, shared, websocket_session, &err)
        }
    }
}

fn handle_runtime_proxy_websocket_text_message(
    session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
    request_text: &str,
) -> Result<RuntimeWebsocketSessionReadAction> {
    let message_id = runtime_proxy_next_request_id(shared);
    let request_metadata = parse_runtime_websocket_request_metadata(request_text);
    runtime_proxy_log(
        shared,
        format!(
            "request={message_id} websocket_session={session_id} inbound_text previous_response_id={:?} turn_state={:?} bytes={}",
            request_metadata.previous_response_id,
            request_metadata
                .turn_state
                .clone()
                .or_else(|| runtime_request_turn_state(handshake_request)),
            request_text.len()
        ),
    );
    let compat_surface =
        runtime_detect_websocket_message_compatibility_surface(handshake_request, request_text);
    runtime_proxy_log_request_compatibility(shared, message_id, &compat_surface);
    proxy_runtime_websocket_text_message(RuntimeWebsocketTextMessageInput {
        session_id,
        request_id: message_id,
        local_socket,
        handshake_request,
        request_text,
        request_metadata: &request_metadata,
        shared,
        websocket_session,
    })?;
    match runtime_websocket_read_action(
        WebsocketReadFrame::Text,
        websocket_session.is_realtime_duplex(),
        websocket_session.has_socket(),
    )? {
        WebsocketReadAction::Continue => Ok(RuntimeWebsocketSessionReadAction::Continue),
        WebsocketReadAction::Return => {
            let result = run_runtime_realtime_websocket_duplex_session(
                session_id,
                local_socket,
                handshake_request,
                shared,
                websocket_session,
            );
            websocket_session.reset();
            Ok(RuntimeWebsocketSessionReadAction::Return(result))
        }
        WebsocketReadAction::Break | WebsocketReadAction::Error => Err(anyhow::anyhow!(
            "unexpected Mojo websocket text-frame action"
        )),
    }
}

fn handle_runtime_proxy_websocket_read_error(
    session_id: u64,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
    error: &WsError,
) -> Result<RuntimeWebsocketSessionReadAction> {
    let action = runtime_websocket_read_action(
        WebsocketReadFrame::ReadError {
            local_disconnect: runtime_websocket_local_disconnect_error(error),
        },
        false,
        false,
    )?;
    if action == WebsocketReadAction::Break {
        runtime_proxy_log(
            shared,
            format!("websocket_session={session_id} local_connection_closed"),
        );
        websocket_session.close();
        return Ok(RuntimeWebsocketSessionReadAction::Break);
    }
    if action != WebsocketReadAction::Error {
        return Err(anyhow::anyhow!(
            "unexpected Mojo websocket read-error action: {action:?}"
        ));
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "local_read_error",
            [
                runtime_proxy_log_field("websocket_session", session_id.to_string()),
                runtime_proxy_log_field(
                    "error",
                    runtime_websocket_error_log_value(&error.to_string()),
                ),
            ],
        ),
    );
    websocket_session.close();
    Err(anyhow::anyhow!(
        "runtime websocket session ended unexpectedly: {error}"
    ))
}
