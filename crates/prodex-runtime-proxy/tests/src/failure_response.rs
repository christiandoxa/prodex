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
    assert!(String::from_utf8_lossy(&parts.body).contains("stale_continuation"));
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
    assert!(String::from_utf8_lossy(&translated.body).contains("stale_continuation"));
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
fn precommit_budget_scales_to_profile_pool() {
    let (base_attempt_limit, base_budget) = runtime_proxy_precommit_budget(false, false);
    let profile_count = base_attempt_limit + 3;

    let (attempt_limit, budget) =
        runtime_proxy_precommit_budget_for_profile_count(false, false, profile_count);

    assert_eq!(attempt_limit, profile_count);
    assert!(budget > base_budget);
    assert!(runtime_proxy_precommit_budget_exhausted_for_profile_count(
        Instant::now(),
        profile_count,
        false,
        false,
        profile_count,
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted_for_profile_count(
        Instant::now(),
        profile_count - 1,
        false,
        false,
        profile_count,
    ));
}

#[test]
fn precommit_budget_keeps_base_limit_for_small_pool() {
    let (base_attempt_limit, base_budget) = runtime_proxy_precommit_budget(true, false);

    let (attempt_limit, budget) = runtime_proxy_precommit_budget_for_profile_count(true, false, 1);

    assert_eq!(attempt_limit, base_attempt_limit);
    assert_eq!(budget, base_budget);
}
