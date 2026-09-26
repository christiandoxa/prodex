use super::*;
use crate::runtime_buffered_response_content_type;

#[test]
fn stale_continuation_parts_are_json_409() {
    let parts = runtime_proxy_stale_continuation_http_parts();

    assert_eq!(parts.status, 409);
    assert_eq!(
        runtime_buffered_response_content_type(&parts),
        Some("application/json")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&parts.body).unwrap(),
        serde_json::json!({
            "error": {
                "code": "stale_continuation",
                "message": runtime_proxy_stale_continuation_message()
            }
        })
    );
}

#[test]
fn translates_previous_response_not_found_payload_to_stale_continuation() {
    let parts = RuntimeBufferedResponseParts {
        status: 400,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: br#"{"error":{"code":"previous_response_not_found"}}"#
            .to_vec()
            .into(),
    };

    let translated = runtime_proxy_translate_previous_response_http_parts(parts);

    assert_eq!(translated.status, 409);
    assert_eq!(
        runtime_buffered_response_content_type(&translated),
        Some("application/json")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&translated.body).unwrap(),
        serde_json::json!({
            "error": {
                "code": "stale_continuation",
                "message": runtime_proxy_stale_continuation_message()
            }
        })
    );
}

#[test]
fn translates_previous_response_not_found_text_to_stale_continuation() {
    let parts = RuntimeBufferedResponseParts {
        status: 404,
        headers: vec![("Content-Type".to_string(), b"text/plain".to_vec())],
        body: b"previous_response_not_found: missing".to_vec().into(),
    };

    let translated = runtime_proxy_translate_previous_response_http_parts(parts);

    assert_eq!(translated.status, 409);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&translated.body).unwrap()["error"]["code"],
        "stale_continuation"
    );
}

#[test]
fn leaves_non_previous_response_failure_parts_unchanged() {
    let parts = RuntimeBufferedResponseParts {
        status: 500,
        headers: vec![("Content-Type".to_string(), b"text/plain".to_vec())],
        body: b"upstream failed".to_vec().into(),
    };

    let translated = runtime_proxy_translate_previous_response_http_parts(parts.clone());

    assert_eq!(translated, parts);
}

#[test]
fn preserves_429_even_when_body_mentions_previous_response_not_found() {
    let original = RuntimeBufferedResponseParts {
        status: 429,
        headers: vec![("x-provider".to_string(), b"example".to_vec())],
        body: br#"{"error":{"code":"previous_response_not_found"}}"#
            .to_vec()
            .into(),
    };

    assert_eq!(
        runtime_proxy_translate_previous_response_http_parts(original.clone()),
        original
    );
}

#[test]
fn websocket_previous_response_detection_matches_text_and_binary() {
    let text =
        RuntimeWebsocketErrorPayload::Text("previous_response_not_found: missing".to_string());
    let binary =
        RuntimeWebsocketErrorPayload::Binary(b"previous_response_not_found: missing".to_vec());

    assert!(runtime_websocket_error_payload_is_previous_response_not_found(&text));
    assert!(runtime_websocket_error_payload_is_previous_response_not_found(&binary));
    assert!(
        !runtime_websocket_error_payload_is_previous_response_not_found(
            &RuntimeWebsocketErrorPayload::Empty
        )
    );
}

#[test]
fn precommit_budget_matches_independent_expected_values() {
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(false, false, 0),
        (4, Duration::from_millis(1_000)),
    );
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(false, true, 0),
        (3, Duration::from_millis(150)),
    );
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(false, true, 2),
        (4, Duration::from_millis(200)),
    );
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(true, false, 0),
        (8, Duration::from_millis(4_000)),
    );
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(true, true, 5),
        (10, Duration::from_millis(5_000)),
    );
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(false, false, 3),
        (6, Duration::from_millis(1_500)),
    );
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(false, false, 1_000_000),
        (2_000_000, Duration::from_millis(500_000_000)),
    );
    #[cfg(target_pointer_width = "16")]
    let maximum_profile_budget_ms = 16_383_750;
    #[cfg(target_pointer_width = "32")]
    let maximum_profile_budget_ms = 1_073_741_823_750;
    #[cfg(target_pointer_width = "64")]
    let maximum_profile_budget_ms = u64::MAX;
    assert_eq!(
        runtime_proxy_precommit_budget_for_profile_count(false, false, usize::MAX),
        (usize::MAX, Duration::from_millis(maximum_profile_budget_ms)),
    );
}

#[test]
fn precommit_budget_exhaustion_uses_attempt_and_elapsed_limits() {
    assert!(!runtime_proxy_precommit_budget_exhausted_for_profile_count(
        Instant::now(),
        5,
        false,
        false,
        3,
    ));
    assert!(runtime_proxy_precommit_budget_exhausted_for_profile_count(
        Instant::now(),
        6,
        false,
        false,
        3,
    ));
    let expired = Instant::now()
        .checked_sub(Duration::from_millis(1_501))
        .expect("expired instant");

    assert!(runtime_proxy_precommit_budget_exhausted_for_profile_count(
        expired, 0, false, false, 3,
    ));
}
