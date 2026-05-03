use super::*;

pub(crate) fn runtime_proxy_backend_is_websocket_upgrade(stream: &TcpStream) -> bool {
    let mut buffer = [0_u8; 2048];
    let Ok(read) = stream.peek(&mut buffer) else {
        return false;
    };
    if read == 0 {
        return false;
    }
    let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
    request.contains("upgrade: websocket")
}

#[allow(clippy::result_large_err)]
pub(crate) fn handle_runtime_proxy_backend_websocket(
    stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    websocket_requests: &Arc<Mutex<Vec<String>>>,
    mode: RuntimeProxyBackendMode,
) {
    let account_id = Arc::new(Mutex::new(String::new()));
    let request_turn_state = Arc::new(Mutex::new(None::<String>));
    let captured_headers = Arc::new(Mutex::new(BTreeMap::new()));
    let captured_account_id = Arc::clone(&account_id);
    let captured_turn_state = Arc::clone(&request_turn_state);
    let captured_headers_for_callback = Arc::clone(&captured_headers);
    let callback = move |req: &tungstenite::handshake::server::Request,
                         response: tungstenite::handshake::server::Response| {
        if let Some(value) = req
            .headers()
            .get("ChatGPT-Account-Id")
            .and_then(|value| value.to_str().ok())
        {
            *captured_account_id
                .lock()
                .expect("captured_account_id poisoned") = value.to_string();
        }
        if let Some(value) = req
            .headers()
            .get("x-codex-turn-state")
            .and_then(|value| value.to_str().ok())
        {
            *captured_turn_state
                .lock()
                .expect("captured_turn_state poisoned") = Some(value.to_string());
        }
        let mut headers = captured_headers_for_callback
            .lock()
            .expect("captured_headers poisoned");
        headers.clear();
        for (name, value) in req.headers() {
            if let Ok(value) = value.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), value.to_string());
            }
        }
        let mut response = response;
        if matches!(
            mode,
            RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit
        ) && req
            .headers()
            .get("ChatGPT-Account-Id")
            .and_then(|value| value.to_str().ok())
            == Some("second-account")
        {
            response.headers_mut().insert(
                tungstenite::http::header::HeaderName::from_static("x-codex-turn-state"),
                tungstenite::http::HeaderValue::from_static("turn-second"),
            );
        }
        Ok(response)
    };
    let mut websocket = tungstenite::accept_hdr(stream, callback)
        .expect("backend websocket handshake should succeed");
    let account_id = account_id.lock().expect("account_id poisoned").clone();
    let mut effective_turn_state = request_turn_state
        .lock()
        .expect("request_turn_state poisoned")
        .clone();
    responses_accounts
        .lock()
        .expect("responses_accounts poisoned")
        .push(account_id.clone());
    responses_headers
        .lock()
        .expect("responses_headers poisoned")
        .push(
            captured_headers
                .lock()
                .expect("captured_headers poisoned")
                .clone(),
        );
    let response_turn_state = (matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
    ) && account_id == "second-account")
        .then(|| "turn-second".to_string());
    let mut request_count = 0usize;

    loop {
        let request = match websocket.read() {
            Ok(WsMessage::Text(text)) => text.to_string(),
            Ok(WsMessage::Ping(payload)) => {
                websocket
                    .send(WsMessage::Pong(payload))
                    .expect("backend websocket pong should be sent");
                continue;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => continue,
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Ok(other) => panic!("backend websocket expects text requests, got {other:?}"),
            Err(err) => panic!("backend websocket failed to read request: {err}"),
        };
        websocket_requests
            .lock()
            .expect("websocket_requests poisoned")
            .push(request.clone());
        request_count += 1;
        let previous_response_id = runtime_request_previous_response_id_from_text(&request);

        if matches!(mode, RuntimeProxyBackendMode::WebsocketRealtimeSideband) {
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
            websocket
                .send(WsMessage::Text(payload.to_string().into()))
                .expect("realtime sideband response should be sent");
            continue;
        }

        match account_id.as_str() {
            "main-account" => {
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
                        | RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
                ) && request_count == 1
                {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": "resp-main"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response": {
                                    "id": "resp-main"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                    continue;
                }
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
                        | RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
                ) && request_count >= 2
                {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.in_progress"
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.in_progress should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.output_item.added",
                                "item": {
                                    "type": "message",
                                    "id": "msg-main"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.output_item.added should be sent");
                    let delayed_error = if matches!(
                        mode,
                        RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
                    ) {
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
                    websocket
                        .send(WsMessage::Text(delayed_error.to_string().into()))
                        .expect("delayed retryable error should be sent");
                    continue;
                }
                let immediate_error = if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketOverloaded
                ) {
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
                websocket
                    .send(WsMessage::Text(immediate_error.to_string().into()))
                    .expect("retryable error should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterPrelude
                ) && runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
                        .expect("next response id should exist");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": next_response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                thread::sleep(Duration::from_millis(50));
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.failed",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("precommit previous_response_not_found should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit
                ) && runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
                        .expect("next response id should exist");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": next_response_id.clone()
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.output_text.delta",
                            "response": {
                                "id": next_response_id
                            },
                            "delta": "hello"
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.output_text.delta should be sent");
                thread::sleep(Duration::from_millis(50));
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.failed",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("post-commit previous_response_not_found should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                        | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                        | RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState
                        | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                ) && runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) && effective_turn_state.as_deref() != Some("turn-second") =>
            {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("previous_response_not_found should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketOwnedToolOutputNeedsSessionReplay
                ) && runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) && request_body_contains_only_function_call_output(&request)
                    && request_body_contains_session_id(&request)
                    && effective_turn_state.is_none() =>
            {
                let (call_id, item_label) = serde_json::from_str::<serde_json::Value>(&request)
                    .ok()
                    .and_then(|body_json| {
                        body_json
                            .get("input")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|input| input.first())
                            .map(|item| {
                                let call_id = item
                                    .get("call_id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("call_missing");
                                let item_label = item
                                    .get("type")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("tool_call_output")
                                    .replace('_', " ");
                                (call_id.to_string(), item_label)
                            })
                    })
                    .unwrap_or_else(|| {
                        ("call_missing".to_string(), "tool call output".to_string())
                    });
                websocket
                    .send(WsMessage::Text(
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
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("tool context regression should be sent");
            }
            "second-account"
                if runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
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
                    break;
                }
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                ) && request_count == 2
                {
                    thread::sleep(Duration::from_millis(
                        runtime_proxy_websocket_precommit_progress_timeout_ms() + 100,
                    ));
                    break;
                }
                if matches!(mode, RuntimeProxyBackendMode::WebsocketTopLevelResponseId) {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.output_item.done",
                                "response_id": next_response_id.clone(),
                                "item": {
                                    "id": "item-second"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.output_item.done should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response_id": next_response_id
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                } else {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": next_response_id.clone()
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response": {
                                    "id": next_response_id
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                }
            }
            "second-account" if previous_response_id.is_some() => {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("previous_response_not_found should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState
                ) && previous_response_id.is_none()
                    && request_body_contains_only_function_call_output(&request)
                    && !request_body_contains_session_id(&request) =>
            {
                let (call_id, item_label) = serde_json::from_str::<serde_json::Value>(&request)
                    .ok()
                    .and_then(|body_json| {
                        body_json
                            .get("input")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|input| input.first())
                            .map(|item| {
                                let call_id = item
                                    .get("call_id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("call_missing");
                                let item_label = item
                                    .get("type")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("tool_call_output")
                                    .replace('_', " ");
                                (call_id.to_string(), item_label)
                            })
                    })
                    .unwrap_or_else(|| {
                        ("call_missing".to_string(), "tool call output".to_string())
                    });
                websocket
                    .send(WsMessage::Text(
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
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("tool output mismatch should be sent");
            }
            "second-account" => {
                let response_id =
                    runtime_proxy_backend_initial_response_id_for_account("second-account")
                        .expect("second-account response id should exist");
                if matches!(mode, RuntimeProxyBackendMode::WebsocketReuseSilentHang)
                    && request_count == 2
                {
                    thread::sleep(Duration::from_millis(
                        runtime_proxy_stream_idle_timeout_ms() + 100,
                    ));
                    break;
                }
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketReusePrecommitHoldStall
                ) && request_count == 2
                {
                    let hold_delay = Duration::from_millis(
                        runtime_proxy_websocket_precommit_progress_timeout_ms() / 2 + 10,
                    );
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": response_id
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    thread::sleep(hold_delay);
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.in_progress",
                                "response": {
                                    "id": response_id
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.in_progress should be sent");
                    thread::sleep(hold_delay);
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.output_item.added",
                                "response": {
                                    "id": response_id
                                },
                                "item": {
                                    "type": "message",
                                    "id": "msg-second"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.output_item.added should be sent");
                    continue;
                }
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                if matches!(mode, RuntimeProxyBackendMode::WebsocketCloseMidTurn) {
                    let _ = websocket.close(None);
                    break;
                }
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.completed should be sent");
            }
            "third-account"
                if runtime_proxy_backend_is_owned_continuation(
                    "third-account",
                    previous_response_id.as_deref(),
                ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
                        .expect("next response id should exist");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": next_response_id.clone()
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {
                                "id": next_response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.completed should be sent");
            }
            "third-account" if previous_response_id.is_some() => {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("previous_response_not_found should be sent");
            }
            "third-account" => {
                let response_id =
                    runtime_proxy_backend_initial_response_id_for_account("third-account")
                        .expect("third-account response id should exist");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.completed should be sent");
            }
            _ => {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 429,
                            "error": {
                                "code": "rate_limit_exceeded",
                                "message": "unexpected account",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("unexpected account error should be sent");
            }
        }

        if response_turn_state.is_some() {
            effective_turn_state = response_turn_state.clone();
        }
    }
}
