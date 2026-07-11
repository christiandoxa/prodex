use super::*;

pub(super) fn maybe_handle_realtime_sideband(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    mode: RuntimeProxyBackendMode,
    request_count: usize,
) -> bool {
    if !matches!(mode, RuntimeProxyBackendMode::WebsocketRealtimeSideband) {
        return false;
    }

    let payload = match request_count {
        1 => serde_json::json!({
            "type": "session.updated",
            "session": {
                "id": "sess-realtime",
                "instructions": "backend prompt"
            }
        }),
        2 => serde_json::json!({
            "type": "conversation.item.added",
            "item": {
                "id": "item-realtime"
            }
        }),
        3 => serde_json::json!({
            "type": "response.done",
            "response": {
                "id": "resp-realtime"
            }
        }),
        _ => serde_json::json!({
            "type": "error",
            "error": {
                "message": "unexpected realtime sideband request"
            }
        }),
    };
    events::send_backend_websocket_json(websocket, payload, "realtime sideband response should be sent");
    true
}
