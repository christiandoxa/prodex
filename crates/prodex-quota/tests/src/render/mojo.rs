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
#[test]
fn openai_model_kind_matches_rust_normalization_oracle() {
    let normalize = |value: &str| {
        value
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
    };
    let rust_kind = |value: Option<&str>| match value.map(normalize).as_deref() {
        None => prodex_mojo_core::quota::QUOTA_MODEL_KIND_NONE,
        Some("luna" | "gpt56luna") => prodex_mojo_core::quota::QUOTA_MODEL_KIND_LUNA,
        Some("spark" | "gpt53codexspark" | "gpt53spark") => {
            prodex_mojo_core::quota::QUOTA_MODEL_KIND_RETIRED_SPARK
        }
        Some(_) => prodex_mojo_core::quota::QUOTA_MODEL_KIND_OTHER,
    };

    let samples = [
        None,
        Some(""),
        Some(" luna "),
        Some("L-U_N.A"),
        Some("gpt-5.6-luna"),
        Some("GPT_5 6_LUNA"),
        Some("spark"),
        Some("GPT-5.3-CODEX-SPARK"),
        Some("gpt 5.3 spark"),
        Some("gpt-5.6-sol"),
        Some("λ-gpt-5.6-luna-🔥"),
    ];
    for sample in samples {
        assert_eq!(
            crate::mojo::openai_model_kind(sample),
            rust_kind(sample),
            "model={sample:?}"
        );
    }
}

#[test]
fn luna_reserve_identifier_matches_rust_oracle() {
    let normalize = |value: &str| {
        value
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
    };
    let rust = |model_slug: Option<&str>,
                limit_id: Option<&str>,
                limit_name: Option<&str>,
                metered_feature: Option<&str>| {
        model_slug.is_some_and(|slug| normalize(slug) == "gpt56luna")
            && [limit_id, limit_name, metered_feature]
                .into_iter()
                .flatten()
                .any(|value| {
                    let normalized = normalize(value);
                    normalized == "gptreserve"
                        || (normalized.contains("luna") && normalized.contains("reserve"))
                })
    };

    let cases = [
        (
            Some("gpt-5.6-luna"),
            Some("base_model_inference"),
            Some("gpt-reserve"),
            None,
        ),
        (Some("gpt-5.6-luna"), None, Some("Luna Reserve"), None),
        (
            Some(" GPT_5.6_LUNA "),
            None,
            None,
            Some("future-luna__reserve"),
        ),
        (
            Some("gpt-5.6-sol"),
            Some("gpt-reserve"),
            Some("Luna Reserve"),
            Some("luna_reserve"),
        ),
        (Some("gpt-5.6-luna"), None, None, Some("reserve_only")),
        (None, Some("gpt-reserve"), None, None),
    ];
    for (model_slug, limit_id, limit_name, metered_feature) in cases {
        assert_eq!(
            crate::mojo::luna_reserve_identifier(model_slug, limit_id, limit_name, metered_feature,),
            rust(model_slug, limit_id, limit_name, metered_feature),
            "case={model_slug:?}/{limit_id:?}/{limit_name:?}/{metered_feature:?}"
        );
    }
}

#[test]
fn openai_model_capacity_plan_matches_exhaustive_boolean_oracle() {
    use prodex_mojo_core::quota::{
        OpenAiModelCapacityInput, QUOTA_MODEL_KIND_LUNA, QUOTA_MODEL_KIND_NONE,
        QUOTA_MODEL_KIND_OTHER, QUOTA_MODEL_KIND_RETIRED_SPARK, QUOTA_MODEL_PAIR_DEFAULT,
        QUOTA_MODEL_PAIR_NONE, QUOTA_MODEL_PAIR_REGULAR, QUOTA_MODEL_PAIR_RESERVE,
    };

    for model_kind in [
        QUOTA_MODEL_KIND_NONE,
        QUOTA_MODEL_KIND_LUNA,
        QUOTA_MODEL_KIND_RETIRED_SPARK,
        QUOTA_MODEL_KIND_OTHER,
    ] {
        for bits in 0_u16..512 {
            let bit = |index| bits & (1_u16 << index) != 0_u16;
            let input = OpenAiModelCapacityInput {
                model_kind,
                regular_present: bit(0),
                regular_ready: bit(1),
                generic_ready: bit(2),
                reserve_ready: bit(3),
                regular_blocked: bit(4),
                any_unknown_window: bit(5),
                any_exhausted_window: bit(6),
                include_code_review: bit(7),
                code_review_ready: bit(8),
            };
            let unknown_luna_capacity = input.regular_present
                && !input.regular_blocked
                && !input.regular_ready
                && input.any_unknown_window
                && !input.any_exhausted_window;
            let (selected_pair, ready) = match model_kind {
                QUOTA_MODEL_KIND_NONE => (QUOTA_MODEL_PAIR_DEFAULT, input.generic_ready),
                QUOTA_MODEL_KIND_RETIRED_SPARK => (QUOTA_MODEL_PAIR_NONE, false),
                QUOTA_MODEL_KIND_LUNA => {
                    let selected = if input.regular_ready {
                        QUOTA_MODEL_PAIR_REGULAR
                    } else if input.reserve_ready {
                        QUOTA_MODEL_PAIR_RESERVE
                    } else if input.regular_present {
                        QUOTA_MODEL_PAIR_REGULAR
                    } else {
                        QUOTA_MODEL_PAIR_NONE
                    };
                    (selected, input.regular_ready || input.reserve_ready)
                }
                QUOTA_MODEL_KIND_OTHER => (
                    if input.regular_present {
                        QUOTA_MODEL_PAIR_REGULAR
                    } else {
                        QUOTA_MODEL_PAIR_NONE
                    },
                    input.regular_ready,
                ),
                _ => unreachable!(),
            };
            let mut supports =
                ready || (model_kind == QUOTA_MODEL_KIND_LUNA && unknown_luna_capacity);
            if input.include_code_review && !input.code_review_ready {
                supports = false;
            }

            let actual = crate::mojo::openai_model_capacity_plan(input);
            assert_eq!(
                actual.selected_pair, selected_pair,
                "kind={model_kind} bits={bits}"
            );
            assert_eq!(actual.ready, ready, "kind={model_kind} bits={bits}");
            assert_eq!(actual.supports, supports, "kind={model_kind} bits={bits}");
            assert_eq!(
                actual.unknown_luna_capacity, unknown_luna_capacity,
                "kind={model_kind} bits={bits}"
            );
        }
    }
}
