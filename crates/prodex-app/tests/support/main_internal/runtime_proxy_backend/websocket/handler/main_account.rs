use super::*;

pub(super) fn handle_main_account(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    mode: RuntimeProxyBackendMode,
    request_count: usize,
) -> BackendWebsocketAction {
    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
            | RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
    ) && request_count == 1
    {
        events::send_response_created(websocket, "resp-main");
        events::send_response_completed(websocket, "resp-main");
        return BackendWebsocketAction::Continue;
    }

    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
            | RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
    ) && request_count >= 2
    {
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.in_progress"
            }),
            "response.in_progress should be sent",
        );
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "message",
                    "id": "msg-main"
                }
            }),
            "response.output_item.added should be sent",
        );
        let delayed_error =
            if matches!(mode, RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude) {
                serde_json::json!({
                    "type": "response.failed",
                    "status": 429,
                    "error": {
                        "message": "Selected model is at capacity. Please try a different model."
                    }
                })
            } else {
                serde_json::json!({
                    "type": "response.failed",
                    "status": 429,
                    "error": {
                        "type": "usage_limit_reached",
                        "message": "The usage limit has been reached"
                    }
                })
            };
        events::send_backend_websocket_json(
            websocket,
            delayed_error,
            "delayed retryable error should be sent",
        );
        return BackendWebsocketAction::Continue;
    }

    let immediate_error = if matches!(mode, RuntimeProxyBackendMode::WebsocketOverloaded) {
        serde_json::json!({
            "type": "response.failed",
            "status": 429,
            "error": {
                "message": "Selected model is at capacity. Please try a different model."
            }
        })
    } else {
        serde_json::json!({
            "type": "error",
            "status": 429,
            "error": {
                "code": "insufficient_quota",
                "message": "main quota exhausted",
            }
        })
    };
    events::send_backend_websocket_json(websocket, immediate_error, "retryable error should be sent");
    BackendWebsocketAction::Continue
}
