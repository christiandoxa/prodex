use super::*;

pub(super) fn handle_runtime_proxy_backend_compact_route(
    account_id: &str,
    request: &str,
    mode: RuntimeProxyBackendMode,
) -> RuntimeProxyBackendHttpResponse {
    let previous_response_id = request_previous_response_id(request);
    let (status_line, content_type, body, response_turn_state, initial_body_stall, chunk_delay) =
        match (account_id, mode) {
            (
                "main-account",
                RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                | RuntimeProxyBackendMode::HttpOnlyWorkspaceCreditsExhausted
                | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
            ) => (
                if matches!(mode, RuntimeProxyBackendMode::HttpOnlyWorkspaceCreditsExhausted) {
                    "HTTP/1.1 402 Payment Required"
                } else {
                    "HTTP/1.1 429 Too Many Requests"
                },
                "application/json",
                if matches!(mode, RuntimeProxyBackendMode::HttpOnlyWorkspaceCreditsExhausted) {
                    serde_json::json!({
                        "error": {
                            "message": "Your workspace is out of credits. Ask your workspace owner to refill in order to continue."
                        }
                    })
                    .to_string()
                } else {
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
                    .to_string()
                },
                None,
                None,
                None,
            ),
            (
                "second-account",
                RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                | RuntimeProxyBackendMode::HttpOnlyWorkspaceCreditsExhausted,
            ) => (
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
                    match account_id {
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
