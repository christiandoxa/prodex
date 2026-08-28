use super::*;
use std::collections::BTreeMap;

#[test]
fn real_mojo_quota_smoke_calls_exported_c_abi() {
    assert_eq!(remaining_percent(Some(42)), 58);
}

#[test]
fn gemini_renderer_uses_normalized_batch_results() {
    let amount_only = GeminiQuotaBucket {
        remaining_amount: Some("50".to_string()),
        remaining_fraction: None,
        reset_time: None,
        token_type: None,
        model_id: Some("models/gemini-test".to_string()),
    };
    assert_eq!(format_gemini_bucket_summary(&amount_only), "gemini-test 50");
    assert_eq!(
        format_gemini_main_quota(&GeminiQuotaInfo {
            email: None,
            plan: None,
            project_id: None,
            buckets: vec![amount_only],
        }),
        "gemini 50"
    );

    let invalid_amount_with_fraction = GeminiQuotaBucket {
        remaining_amount: Some("not-a-number".to_string()),
        remaining_fraction: Some(0.5),
        reset_time: None,
        token_type: None,
        model_id: Some("models/gemini-test".to_string()),
    };
    assert_eq!(
        format_gemini_bucket_summary(&invalid_amount_with_fraction),
        "gemini-test quota unknown"
    );
    assert_eq!(
        format_gemini_main_quota(&GeminiQuotaInfo {
            email: None,
            plan: None,
            project_id: None,
            buckets: vec![invalid_amount_with_fraction],
        }),
        "gemini 50%"
    );

    let fraction_only = GeminiQuotaBucket {
        remaining_amount: None,
        remaining_fraction: Some(0.5),
        reset_time: None,
        token_type: None,
        model_id: Some("models/gemini-test".to_string()),
    };
    assert_eq!(
        format_gemini_bucket_summary(&fraction_only),
        "gemini-test 50/100"
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
fn main_quota_aggregate_matches_rust_oracle_for_generated_rows() {
    let mut state = 0x71756f74615f6167_u64;
    for case in 0..2_000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let count = (state % 32) as usize;
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let remaining_percent = (state & 3 != 0).then(|| (state % 201) as i64 - 100);
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let reset_at = (state & 3 != 0).then_some((state % 10_000) as i64);
            rows.push((remaining_percent, reset_at));
        }
        let mut profiles_with_data = 0usize;
        let mut pool_remaining = 0_i64;
        let mut earliest_reset_at: Option<i64> = None;
        for (remaining_percent, reset_at) in &rows {
            let Some(remaining_percent) = remaining_percent else {
                continue;
            };
            profiles_with_data += 1;
            pool_remaining = pool_remaining.saturating_add(*remaining_percent);
            if let Some(reset_at) = reset_at {
                earliest_reset_at =
                    Some(earliest_reset_at.map_or(*reset_at, |current| current.min(*reset_at)));
            }
        }
        assert_eq!(
            crate::mojo::main_quota_aggregate(&rows),
            Ok((profiles_with_data, pool_remaining, earliest_reset_at)),
            "quota aggregation case {case}"
        );
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
