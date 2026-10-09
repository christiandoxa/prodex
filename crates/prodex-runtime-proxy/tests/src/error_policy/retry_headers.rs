use super::*;

#[test]
fn structured_stream_retry_after_overrides_rate_limit_message() {
    let value = serde_json::json!({
        "type": "response.failed",
        "response": {
            "error": {
                "code": "rate_limit_exceeded",
                "message": "Please try again in 1s.",
                "headers": {"Retry-After": "5"}
            }
        }
    });

    let policy = runtime_stream_error_policy_from_value(&value, RuntimeHttpErrorPhase::PreCommit);

    assert_eq!(policy.class, RuntimeHttpErrorClass::RateLimited);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RetryProfile);
    assert_eq!(policy.retry_after, Some(Duration::from_secs(5)));
}

#[test]
fn websocket_error_retry_after_prefers_error_headers_over_top_level() {
    let value = serde_json::json!({
        "type": "error",
        "status": 429,
        "error": {
            "code": "rate_limit_exceeded",
            "message": "Please try again in 1s.",
            "headers": {"Retry-After": "5"}
        },
        "headers": {"retry-after": "12"}
    });

    let policy = runtime_stream_error_policy_from_value(&value, RuntimeHttpErrorPhase::PreCommit);

    assert_eq!(policy.class, RuntimeHttpErrorClass::RateLimited);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RetryProfile);
    assert_eq!(policy.retry_after, Some(Duration::from_secs(5)));
}

#[test]
fn structured_retry_after_uses_upstream_header_value_rules() {
    let base = |headers: serde_json::Value| {
        serde_json::json!({
            "type": "response.failed",
            "response": {
                "error": {
                    "code": "rate_limit_exceeded",
                    "message": "Please try again in 12s.",
                    "headers": headers
                }
            }
        })
    };

    assert_eq!(
        runtime_stream_error_policy_from_value(
            &base(serde_json::json!({"Retry-After": "\n5\n"})),
            RuntimeHttpErrorPhase::PreCommit,
        )
        .retry_after,
        Some(Duration::from_secs(12))
    );
    assert_eq!(
        runtime_stream_error_policy_from_value(
            &base(serde_json::json!({"Retry-After": "\t5\t"})),
            RuntimeHttpErrorPhase::PreCommit,
        )
        .retry_after,
        Some(Duration::from_secs(5))
    );
    assert_eq!(
        runtime_stream_error_policy_from_value(
            &base(serde_json::json!({
                "Retry-After": "5",
                "retry-after": "30"
            })),
            RuntimeHttpErrorPhase::PreCommit,
        )
        .retry_after,
        Some(Duration::from_secs(30))
    );
    assert_eq!(
        runtime_stream_error_policy_from_value(
            &base(serde_json::json!({"Retry-After": 5})),
            RuntimeHttpErrorPhase::PreCommit,
        )
        .retry_after,
        Some(Duration::from_secs(5))
    );
}

fn failed_with_headers(headers: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"type":"response.failed","response":{"error":{
        "code":"rate_limit_exceeded","message":"Please try again in 12s.",
        "headers":headers
    }}})
}

#[test]
fn duplicate_json_header_names_follow_upstream_insert_order_not_maximum() {
    for (headers, seconds) in [
        (serde_json::json!({"Retry-After":"30","retry-after":"5"}), 5),
        (
            serde_json::json!({"Retry-After":"30","retry-after":"invalid"}),
            12,
        ),
        (
            serde_json::json!({"Retry-After":"30","retry-after":"\n5"}),
            30,
        ),
        (
            serde_json::json!({"Retry-After":"30","retry-after":true}),
            12,
        ),
        (serde_json::json!({" Retry-After":"5"}), 12),
        (serde_json::json!({"Retry-After":"\u{a0}5\u{a0}"}), 12),
    ] {
        let policy = runtime_stream_error_policy_from_value(
            &failed_with_headers(headers),
            RuntimeHttpErrorPhase::PreCommit,
        );
        assert_eq!(policy.retry_after, Some(Duration::from_secs(seconds)));
    }
}

#[test]
fn invalid_nested_websocket_advice_uses_outer_headers_but_zero_wins() {
    for (inner, expected) in [("bad", 12), ("0", 0), ("000", 0), ("5", 5)] {
        let event = serde_json::json!({"type":"error","status":429,"error":{
            "code":"rate_limit_exceeded","message":"try again in 1s",
            "headers":{"Retry-After":inner}},"headers":{"Retry-After":"12"}});
        assert_eq!(
            runtime_stream_error_policy_from_value(&event, RuntimeHttpErrorPhase::PreCommit)
                .retry_after,
            Some(Duration::from_secs(expected))
        );
    }
}

#[test]
fn http_date_retry_advice_and_numeric_cap_preserve_existing_local_budget() {
    let future = httpdate::fmt_http_date(std::time::SystemTime::now() + Duration::from_secs(3600));
    for (header, seconds) in [
        (future, 300),
        ("Sun, 06 Nov 1994 08:49:37 GMT".into(), 0),
        ("99999".into(), 300),
        ("1.5".into(), 12),
    ] {
        let event = failed_with_headers(serde_json::json!({"Retry-After":header}));
        assert_eq!(
            runtime_stream_error_policy_from_value(&event, RuntimeHttpErrorPhase::PreCommit)
                .retry_after,
            Some(Duration::from_secs(seconds))
        );
    }
}

#[test]
fn real_sse_and_websocket_inspectors_retain_advice_without_postcommit_retry() {
    let event = failed_with_headers(serde_json::json!({"Retry-After":"5"}));
    let wire = serde_json::to_string(&event).unwrap();
    let sse = crate::parse_runtime_sse_event(std::slice::from_ref(&wire));
    assert!(sse.rate_limited);
    assert_eq!(sse.retry_after, Some(Duration::from_secs(5)));
    let ws = crate::inspect_runtime_websocket_text_frame_with_phase(
        &wire,
        RuntimeHttpErrorPhase::PreCommit,
    );
    assert_eq!(
        ws.retry_kind,
        Some(crate::RuntimeWebsocketRetryInspectionKind::RateLimited)
    );
    assert_eq!(ws.retry_after, Some(Duration::from_secs(5)));
    let committed = crate::inspect_runtime_websocket_text_frame_with_phase(
        &wire,
        RuntimeHttpErrorPhase::Committed,
    );
    assert_eq!(committed.retry_kind, None);
    assert_eq!(committed.retry_after, Some(Duration::from_secs(5)));
    let bytes = runtime_stream_error_policy(wire.as_bytes(), RuntimeHttpErrorPhase::PreCommit);
    assert_eq!(bytes.retry_after, Some(Duration::from_secs(5)));
}

#[test]
fn advice_does_not_reclassify_quota_or_success_as_retryable() {
    let quota = serde_json::json!({"type":"response.failed","response":{"error":{
        "code":"insufficient_quota","headers":{"Retry-After":"5"}}}});
    let policy = runtime_stream_error_policy_from_value(&quota, RuntimeHttpErrorPhase::PreCommit);
    assert_eq!(policy.class, RuntimeHttpErrorClass::Quota);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RotateProfile);
    assert_eq!(policy.retry_after, None);
    let success = serde_json::json!({"type":"response.completed","headers":{"Retry-After":"5"}});
    assert_eq!(
        runtime_stream_error_policy_from_value(&success, RuntimeHttpErrorPhase::PreCommit)
            .retry_after,
        None
    );
}
