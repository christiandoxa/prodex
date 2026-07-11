use super::*;

pub(super) fn send_backend_websocket_json(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    payload: serde_json::Value,
    expect_message: &str,
) {
    websocket
        .send(WsMessage::Text(payload.to_string().into()))
        .expect(expect_message);
}

pub(super) fn send_keepalive_before_content(websocket: &mut tungstenite::WebSocket<TcpStream>) {
    websocket
        .send(WsMessage::Ping("upstream-ping-before-content".into()))
        .expect("upstream websocket ping should be sent");
    websocket
        .send(WsMessage::Pong("upstream-pong-before-content".into()))
        .expect("upstream websocket pong should be sent");
}

pub(super) fn send_response_created(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    response_id: &str,
) {
    send_backend_websocket_json(
        websocket,
        serde_json::json!({
            "type": "response.created",
            "response": {
                "id": response_id
            }
        }),
        "response.created should be sent",
    );
}

pub(super) fn send_response_completed(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    response_id: &str,
) {
    send_backend_websocket_json(
        websocket,
        serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": response_id
            }
        }),
        "response.completed should be sent",
    );
}

pub(super) fn send_previous_response_not_found(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    previous_response_id: Option<&str>,
) {
    send_backend_websocket_json(
        websocket,
        serde_json::json!({
            "type": "error",
            "status": 400,
            "error": {
                "code": "previous_response_not_found",
                "message": format!(
                    "Previous response with id '{}' not found.",
                    previous_response_id.unwrap_or_default()
                ),
                "param": "previous_response_id",
            }
        }),
        "previous_response_not_found should be sent",
    );
}

pub(super) fn send_tool_context_error(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    request: &str,
    expect_message: &str,
) {
    let (call_id, item_label) = super::super::tool::request_function_call_output_label(request);
    send_backend_websocket_json(
        websocket,
        serde_json::json!({
            "type": "error",
            "status": 400,
            "error": {
                "type": "invalid_request_error",
                "message": format!(
                    "No tool call found for {item_label} with call_id {call_id}."
                ),
                "param": "input",
            }
        }),
        expect_message,
    );
}
