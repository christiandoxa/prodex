use super::*;

pub(super) fn handle_second_account(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    mode: RuntimeProxyBackendMode,
    request: &str,
    previous_response_id: Option<&str>,
    request_count: usize,
    effective_turn_state: Option<&str>,
) -> BackendWebsocketAction {
    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterPrelude
    ) && runtime_proxy_backend_is_owned_continuation("second-account", previous_response_id)
    {
        let next_response_id = runtime_proxy_backend_next_response_id(previous_response_id)
            .expect("next response id should exist");
        events::send_response_created(websocket, &next_response_id);
        thread::sleep(Duration::from_millis(50));
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.failed",
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
            "precommit previous_response_not_found should be sent",
        );
        return BackendWebsocketAction::Continue;
    }

    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit
    ) && runtime_proxy_backend_is_owned_continuation("second-account", previous_response_id)
    {
        let next_response_id = runtime_proxy_backend_next_response_id(previous_response_id)
            .expect("next response id should exist");
        events::send_response_created(websocket, &next_response_id);
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.output_text.delta",
                "response": {
                    "id": next_response_id
                },
                "delta": "hello"
            }),
            "response.output_text.delta should be sent",
        );
        thread::sleep(Duration::from_millis(50));
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.failed",
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
            "post-commit previous_response_not_found should be sent",
        );
        return BackendWebsocketAction::Continue;
    }

    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState
            | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
    ) && runtime_proxy_backend_is_owned_continuation("second-account", previous_response_id)
        && effective_turn_state != Some("turn-second")
    {
        events::send_previous_response_not_found(websocket, previous_response_id);
        return BackendWebsocketAction::Continue;
    }

    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketOwnedToolOutputNeedsSessionReplay
    ) && runtime_proxy_backend_is_owned_continuation("second-account", previous_response_id)
        && request_body_contains_only_function_call_output(request)
        && request_body_contains_session_id(request)
        && effective_turn_state.is_none()
    {
        events::send_tool_context_error(
            websocket,
            request,
            "tool context regression should be sent",
        );
        return BackendWebsocketAction::Continue;
    }

    if runtime_proxy_backend_is_owned_continuation("second-account", previous_response_id) {
        return handle_owned_second_account_continuation(
            websocket,
            mode,
            previous_response_id,
            request_count,
        );
    }

    if previous_response_id.is_some() {
        events::send_previous_response_not_found(websocket, previous_response_id);
        return BackendWebsocketAction::Continue;
    }

    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState
    ) && request_body_contains_only_function_call_output(request)
        && !request_body_contains_session_id(request)
    {
        events::send_tool_context_error(websocket, request, "tool output mismatch should be sent");
        return BackendWebsocketAction::Continue;
    }

    handle_fresh_second_account(websocket, mode, request_count)
}

fn handle_owned_second_account_continuation(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    mode: RuntimeProxyBackendMode,
    previous_response_id: Option<&str>,
    request_count: usize,
) -> BackendWebsocketAction {
    let next_response_id = runtime_proxy_backend_next_response_id(previous_response_id)
        .expect("next response id should exist");
    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketReuseOwnedPreviousResponseSilentHang
            | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
    ) && request_count == 2
    {
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 100,
        ));
        return BackendWebsocketAction::Break;
    }
    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
    ) && request_count == 2
    {
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 100,
        ));
        return BackendWebsocketAction::Break;
    }
    if matches!(mode, RuntimeProxyBackendMode::WebsocketTopLevelResponseId) {
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.output_item.done",
                "response_id": next_response_id.clone(),
                "item": {
                    "id": "item-second"
                }
            }),
            "response.output_item.done should be sent",
        );
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.completed",
                "response_id": next_response_id
            }),
            "response.completed should be sent",
        );
    } else {
        if matches!(mode, RuntimeProxyBackendMode::WebsocketKeepaliveBeforeContent) {
            events::send_keepalive_before_content(websocket);
        }
        events::send_response_created(websocket, &next_response_id);
        events::send_response_completed(websocket, &next_response_id);
    }
    BackendWebsocketAction::Continue
}

fn handle_fresh_second_account(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    mode: RuntimeProxyBackendMode,
    request_count: usize,
) -> BackendWebsocketAction {
    let response_id = runtime_proxy_backend_initial_response_id_for_account("second-account")
        .expect("second-account response id should exist");
    if matches!(mode, RuntimeProxyBackendMode::WebsocketReuseSilentHang) && request_count == 2 {
        thread::sleep(Duration::from_millis(
            runtime_proxy_stream_idle_timeout_ms() + 100,
        ));
        return BackendWebsocketAction::Break;
    }
    if matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketReusePrecommitHoldStall
    ) && request_count == 2
    {
        let hold_delay =
            Duration::from_millis(runtime_proxy_websocket_precommit_progress_timeout_ms() / 2 + 10);
        events::send_response_created(websocket, response_id);
        thread::sleep(hold_delay);
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.in_progress",
                "response": {
                    "id": response_id
                }
            }),
            "response.in_progress should be sent",
        );
        thread::sleep(hold_delay);
        events::send_backend_websocket_json(
            websocket,
            serde_json::json!({
                "type": "response.output_item.added",
                "response": {
                    "id": response_id
                },
                "item": {
                    "type": "message",
                    "id": "msg-second"
                }
            }),
            "response.output_item.added should be sent",
        );
        return BackendWebsocketAction::Continue;
    }
    if matches!(mode, RuntimeProxyBackendMode::WebsocketKeepaliveBeforeContent) {
        events::send_keepalive_before_content(websocket);
    }
    events::send_response_created(websocket, response_id);
    if matches!(mode, RuntimeProxyBackendMode::WebsocketCloseMidTurn) {
        let _ = websocket.close(None);
        return BackendWebsocketAction::Break;
    }
    events::send_response_completed(websocket, response_id);
    BackendWebsocketAction::Continue
}
