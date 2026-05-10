use super::*;

pub(super) fn handle_runtime_proxy_backend_responses_route(
    account_id: &str,
    request: &str,
    request_body: &str,
    turn_state: Option<&str>,
    mode: RuntimeProxyBackendMode,
) -> RuntimeProxyBackendHttpResponse {
    let previous_response_id = request_previous_response_id(request);
    let body_json =
        serde_json::from_str::<serde_json::Value>(request_body).unwrap_or(serde_json::Value::Null);
    let (status_line, content_type, body, response_turn_state, initial_body_stall, chunk_delay) =
        match account_id {
            "main-account" if matches!(mode, RuntimeProxyBackendMode::HttpOnlyPlain429) => (
                "HTTP/1.1 429 Too Many Requests",
                "text/plain",
                "Too Many Requests".to_string(),
                None,
                None,
                None,
            ),
            "main-account" if matches!(mode, RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain) => (
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
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
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
                ) && turn_state != Some("turn-second") =>
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
                let response_id = runtime_proxy_backend_initial_response_id_for_account(
                    "second-account",
                )
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
                        response_id, response_id
                    ),
                    None,
                    None,
                    None,
                )
            }
            "second-account"
                if matches!(mode, RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup)
                    && body_json
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
                if matches!(mode, RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup)
                    && previous_response_id.as_deref() == Some("resp_ws_followup_1") =>
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
                if matches!(mode, RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup) =>
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
            "second-account" if matches!(mode, RuntimeProxyBackendMode::HttpOnlyAnthropicMcpStream) => {
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
                ) && previous_response_id.is_some()
                    && request_body_contains_only_function_call_output(request_body)
                    && request_body_contains_session_id(request_body) =>
            {
                previous_response_tool_context_missing_response(&body_json)
            }
            "second-account"
                if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson)
                    && runtime_proxy_backend_is_owned_continuation(
                        "second-account",
                        previous_response_id.as_deref(),
                    ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
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
                owned_continuation_sse_response("second-account", previous_response_id.as_deref(), mode)
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::HttpOnlyQuotaThenToolOutputFreshFallbackError
                ) && previous_response_id.is_none()
                    && request_body_contains_only_function_call_output(request_body)
                    && !request_body_contains_session_id(request_body) =>
            {
                previous_response_tool_context_missing_response(&body_json)
            }
            "second-account" if previous_response_id.is_some() => previous_response_not_found_response(
                previous_response_id.as_deref(),
                matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                    .then_some(Duration::from_millis(750)),
            ),
            "second-account" => initial_account_response("second-account", mode),
            "third-account"
                if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson)
                    && runtime_proxy_backend_is_owned_continuation(
                        "third-account",
                        previous_response_id.as_deref(),
                    ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
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
                owned_continuation_sse_response("third-account", previous_response_id.as_deref(), mode)
            }
            "third-account" if previous_response_id.is_some() => previous_response_not_found_response(
                previous_response_id.as_deref(),
                matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                    .then_some(Duration::from_millis(750)),
            ),
            "third-account" => initial_account_response("third-account", mode),
            "fourth-account" | "fifth-account" => initial_account_response(account_id, mode),
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
        };
    RuntimeProxyBackendHttpResponse::new(
        status_line,
        content_type,
        body,
        response_turn_state,
        initial_body_stall,
        chunk_delay,
    )
}

fn previous_response_tool_context_missing_response(
    body_json: &serde_json::Value,
) -> (
    &'static str,
    &'static str,
    String,
    Option<String>,
    Option<Duration>,
    Option<Duration>,
) {
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
        .unwrap_or_else(|| ("call_missing".to_string(), "tool call output".to_string()));
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

fn previous_response_not_found_response(
    previous_response_id: Option<&str>,
    initial_body_stall: Option<Duration>,
) -> (
    &'static str,
    &'static str,
    String,
    Option<String>,
    Option<Duration>,
    Option<Duration>,
) {
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
                    previous_response_id.unwrap_or_default()
                ),
                "param": "previous_response_id",
            }
        })
        .to_string(),
        None,
        initial_body_stall,
        None,
    )
}

fn owned_continuation_sse_response(
    account_id: &str,
    previous_response_id: Option<&str>,
    mode: RuntimeProxyBackendMode,
) -> (
    &'static str,
    &'static str,
    String,
    Option<String>,
    Option<Duration>,
    Option<Duration>,
) {
    let next_response_id =
        runtime_proxy_backend_next_response_id(previous_response_id).expect("next response id should exist");
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
        ),
        None,
        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
            .then_some(Duration::from_millis(750)),
        matches!(
            (account_id, mode),
            (
                "second-account",
                RuntimeProxyBackendMode::HttpOnlySlowStream
                    | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
            ) | ("third-account", RuntimeProxyBackendMode::HttpOnlySlowStream)
        )
        .then_some(Duration::from_millis(100)),
    )
}

fn initial_account_response(
    account_id: &str,
    mode: RuntimeProxyBackendMode,
) -> (
    &'static str,
    &'static str,
    String,
    Option<String>,
    Option<Duration>,
    Option<Duration>,
) {
    let response_id = runtime_proxy_backend_initial_response_id_for_account(account_id)
        .expect("test account response id should exist");
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
                response_id, response_id
            ),
            None,
            matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                .then_some(Duration::from_millis(750)),
            matches!(
                (account_id, mode),
                (
                    "second-account",
                    RuntimeProxyBackendMode::HttpOnlySlowStream
                        | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                ) | ("third-account" | "fourth-account" | "fifth-account", RuntimeProxyBackendMode::HttpOnlySlowStream)
            )
            .then_some(Duration::from_millis(100)),
        )
    }
}
