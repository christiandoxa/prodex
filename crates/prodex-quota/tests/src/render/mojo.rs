use super::*;
use std::collections::BTreeMap;

#[test]
fn real_mojo_quota_smoke_calls_exported_c_abi() {
    assert_eq!(remaining_percent(Some(42)), 58);
}

#[test]
fn gemini_renderer_uses_normalized_batch_results() {
    let info = |bucket: GeminiQuotaBucket| GeminiQuotaInfo {
        email: None,
        plan: None,
        project_id: None,
        buckets: vec![bucket],
    };

    let amount_only = info(GeminiQuotaBucket {
        remaining_amount: Some("50".to_string()),
        remaining_fraction: None,
        reset_time: None,
        token_type: None,
        model_id: Some("models/gemini-test".to_string()),
    });
    assert_eq!(
        format_gemini_bucket_summaries(&amount_only),
        vec!["gemini-test 50".to_string()]
    );
    assert_eq!(format_gemini_main_quota(&amount_only), "gemini 50");

    let invalid_amount_with_fraction = info(GeminiQuotaBucket {
        remaining_amount: Some("not-a-number".to_string()),
        remaining_fraction: Some(0.5),
        reset_time: None,
        token_type: None,
        model_id: Some("models/gemini-test".to_string()),
    });
    assert_eq!(
        format_gemini_bucket_summaries(&invalid_amount_with_fraction),
        vec!["gemini-test quota unknown".to_string()]
    );
    assert_eq!(
        format_gemini_main_quota(&invalid_amount_with_fraction),
        "gemini 50%"
    );

    let fraction_only = info(GeminiQuotaBucket {
        remaining_amount: None,
        remaining_fraction: Some(0.5),
        reset_time: None,
        token_type: None,
        model_id: Some("models/gemini-test".to_string()),
    });
    assert_eq!(
        format_gemini_bucket_summaries(&fraction_only),
        vec!["gemini-test 50/100".to_string()]
    );
}

#[test]
fn remaining_percent_matches_rust_oracle() {
    for (used_percent, expected) in [
        (None, 0),
        (Some(i64::MIN), 100),
        (Some(-1), 100),
        (Some(0), 100),
        (Some(42), 58),
        (Some(100), 0),
        (Some(101), 0),
        (Some(i64::MAX), 0),
    ] {
        assert_eq!(remaining_percent(used_percent), expected);
        let rust = used_percent.map_or(0, |used| {
            if used < 0 {
                100
            } else if used > 100 {
                0
            } else {
                100 - used
            }
        });
        assert_eq!(remaining_percent(used_percent), rust);
    }
}

#[test]
fn quota_capacity_keeps_unknown_reserve_non_routable() {
    let _compiled_core = prodex_mojo_core::MOJO_ACTIVE;
    let now = 1_700_000_000;
    let window_pair = |five_hour_remaining: i64,
                       five_hour_reset_at: i64,
                       weekly_remaining: i64,
                       weekly_reset_at: i64| WindowPair {
        allowed: None,
        limit_reached: None,
        extra: BTreeMap::new(),
        primary_window: Some(UsageWindow {
            used_percent: Some(100 - five_hour_remaining),
            reset_at: Some(five_hour_reset_at),
            limit_window_seconds: Some(18_000),
        }),
        secondary_window: Some(UsageWindow {
            used_percent: Some(100 - weekly_remaining),
            reset_at: Some(weekly_reset_at),
            limit_window_seconds: Some(604_800),
        }),
    };
    let mut usage = UsageResponse {
        email: None,
        plan_type: Some("plus".to_string()),
        rate_limit: Some(window_pair(0, now + 3_600, 0, now + 86_400)),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };
    usage.additional_rate_limits.push(AdditionalRateLimit {
        limit_id: Some("luna-reserve".to_string()),
        limit_name: Some("Luna Reserve".to_string()),
        metered_feature: Some("future_reserve".to_string()),
        rate_limit: window_pair(88, now + 7_200, 97, now + 172_800),
        allowed: None,
        limit_reached: None,
        extra: BTreeMap::new(),
    });

    let candidates = crate::capacity::quota_capacity_candidates_for_usage_at(
        &usage,
        prodex_runtime_state::RuntimeRouteKind::Responses,
        now,
    )
    .expect("normalized capacity rows should classify");
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        candidates[0].output.lane,
        prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_MAIN
    );
    assert!(!candidates[0].output.routing_eligible);
    assert_eq!(
        candidates[1].output.lane,
        prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL
    );
    assert!(candidates[1].output.usable);
    assert!(!candidates[1].output.routing_eligible);
    assert!(!openai_quota_has_ready_limit(&usage));
}
#[test]
fn quota_error_summary_kind_matches_expected_precedence() {
    use prodex_mojo_core::quota::*;
    for (message, expected) in [
        ("quota unavailable", QUOTA_ERROR_KIND_UNAVAILABLE),
        ("missing server config", QUOTA_ERROR_KIND_CONFIG),
        ("HTTP 503 timeout", QUOTA_ERROR_KIND_SERVER),
        ("request timed out", QUOTA_ERROR_KIND_TIMEOUT),
        ("TLS proxy failure", QUOTA_ERROR_KIND_NETWORK),
        ("proxy handshake", QUOTA_ERROR_KIND_PROXY),
        ("connection refused", QUOTA_ERROR_KIND_CONNECTION),
        ("invalid token <redacted>", QUOTA_ERROR_KIND_INVALID_AUTH),
        ("HTTP 429 rate limit", QUOTA_ERROR_KIND_RATE_LIMIT),
        ("invalid json decode", QUOTA_ERROR_KIND_PARSE),
        ("empty response", QUOTA_ERROR_KIND_EMPTY),
        ("request cancelled", QUOTA_ERROR_KIND_CANCELLED),
        ("HTTP 403 forbidden", QUOTA_ERROR_KIND_FORBIDDEN),
        ("HTTP 404 not found", QUOTA_ERROR_KIND_NOT_FOUND),
        ("opaque provider failure", QUOTA_ERROR_KIND_OTHER),
        ("", QUOTA_ERROR_KIND_UNKNOWN),
    ] {
        assert_eq!(
            prodex_mojo_core::quota::quota_error_summary_kind(message).unwrap(),
            expected,
            "message={message:?}"
        );
    }
}

#[test]
fn blocked_limit_kind_matches_expected_status_priority() {
    use prodex_mojo_core::quota::*;
    for (message, expected) in [
        ("5h exhausted until tomorrow", QUOTA_BLOCKED_KIND_FIVE_HOUR),
        ("weekly exhausted until Friday", QUOTA_BLOCKED_KIND_WEEKLY),
        ("custom exhausted bucket", QUOTA_BLOCKED_KIND_EXHAUSTED),
        ("5h quota unknown", QUOTA_BLOCKED_KIND_NONE),
        ("", QUOTA_BLOCKED_KIND_NONE),
    ] {
        assert_eq!(
            prodex_mojo_core::quota::blocked_limit_kind(message).unwrap(),
            expected,
            "message={message:?}"
        );
    }
}
