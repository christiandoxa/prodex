#[test]
fn previous_response_message_detects_top_level_json_shape() {
    let payload = serde_json::json!({
        "type": "invalid_request_error",
        "code": "previous_response_not_found",
        "message": "Previous response with id 'resp-123' not found.",
    });

    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        Some("Previous response with id 'resp-123' not found.".to_string())
    );
}

#[test]
fn previous_response_message_detects_plain_text_tool_context_signature() {
    let body = b"invalid_request_error: No tool call found for output item call_123";

    assert_eq!(
        extract_runtime_proxy_previous_response_message(body),
        Some("invalid_request_error: No tool call found for output item call_123".to_string())
    );
}

#[test]
fn previous_response_message_ignores_non_error_content_text() {
    let payload = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": "The docs mention previous_response_not_found and No tool call found as examples.",
    });

    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        None
    );
}

#[test]
fn quota_message_detects_nested_error_payloads() {
    let payload = serde_json::json!({
        "items": [
            {
                "wrapper": {
                    "error": {
                        "code": "rate_limit_exceeded",
                        "message": "You've hit your usage limit. Try again at 8pm."
                    }
                }
            }
        ]
    });

    assert_eq!(
        extract_runtime_proxy_quota_message_from_value(&payload),
        Some("You've hit your usage limit. Try again at 8pm.".to_string())
    );
}

#[test]
fn quota_http_body_detection_accepts_explicit_quota_payloads_but_not_generic_429() {
    assert_eq!(
        extract_runtime_proxy_quota_message(br#"{"error":{"message":"Too Many Requests"}}"#),
        None
    );
    assert_eq!(
        extract_runtime_proxy_quota_message(
            br#"{"error":{"message":"The usage limit has been reached"}}"#
        ),
        None
    );
    assert_eq!(
        extract_runtime_proxy_quota_message(br#"{"detail":{"code":"deactivated_workspace"}}"#),
        None
    );

    for code in [
        "insufficient_quota",
        "quota_exhausted",
        "quota_exceeded",
        "resource_exhausted",
        "usage_not_included",
        "usage_limit_reached",
        "workspace_member_credits_depleted",
    ] {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": code,
                "message": "Quota exhausted"
            }
        }))
        .expect("quota body should serialize");

        assert_eq!(
            extract_runtime_proxy_quota_message(&body),
            Some("Quota exhausted".to_string()),
            "{code}"
        );
    }

    assert_eq!(
        extract_runtime_proxy_quota_message(
            br#"{"error":{"message":"Your workspace is out of credits. Ask your workspace owner to refill in order to continue."}}"#
        ),
        None
    );
}

#[test]
fn websocket_workspace_credit_detection_uses_central_code_and_message_rules() {
    for payload in [
        RuntimeWebsocketErrorPayload::Text(
            r#"{"error":{"code":"workspace_member_credits_depleted"}}"#.to_string(),
        ),
        RuntimeWebsocketErrorPayload::Text(
            "Your workspace is out of credits. Ask your workspace owner to refill.".to_string(),
        ),
        RuntimeWebsocketErrorPayload::Binary(
            br#"{"error":{"type":"workspace_member_credits_depleted"}}"#.to_vec(),
        ),
    ] {
        assert!(runtime_websocket_workspace_credit_exhausted(&payload));
    }

    assert!(!runtime_websocket_workspace_credit_exhausted(
        &RuntimeWebsocketErrorPayload::Text(
            r#"{"error":{"code":"insufficient_quota"}}"#.to_string(),
        )
    ));
}

#[test]
fn overload_message_detects_top_level_and_nested_error_payloads() {
    let top_level = serde_json::json!({
        "code": "server_is_overloaded",
        "message": "Upstream Codex backend is currently overloaded.",
    });
    let nested = serde_json::json!({
        "meta": [
            {
                "response": {
                    "error": {
                        "message": "Selected model is at capacity. Please try again."
                    }
                }
            }
        ]
    });

    assert_eq!(
        extract_runtime_proxy_overload_message_from_value(&top_level),
        Some("Upstream Codex backend is currently overloaded.".to_string())
    );
    assert_eq!(
        extract_runtime_proxy_overload_message_from_value(&nested),
        Some("Selected model is at capacity. Please try again.".to_string())
    );
}

#[test]
fn invalid_utf8_bodies_do_not_trigger_lossy_retry_classification() {
    assert_eq!(
        extract_runtime_proxy_quota_message(b"\xffYou've hit your usage limit. Try again at 8pm."),
        None
    );
    assert_eq!(
        extract_runtime_proxy_previous_response_message(
            b"\xffprevious_response_not_found: missing"
        ),
        None
    );
    assert_eq!(
        extract_runtime_proxy_overload_message(
            503,
            b"\xffSelected model is at capacity. Please try again."
        ),
        Some("Upstream Codex backend returned transient HTTP 503.".to_string())
    );
    assert_eq!(
        extract_runtime_proxy_overload_message(500, b"\xff"),
        Some("Upstream Codex backend is currently experiencing high demand.".to_string())
    );
}

