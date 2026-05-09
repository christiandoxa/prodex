use super::*;

pub(super) fn handle_runtime_proxy_backend_request(
    mut stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: &Arc<Mutex<Vec<String>>>,
    usage_accounts: &Arc<Mutex<Vec<String>>>,
    mode: RuntimeProxyBackendMode,
) {
    let request = match read_http_request(&mut stream) {
        Some(request) => request,
        None => return,
    };

    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let request_body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    let account_id = request_header(&request, "ChatGPT-Account-Id").unwrap_or_default();
    let turn_state = request_header(&request, "x-codex-turn-state");
    let captured_headers = request_headers_map(&request);

    if path.ends_with("/backend-api/codex/responses")
        && account_id == "main-account"
        && matches!(mode, RuntimeProxyBackendMode::HttpOnlyResetBeforeFirstByte)
    {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(captured_headers);
        responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .push(request_body);
        return;
    }

    let (status_line, content_type, body, response_turn_state, initial_body_stall, chunk_delay) =
        if path.ends_with("/backend-api/wham/usage") {
            usage_accounts
                .lock()
                .expect("usage_accounts poisoned")
                .push(account_id.clone());
            let body = match account_id.as_str() {
                "main-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("main@example.com", 0, 0)
                }
                "main-account" => runtime_proxy_usage_body("main@example.com"),
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("second@example.com", 0, 0)
                }
                "second-account" => runtime_proxy_usage_body("second@example.com"),
                "third-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("third@example.com", 0, 0)
                }
                "third-account" => runtime_proxy_usage_body("third@example.com"),
                "fourth-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("fourth@example.com", 0, 0)
                }
                "fourth-account" => runtime_proxy_usage_body("fourth@example.com"),
                "fifth-account" => runtime_proxy_usage_body("fifth@example.com"),
                _ => serde_json::json!({ "error": "unauthorized" }).to_string(),
            };
            let status = if matches!(
                account_id.as_str(),
                "main-account"
                    | "second-account"
                    | "third-account"
                    | "fourth-account"
                    | "fifth-account"
            ) {
                "HTTP/1.1 200 OK"
            } else {
                "HTTP/1.1 401 Unauthorized"
            };
            (status, "application/json", body, None, None, None)
        } else if path.ends_with("/backend-api/status") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            match (account_id.as_str(), mode) {
                (
                    "main-account",
                    RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                    | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
                ) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "type": "usage_limit_reached",
                            "message": "The usage limit has been reached",
                        },
                        "status": 429
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", RuntimeProxyBackendMode::HttpOnlyPlain429) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "text/plain",
                    "Too Many Requests".to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain) => (
                    "HTTP/1.1 401 Unauthorized",
                    "application/json",
                    serde_json::json!({
                        "error": "unauthorized"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "main-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("second-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "second-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("third-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "third-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                _ => (
                    "HTTP/1.1 401 Unauthorized",
                    "application/json",
                    serde_json::json!({ "error": "unauthorized" }).to_string(),
                    None,
                    None,
                    None,
                ),
            }
        } else if path.ends_with("/backend-api/codex/realtime/calls") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            responses_bodies
                .lock()
                .expect("responses_bodies poisoned")
                .push(request_body);
            match (account_id.as_str(), mode) {
                (
                    "main-account",
                    RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                    | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
                ) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "type": "usage_limit_reached",
                            "message": "The usage limit has been reached",
                        },
                        "status": 429
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "main-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("second-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "second-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("third-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "third-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                _ => (
                    "HTTP/1.1 401 Unauthorized",
                    "application/json",
                    serde_json::json!({ "error": "unauthorized" }).to_string(),
                    None,
                    None,
                    None,
                ),
            }
        } else if path.ends_with("/backend-api/codex/responses") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            responses_bodies
                .lock()
                .expect("responses_bodies poisoned")
                .push(request_body.clone());
            let previous_response_id = request_previous_response_id(&request);
            let body_json = serde_json::from_str::<serde_json::Value>(&request_body)
                .unwrap_or(serde_json::Value::Null);
            match account_id.as_str() {
                "main-account" if matches!(mode, RuntimeProxyBackendMode::HttpOnlyPlain429) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "text/plain",
                    "Too Many Requests".to_string(),
                    None,
                    None,
                    None,
                ),
                "main-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain) =>
                {
                    (
                        "HTTP/1.1 401 Unauthorized",
                        "application/json",
                        serde_json::json!({
                            "error": "unauthorized"
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "main-account" => {
                    let body = if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyDelayedQuotaAfterOutputItemAdded
                    ) {
                        concat!(
                            "event: response.created\r\n",
                            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-main\"}}\r\n",
                            "\r\n",
                            "event: response.in_progress\r\n",
                            "data: {\"type\":\"response.in_progress\",\"response\":{\"id\":\"resp-main\"}}\r\n",
                            "\r\n",
                            "event: response.output_item.added\r\n",
                            "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"msg-main\"}}\r\n",
                            "\r\n",
                            "event: response.failed\r\n",
                            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"You've hit your usage limit. To get more access now, send a request to your admin or try again at Mar 24th, 2026 2:04 AM.\"}}}\r\n",
                            "\r\n"
                        )
                        .to_string()
                    } else if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                            | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) {
                        concat!(
                            "event: response.failed\r\n",
                            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"You've hit your usage limit. To get more access now, send a request to your admin or try again at Mar 24th, 2026 2:04 AM.\"}}}\r\n",
                            "\r\n"
                        )
                        .to_string()
                    } else {
                        concat!(
                            "event: response.failed\r\n",
                            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"main quota exhausted\"}}}\r\n",
                            "\r\n"
                        )
                        .to_string()
                    };
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        body,
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(
                            mode,
                            RuntimeProxyBackendMode::HttpOnlySlowStream
                                | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                        )
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseNotFoundAfterCommit
                    ) && runtime_proxy_backend_is_owned_continuation(
                        "second-account",
                        previous_response_id.as_deref(),
                    ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n",
                                "event: response.output_text.delta\r\n",
                                "data: {{\"type\":\"response.output_text.delta\",\"response\":{{\"id\":\"{}\"}},\"delta\":\"hello\"}}\r\n",
                                "\r\n",
                                "event: response.failed\r\n",
                                "data: {{\"type\":\"response.failed\",\"response\":{{\"error\":{{\"code\":\"previous_response_not_found\",\"message\":\"Previous response with id '{}' not found.\",\"param\":\"previous_response_id\"}}}}}}\r\n",
                                "\r\n"
                            ),
                            next_response_id,
                            next_response_id,
                            previous_response_id.as_deref().unwrap_or_default(),
                        ),
                        Some("turn-second".to_string()),
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState
                            | RuntimeProxyBackendMode::HttpOnlySseHeadersArrayTurnState
                    ) && runtime_proxy_backend_is_owned_continuation(
                        "second-account",
                        previous_response_id.as_deref(),
                    )
                        && turn_state.as_deref() != Some("turn-second") =>
                {
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
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
                        .to_string(),
                        Some("turn-second".to_string()),
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlySseHeadersArrayTurnState)
                        && previous_response_id.is_none() =>
                {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account("second-account")
                            .expect("second-account response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\",\"headers\":[[\"x-codex-turn-state\",\"turn-second\"]]}}}}\r\n",
                                "\r\n",
                                "event: response.completed\r\n",
                                "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n"
                            ),
                            response_id,
                            response_id
                        )
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup
                    ) && body_json
                        .get("stream")
                        .and_then(serde_json::Value::as_bool)
                        != Some(true) =>
                {
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "error": {
                                "message": "{\"detail\":\"Stream must be set to true\"}"
                            }
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup
                    ) && previous_response_id.as_deref() == Some("resp_ws_followup_1") =>
                {
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": "resp_ws_followup_2",
                            "object": "response",
                            "status": "completed",
                            "usage": {
                                "input_tokens": 18,
                                "output_tokens": 9
                            },
                            "tool_usage": {
                                "web_search": {
                                    "num_requests": 1
                                }
                            },
                            "output": [
                                {
                                    "type": "message",
                                    "content": [
                                        {
                                            "type": "output_text",
                                            "text": "Ringkasan terbaru reksadana Indonesia."
                                        }
                                    ]
                                }
                            ]
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup
                    ) =>
                {
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": "resp_ws_followup_1",
                            "object": "response",
                            "status": "completed",
                            "usage": {
                                "input_tokens": 12,
                                "output_tokens": 6
                            },
                            "tool_usage": {
                                "web_search": {
                                    "num_requests": 1
                                }
                            },
                            "output": [
                                {
                                    "type": "web_search_call",
                                    "id": "ws_1",
                                    "status": "completed",
                                    "action": {
                                        "type": "search",
                                        "queries": ["berita terbaru reksadana Indonesia"],
                                        "sources": [
                                            {
                                                "type": "url",
                                                "url": "https://example.com/news",
                                                "title": "Example News"
                                            }
                                        ]
                                    }
                                }
                            ]
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyAnthropicMcpStream) =>
                {
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        concat!(
                            "event: response.created\r\n",
                            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_mcp_1\"}}\r\n",
                            "\r\n",
                            "event: response.output_item.done\r\n",
                            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"list_files\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/workspace\\\"}\",\"output\":\"README.md\\nsrc/main.rs\"}}\r\n",
                            "\r\n",
                            "event: response.completed\r\n",
                            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_mcp_1\",\"usage\":{\"input_tokens\":14,\"output_tokens\":8},\"output\":[{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"list_files\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/workspace\\\"}\",\"output\":\"README.md\\nsrc/main.rs\"}]}}\r\n",
                            "\r\n"
                        )
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseToolContextMissing
                    )
                        && previous_response_id.is_some()
                        && request_body_contains_only_function_call_output(&request_body)
                        && request_body_contains_session_id(&request_body) =>
                {
                    let (call_id, item_label) = body_json
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
                        .unwrap_or_else(|| {
                            ("call_missing".to_string(), "tool call output".to_string())
                        });
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
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
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson)
                        && runtime_proxy_backend_is_owned_continuation(
                            "second-account",
                            previous_response_id.as_deref(),
                        ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": next_response_id,
                            "object": "response",
                            "status": "completed",
                            "output": []
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if runtime_proxy_backend_is_owned_continuation(
                        "second-account",
                        previous_response_id.as_deref(),
                    ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n",
                                "event: response.completed\r\n",
                                "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n"
                            ),
                            next_response_id.clone(),
                            next_response_id
                        )
                        .to_string(),
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(
                            mode,
                            RuntimeProxyBackendMode::HttpOnlySlowStream
                                | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                        )
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyQuotaThenToolOutputFreshFallbackError
                    )
                        && previous_response_id.is_none()
                        && request_body_contains_only_function_call_output(&request_body)
                        && !request_body_contains_session_id(&request_body) =>
                {
                    let (call_id, item_label) = body_json
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
                        .unwrap_or_else(|| {
                            ("call_missing".to_string(), "tool call output".to_string())
                        });
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
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
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account" if previous_response_id.is_some() => (
                    "HTTP/1.1 400 Bad Request",
                    "application/json",
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
                    .to_string(),
                    None,
                    matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    None,
                ),
                "second-account" => {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account("second-account")
                            .expect("second-account response id should exist");
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson) {
                        (
                            "HTTP/1.1 200 OK",
                            "application/json",
                            serde_json::json!({
                                "id": response_id,
                                "object": "response",
                                "status": "completed",
                                "output": []
                            })
                            .to_string(),
                            None,
                            None,
                            None,
                        )
                    } else {
                        (
                            "HTTP/1.1 200 OK",
                            "text/event-stream",
                            format!(
                                concat!(
                                    "event: response.created\r\n",
                                    "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n",
                                    "event: response.completed\r\n",
                                    "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n"
                                ),
                                response_id,
                                response_id
                            )
                            .to_string(),
                            None,
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                                .then_some(Duration::from_millis(750)),
                            matches!(
                                mode,
                                RuntimeProxyBackendMode::HttpOnlySlowStream
                                    | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                            )
                                .then_some(Duration::from_millis(100)),
                        )
                    }
                }
                "third-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson)
                        && runtime_proxy_backend_is_owned_continuation(
                            "third-account",
                            previous_response_id.as_deref(),
                        ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": next_response_id,
                            "object": "response",
                            "status": "completed",
                            "output": []
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "third-account"
                    if runtime_proxy_backend_is_owned_continuation(
                        "third-account",
                        previous_response_id.as_deref(),
                    ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n",
                                "event: response.completed\r\n",
                                "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n"
                            ),
                            next_response_id.clone(),
                            next_response_id
                        )
                        .to_string(),
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "third-account" if previous_response_id.is_some() => (
                    "HTTP/1.1 400 Bad Request",
                    "application/json",
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
                    .to_string(),
                    None,
                    matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    None,
                ),
                "third-account" => {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account("third-account")
                            .expect("third-account response id should exist");
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson) {
                        (
                            "HTTP/1.1 200 OK",
                            "application/json",
                            serde_json::json!({
                                "id": response_id,
                                "object": "response",
                                "status": "completed",
                                "output": []
                            })
                            .to_string(),
                            None,
                            None,
                            None,
                        )
                    } else {
                        (
                            "HTTP/1.1 200 OK",
                            "text/event-stream",
                            format!(
                                concat!(
                                    "event: response.created\r\n",
                                    "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n",
                                    "event: response.completed\r\n",
                                    "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n"
                                ),
                                response_id,
                                response_id
                            )
                            .to_string(),
                            None,
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                                .then_some(Duration::from_millis(750)),
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                        )
                    }
                }
                "fourth-account" | "fifth-account" => {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account(account_id.as_str())
                            .expect("late-pool account response id should exist");
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson) {
                        (
                            "HTTP/1.1 200 OK",
                            "application/json",
                            serde_json::json!({
                                "id": response_id,
                                "object": "response",
                                "status": "completed",
                                "output": []
                            })
                            .to_string(),
                            None,
                            None,
                            None,
                        )
                    } else {
                        (
                            "HTTP/1.1 200 OK",
                            "text/event-stream",
                            format!(
                                concat!(
                                    "event: response.created\r\n",
                                    "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n",
                                    "event: response.completed\r\n",
                                    "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n"
                                ),
                                response_id,
                                response_id
                            ),
                            None,
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                                .then_some(Duration::from_millis(750)),
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                                .then_some(Duration::from_millis(100)),
                        )
                    }
                }
                _ => (
                    "HTTP/1.1 200 OK",
                    "text/event-stream",
                    concat!(
                        "event: response.failed\r\n",
                        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"unexpected account\"}}}\r\n",
                        "\r\n"
                        )
                        .to_string(),
                    None,
                    matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlySlowStream
                            | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                    )
                        .then_some(Duration::from_millis(100)),
                ),
            }
        } else if path.ends_with("/backend-api/codex/responses/compact") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            responses_bodies
                .lock()
                .expect("responses_bodies poisoned")
                .push(request_body);
            let previous_response_id = request_previous_response_id(&request);
            match (account_id.as_str(), mode) {
                (
                    "main-account",
                    RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                    | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
                ) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "type": "usage_limit_reached",
                            "message": "The usage limit has been reached",
                            "plan_type": "team",
                            "resets_at": 1775183113_i64,
                            "eligible_promo": serde_json::Value::Null,
                            "resets_in_seconds": 259149
                        }
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("second-account", RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some("compact-turn-second".to_string()),
                    None,
                    None,
                ),
                ("main-account", RuntimeProxyBackendMode::HttpOnlyCompactOverloaded) => (
                    "HTTP/1.1 500 Internal Server Error",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "message": "backend under high demand"
                        },
                        "status": 500
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                (_, RuntimeProxyBackendMode::HttpOnlyCompactPreviousResponseNotFound)
                    if previous_response_id.is_some() =>
                {
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "type": "invalid_request_error",
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                ("second-account", RuntimeProxyBackendMode::HttpOnlyCompactOverloaded) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some("compact-turn-second".to_string()),
                    None,
                    None,
                ),
                ("third-account", RuntimeProxyBackendMode::HttpOnlyCompactOverloaded) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some("compact-turn-third".to_string()),
                    None,
                    None,
                ),
                (_, RuntimeProxyBackendMode::HttpOnlyLargeCompactResponse) => {
                    let body = serde_json::json!({
                        "output": [
                            {
                                "type": "message",
                                "content": "x".repeat(RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES + 1024)
                            }
                        ]
                    })
                    .to_string();
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        body,
                        Some("compact-turn-main".to_string()),
                        None,
                        None,
                    )
                }
                _ => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some(
                        match account_id.as_str() {
                            "main-account" => "compact-turn-main",
                            "second-account" => "compact-turn-second",
                            "third-account" => "compact-turn-third",
                            _ => "compact-turn-unknown",
                        }
                        .to_string(),
                    ),
                    None,
                    None,
                ),
            }
        } else {
            (
                "HTTP/1.1 404 Not Found",
                "application/json",
                serde_json::json!({ "error": "not_found" }).to_string(),
                None,
                None,
                None,
            )
        };

    let mut headers = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len(),
    );
    if let Some(turn_state) = response_turn_state.as_deref() {
        headers.push_str(&format!("x-codex-turn-state: {turn_state}\r\n"));
    }
    headers.push_str("\r\n");
    let _ = stream.write_all(headers.as_bytes());
    let _ = stream.flush();
    if let Some(delay) = initial_body_stall {
        thread::sleep(delay);
    }
    if content_type == "text/event-stream"
        && matches!(mode, RuntimeProxyBackendMode::HttpOnlyResetAfterFirstChunk)
        && account_id == "second-account"
    {
        let reset_chunk = body
            .split("\r\n\r\n")
            .next()
            .map(|chunk| format!("{chunk}\r\n\r\n"))
            .unwrap_or_else(|| body.clone());
        let _ = stream.write_all(reset_chunk.as_bytes());
        let _ = stream.flush();
        return;
    }
    if content_type == "text/event-stream"
        && matches!(
            mode,
            RuntimeProxyBackendMode::HttpOnlyPreviousResponseNotFoundAfterCommit
        )
        && account_id == "second-account"
    {
        for (index, event) in body
            .split("\r\n\r\n")
            .filter(|event| !event.is_empty())
            .enumerate()
        {
            let _ = stream.write_all(format!("{event}\r\n\r\n").as_bytes());
            let _ = stream.flush();
            if index == 1 {
                thread::sleep(Duration::from_millis(50));
            }
        }
        return;
    }
    if content_type == "text/event-stream"
        && let Some(delay) = chunk_delay
    {
        let body_bytes = body.as_bytes();
        let chunk_size = body_bytes.len().max(1).div_ceil(4);
        for (index, chunk) in body_bytes.chunks(chunk_size).enumerate() {
            let _ = stream.write_all(chunk);
            let _ = stream.flush();
            if matches!(
                mode,
                RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
            ) && account_id == "second-account"
                && index >= 1
            {
                thread::sleep(Duration::from_millis(
                    runtime_proxy_stream_idle_timeout_ms() + 100,
                ));
                return;
            }
            if index + 1 < body_bytes.chunks(chunk_size).len() {
                thread::sleep(delay);
            }
        }
    } else {
        let _ = stream.write_all(body.as_bytes());
        let _ = stream.flush();
    }
}
