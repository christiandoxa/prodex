use super::*;

pub(super) fn handle_backend_websocket_request(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    account_id: &str,
    mode: RuntimeProxyBackendMode,
    request: &str,
    previous_response_id: Option<&str>,
    request_count: usize,
    effective_turn_state: Option<&str>,
) -> BackendWebsocketAction {
    match account_id {
        "main-account" => main_account::handle_main_account(websocket, mode, request_count),
        "second-account" => second_account::handle_second_account(
            websocket,
            mode,
            request,
            previous_response_id,
            request_count,
            effective_turn_state,
        ),
        "third-account" => third_account::handle_third_account(websocket, previous_response_id),
        _ => {
            events::send_backend_websocket_json(
                websocket,
                serde_json::json!({
                    "type": "error",
                    "status": 429,
                    "error": {
                        "code": "rate_limit_exceeded",
                        "message": "unexpected account",
                    }
                }),
                "unexpected account error should be sent",
            );
            BackendWebsocketAction::Continue
        }
    }
}
