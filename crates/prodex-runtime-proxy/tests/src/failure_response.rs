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
fn precommit_budget_scales_to_profile_pool() {
    let (base_attempt_limit, base_budget) = runtime_proxy_precommit_budget(false, false);
    let profile_count = base_attempt_limit + 3;

    let (attempt_limit, budget) =
        runtime_proxy_precommit_budget_for_profile_count(false, false, profile_count);

    assert_eq!(
        attempt_limit,
        profile_count * RUNTIME_PROXY_PRECOMMIT_ATTEMPTS_PER_PROFILE
    );
    assert!(budget > base_budget);
    assert!(runtime_proxy_precommit_budget_exhausted_for_profile_count(
        Instant::now(),
        attempt_limit,
        false,
        false,
        profile_count,
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted_for_profile_count(
        Instant::now(),
        attempt_limit - 1,
        false,
        false,
        profile_count,
    ));
}

#[test]
fn precommit_budget_attempt_limit_covers_one_bounded_retry_per_profile() {
    for continuation in [false, true] {
        for pressure_mode in [false, true] {
            let (base_attempt_limit, base_budget) =
                runtime_proxy_precommit_budget(continuation, pressure_mode);
            assert!(
                base_attempt_limit >= 3,
                "pre-commit auto-rotate should try at least three profiles before surfacing a final error"
            );

            for profile_count in 0..=base_attempt_limit + 8 {
                let (attempt_limit, budget) = runtime_proxy_precommit_budget_for_profile_count(
                    continuation,
                    pressure_mode,
                    profile_count,
                );
                let effective_profile_count = profile_count.max(1);
                let required_profile_attempts = effective_profile_count
                    .saturating_mul(RUNTIME_PROXY_PRECOMMIT_ATTEMPTS_PER_PROFILE);

                assert!(
                    attempt_limit >= required_profile_attempts,
                    "continuation={continuation} pressure={pressure_mode} profile_count={profile_count}"
                );
                assert!(
                    !runtime_proxy_precommit_budget_exhausted_for_profile_count(
                        Instant::now(),
                        required_profile_attempts - 1,
                        continuation,
                        pressure_mode,
                        profile_count,
                    ),
                    "continuation={continuation} pressure={pressure_mode} profile_count={profile_count}"
                );
                assert!(
                    runtime_proxy_precommit_budget_exhausted_for_profile_count(
                        Instant::now(),
                        attempt_limit,
                        continuation,
                        pressure_mode,
                        profile_count,
                    ),
                    "continuation={continuation} pressure={pressure_mode} profile_count={profile_count}"
                );
                if profile_count > base_attempt_limit {
                    assert!(
                        budget >= base_budget,
                        "continuation={continuation} pressure={pressure_mode} profile_count={profile_count}"
                    );
                }
            }
        }
    }
}

#[test]
fn precommit_budget_keeps_base_limit_for_small_pool() {
    let (base_attempt_limit, base_budget) = runtime_proxy_precommit_budget(true, false);

    let (attempt_limit, budget) = runtime_proxy_precommit_budget_for_profile_count(true, false, 1);

    assert_eq!(attempt_limit, base_attempt_limit);
    assert_eq!(budget, base_budget);
}

#[test]
fn precommit_elapsed_budget_remains_bounded_without_candidate_progress() {
    let profile_count = 2;
    let (_, budget) = runtime_proxy_precommit_budget_for_profile_count(false, false, profile_count);
    let expired = Instant::now()
        .checked_sub(budget + Duration::from_millis(1))
        .expect("expired instant");

    assert!(runtime_proxy_precommit_budget_exhausted_for_profile_count(
        expired,
        0,
        false,
        false,
        profile_count,
    ));
    assert!(runtime_proxy_precommit_budget_exhausted_for_profile_count(
        expired,
        profile_count * RUNTIME_PROXY_PRECOMMIT_ATTEMPTS_PER_PROFILE,
        false,
        false,
        profile_count,
    ));
}
