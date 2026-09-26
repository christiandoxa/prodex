use super::*;
use std::time::Duration;

#[test]
fn committed_profile_promotion_preserves_existing_affinity() {
    assert!(runtime_websocket_should_promote_committed_profile(
        None, None, None, None, None, false, None
    ));
    assert!(runtime_websocket_should_promote_committed_profile(
        None, None, None, None, None, true, None
    ));
    assert!(!runtime_websocket_should_promote_committed_profile(
        Some("resp_1"),
        None,
        None,
        None,
        None,
        false,
        None,
    ));
    assert!(!runtime_websocket_should_promote_committed_profile(
        None,
        None,
        Some("turn-state"),
        None,
        None,
        false,
        None,
    ));
    assert!(!runtime_websocket_should_promote_committed_profile(
        None,
        None,
        None,
        None,
        None,
        false,
        Some("alpha"),
    ));
    assert!(!runtime_websocket_should_promote_committed_profile(
        None,
        Some("alpha"),
        None,
        None,
        None,
        false,
        None,
    ));
    assert!(!runtime_websocket_should_promote_committed_profile(
        None,
        None,
        None,
        Some("alpha"),
        None,
        false,
        None,
    ));
    let compact_followup = ("resp_1".to_string(), "alpha");
    assert!(!runtime_websocket_should_promote_committed_profile(
        None,
        None,
        None,
        None,
        Some(&compact_followup),
        false,
        None,
    ));
}

#[test]
fn direct_current_fallback_reason_labels_match_runtime_logs() {
    assert_eq!(
        RuntimeWebsocketDirectCurrentFallbackReason::PrecommitBudgetExhausted.as_str(),
        "precommit_budget_exhausted"
    );
    assert!(
        RuntimeWebsocketDirectCurrentFallbackReason::PrecommitBudgetExhausted
            .reset_previous_response_retry_index_on_local_block()
    );
    assert!(
        !RuntimeWebsocketDirectCurrentFallbackReason::CandidateExhausted
            .reset_previous_response_retry_index_on_local_block()
    );
}

#[test]
fn websocket_error_payload_preserves_text_binary_and_empty() {
    assert_eq!(
        runtime_websocket_error_payload_from_http_body(b""),
        RuntimeWebsocketErrorPayload::Empty
    );
    assert_eq!(
        runtime_websocket_error_payload_from_http_body(b"quota"),
        RuntimeWebsocketErrorPayload::Text("quota".to_string())
    );
    assert_eq!(
        runtime_websocket_error_payload_from_http_body(b"\xff"),
        RuntimeWebsocketErrorPayload::Binary(vec![0xff])
    );
}

#[test]
fn websocket_precommit_previous_response_frame_translation_preserves_failed_event_shape() {
    let payload = serde_json::json!({
        "type": "response.failed",
        "status": 400,
        "error": {
            "code": "previous_response_not_found",
            "message": "Previous response with id 'resp-123' not found.",
        }
    })
    .to_string();

    let translated = runtime_translate_precommit_previous_response_websocket_text_frame(&payload);
    let value = serde_json::from_str::<serde_json::Value>(&translated).expect("json");

    assert_eq!(
        value.get("type").and_then(serde_json::Value::as_str),
        Some("response.failed")
    );
    assert_eq!(
        value.get("status").and_then(serde_json::Value::as_u64),
        Some(409)
    );
    assert_eq!(
        value
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("stale_continuation")
    );
    assert!(
        !translated.contains("previous_response_not_found"),
        "translated frame should not leak raw previous_response_not_found: {translated}"
    );
}

#[test]
fn websocket_direct_previous_response_frame_is_forwarded_without_translation() {
    let payload = serde_json::json!({
        "type": "response.failed",
        "status": 400,
        "error": {
            "code": "previous_response_not_found",
            "message": "Previous response with id 'resp-123' not found.",
        }
    })
    .to_string();

    let translated = runtime_translate_previous_response_websocket_text_frame(&payload);
    let value = serde_json::from_str::<serde_json::Value>(&translated).expect("json");

    assert_eq!(translated, payload);
    assert_eq!(
        value
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("previous_response_not_found")
    );
}

#[test]
fn websocket_precommit_previous_response_plain_text_translation_uses_proxy_error_shape() {
    let translated = runtime_translate_precommit_previous_response_websocket_text_frame(
        "previous_response_not_found: Previous response with id 'resp-123' not found.",
    );

    let value = serde_json::from_str::<serde_json::Value>(&translated).expect("json");
    assert_eq!(
        value.get("type").and_then(serde_json::Value::as_str),
        Some("error")
    );
    assert_eq!(
        value.get("status").and_then(serde_json::Value::as_u64),
        Some(409)
    );
    assert_eq!(
        value
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("stale_continuation")
    );
}

#[test]
fn websocket_text_frame_inspection_classifies_retry_and_terminal_events() {
    for code in [
        "insufficient_quota",
        "usage_not_included",
        "workspace_member_credits_depleted",
    ] {
        let payload = serde_json::json!({
            "type": "response.failed",
            "error": {
                "code": code,
                "message": "quota exceeded",
            },
            "response": {
                "id": "resp_1",
                "headers": {
                    "x-codex-turn-state": "turn-1"
                }
            }
        })
        .to_string();

        let inspected = inspect_runtime_websocket_text_frame(&payload);

        assert_eq!(
            inspected.retry_kind,
            Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked),
            "{code}"
        );
        assert_eq!(inspected.response_ids, vec!["resp_1".to_string()]);
        assert_eq!(inspected.turn_state.as_deref(), Some("turn-1"));
        assert!(inspected.terminal_event);
    }
}

#[test]
fn websocket_profile_unavailable_is_retryable_without_quota_classification() {
    let payload = serde_json::json!({
        "type": "response.failed",
        "error": {
            "code": "deactivated_workspace",
            "message": "profile unavailable",
        }
    })
    .to_string();

    let inspected = inspect_runtime_websocket_text_frame(&payload);

    assert_eq!(
        inspected.retry_kind,
        Some(RuntimeWebsocketRetryInspectionKind::Overloaded)
    );
    assert!(inspected.terminal_event);
}

#[test]
fn websocket_rate_limit_is_not_classified_as_overload() {
    let payload = serde_json::json!({
        "type": "response.failed",
        "error": {
            "code": "rate_limit_exceeded",
            "message": "Please try again in 1s."
        }
    })
    .to_string();

    let inspected = inspect_runtime_websocket_text_frame(&payload);

    assert_eq!(
        inspected.retry_kind,
        Some(RuntimeWebsocketRetryInspectionKind::RateLimited)
    );
    assert_eq!(inspected.retry_after, Some(Duration::from_secs(1)));
}

#[test]
fn websocket_text_frame_inspection_never_retries_after_commit() {
    for (code, action) in [
        ("insufficient_quota", RuntimeHttpErrorAction::RotateProfile),
        ("server_is_overloaded", RuntimeHttpErrorAction::RetryProfile),
    ] {
        let payload = serde_json::json!({
            "type": "response.failed",
            "status": 429,
            "error": {
                "code": code,
                "message": "upstream failure",
            }
        })
        .to_string();

        let precommit = crate::runtime_stream_error_policy(
            payload.as_bytes(),
            RuntimeHttpErrorPhase::PreCommit,
        );
        assert_eq!(precommit.action, action, "{code}");

        let committed = inspect_runtime_websocket_text_frame_with_phase(
            &payload,
            RuntimeHttpErrorPhase::Committed,
        );
        assert_eq!(committed.retry_kind, None, "{code}");
        assert!(committed.terminal_event, "{code}");
    }
}

#[test]
fn websocket_text_frame_inspection_ignores_non_error_quota_code_text() {
    let payload = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": "The docs mention rate_limit_exceeded as an example.",
    })
    .to_string();

    let inspected = inspect_runtime_websocket_text_frame(&payload);

    assert_eq!(inspected.retry_kind, None);
    assert!(!inspected.terminal_event);
}

#[test]
fn websocket_text_frame_inspection_ignores_non_error_previous_response_code_text() {
    let payload = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": "previous_response_not_found is an upstream error code.",
    })
    .to_string();

    let inspected = inspect_runtime_websocket_text_frame(&payload);

    assert_eq!(inspected.retry_kind, None);
    assert!(!inspected.terminal_event);
}

#[test]
fn websocket_text_frame_inspection_classifies_connection_limit() {
    let payload = serde_json::json!({
        "type": "error",
        "error": {
            "code": "websocket_connection_limit_reached",
            "message": "Responses websocket connection limit reached (60 minutes). Create a new websocket connection to continue.",
        }
    })
    .to_string();

    let inspected = inspect_runtime_websocket_text_frame(&payload);

    assert_eq!(
        inspected.retry_kind,
        Some(RuntimeWebsocketRetryInspectionKind::ConnectionLimitReached)
    );
    assert!(inspected.terminal_event);
}

#[test]
fn websocket_text_frame_inspection_treats_wrapped_status_error_as_terminal() {
    let payload = serde_json::json!({
        "type": "error",
        "status_code": 400,
        "error": {
            "type": "invalid_request_error",
            "message": "Model does not support image inputs",
        }
    })
    .to_string();

    let inspected = inspect_runtime_websocket_text_frame(&payload);

    assert_eq!(inspected.retry_kind, None);
    assert!(inspected.terminal_event);
    assert!(is_runtime_terminal_event(&payload));

    let unwrapped = r#"{"type":"error","error":{"message":"still open"}}"#;
    assert!(!inspect_runtime_websocket_text_frame(unwrapped).terminal_event);
    assert!(!is_runtime_terminal_event(unwrapped));
}

#[test]
fn responses_terminal_event_payloads_are_terminal() {
    assert!(is_runtime_terminal_event(
        r#"{"type":"response.completed"}"#
    ));
    assert!(is_runtime_terminal_event(
        r#"{"type":"response.incomplete"}"#
    ));
}

#[test]
fn websocket_event_kind_helpers_match_expected_values() {
    let cases = [
        ("codex.rate_limits", true, false, false),
        ("codex.response.metadata", true, false, false),
        ("response.metadata", true, false, false),
        ("response.created", true, false, false),
        ("response.in_progress", true, false, false),
        ("response.queued", true, false, false),
        ("response.output_item.added", true, false, false),
        ("response.content_part.added", true, false, false),
        ("response.reasoning_summary_part.added", true, false, false),
        ("session.started", false, true, false),
        ("session.updated", false, true, false),
        ("conversation.item.added", false, true, false),
        ("conversation.item.done", false, true, false),
        ("delegation.created", false, true, false),
        ("response.cancelled", false, true, false),
        ("response.done", false, true, false),
        ("turn.done", false, true, false),
        ("error", false, true, false),
        ("response.completed", false, false, true),
        ("response.failed", false, false, true),
        ("response.incomplete", false, false, true),
        ("", false, false, false),
        ("response.unknown", false, false, false),
        ("réponse.created", false, false, false),
        ("回應.completed", false, false, false),
        ("🚀", false, false, false),
    ];

    for (kind, precommit_hold, realtime_terminal, responses_terminal) in cases {
        let expected = (precommit_hold, realtime_terminal, responses_terminal);
        let mojo = prodex_mojo_core::rich::websocket_event_kind(kind)
            .expect("Mojo websocket event classification");
        assert_eq!(
            (
                runtime_proxy_precommit_hold_event_kind(kind),
                runtime_realtime_websocket_terminal_event_kind(kind),
                runtime_responses_websocket_terminal_event_kind(kind),
            ),
            expected,
            "runtime event helpers for {kind}"
        );
        assert_eq!(
            (
                mojo.precommit_hold,
                mojo.realtime_terminal,
                mojo.responses_terminal,
            ),
            expected,
            "Mojo adapter for {kind}"
        );
    }

    let over_limit_kind = "x".repeat(4_097);
    let mojo = prodex_mojo_core::rich::websocket_event_kind(&over_limit_kind)
        .expect("Mojo websocket event classification");
    assert_eq!(
        (
            runtime_proxy_precommit_hold_event_kind(&over_limit_kind),
            runtime_realtime_websocket_terminal_event_kind(&over_limit_kind),
            runtime_responses_websocket_terminal_event_kind(&over_limit_kind),
        ),
        (false, false, false)
    );
    assert_eq!(
        (
            mojo.precommit_hold,
            mojo.realtime_terminal,
            mojo.responses_terminal,
        ),
        (false, false, false)
    );
}
