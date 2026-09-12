use super::*;

#[test]
fn missing_optional_additional_model_bucket_keeps_regular_models_ready() {
    let usage = main_windows(20, 1_700_001_800, 30, 1_700_259_200);

    assert!(openai_usage_supports_model(
        &usage,
        false,
        Some("gpt-5.6-luna")
    ));
    assert!(openai_usage_supports_model(
        &usage,
        false,
        Some("gpt-5.3-codex")
    ));
}

#[test]
fn retired_model_bucket_does_not_grant_pro_or_prolite_entitlement() {
    for plan in ["pro", "prolite"] {
        let mut usage = main_windows(80, 1_700_001_800, 90, 1_700_259_200);
        usage.plan_type = Some(plan.to_string());
        let mut retired = additional_limit(80, 1_700_003_600, 90, 1_700_086_400);
        retired.limit_id = Some("spark".to_string());
        retired.limit_name = Some("GPT-5.3-Codex-Spark".to_string());
        retired.metered_feature = Some("codex_bengalfox".to_string());
        retired.extra.insert(
            "normalModelSlug".to_string(),
            serde_json::json!("gpt-5.3-codex-spark"),
        );
        usage.additional_rate_limits.push(retired);
        assert!(openai_quota_has_ready_regular_limit(&usage));

        let bucket = usage.additional_rate_limits.first().unwrap();
        assert_eq!(
            additional_rate_limit_model_slug(bucket),
            Some("gpt-5.3-codex-spark")
        );
        for model in ["gpt-5.3-codex-spark", "spark", "gpt-5.3-spark"] {
            assert!(!openai_quota_has_ready_limit_for_model(&usage, Some(model)));
            assert!(!openai_usage_supports_model(&usage, false, Some(model)));
            assert!(openai_quota_runtime_window_pair_for_model(&usage, Some(model)).is_none());
        }
    }
}

#[test]
fn five_hour_exhaustion_blocks_regular_model_even_with_weekly_capacity() {
    let usage = main_windows(0, 1_700_001_800, 80, 1_700_259_200);

    assert!(!openai_quota_has_ready_regular_limit(&usage));
    assert!(!openai_usage_supports_model(
        &usage,
        false,
        Some("gpt-5.3-codex")
    ));
}

#[test]
fn backend_reached_state_overrides_windows_but_false_spend_control_does_not() {
    let mut usage = main_windows(80, 1_700_001_800, 80, 1_700_259_200);
    usage
        .rate_limit
        .as_mut()
        .unwrap()
        .extra
        .insert("spendControlReached".to_string(), serde_json::json!(false));
    assert!(openai_quota_has_ready_regular_limit(&usage));

    usage.rate_limit.as_mut().unwrap().extra.insert(
        "rateLimitReachedType".to_string(),
        serde_json::json!("rate_limit_reached"),
    );
    assert!(!openai_quota_has_ready_regular_limit(&usage));
}

#[test]
fn luna_reserve_requires_its_explicit_bucket_and_never_regularizes_sol() {
    let mut usage = main_windows(80, 1_700_001_800, 0, 1_700_259_200);
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));

    let mut reserve = additional_limit(90, 1_700_003_600, 95, 1_700_086_400);
    reserve.limit_id = Some("base_model_inference".to_string());
    reserve.limit_name = Some("gpt-luna-reserve".to_string());
    reserve.metered_feature = Some("base_model_inference".to_string());
    usage.additional_rate_limits.push(reserve);

    assert!(additional_rate_limit_is_luna_reserve(
        usage.additional_rate_limits.first().unwrap()
    ));
    assert!(openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));
    for model in ["gpt-reserve", "gpt-luna-reserve"] {
        assert!(!openai_quota_has_ready_limit_for_model(&usage, Some(model)));
        assert!(std::ptr::eq(
            openai_quota_runtime_window_pair_for_model(&usage, Some(model)).unwrap(),
            usage.rate_limit.as_ref().unwrap(),
        ));
    }
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-sol")
    ));

    usage.additional_rate_limits[0].extra.insert(
        "normalModelSlug".to_string(),
        serde_json::json!("future-model"),
    );
    assert!(!additional_rate_limit_is_luna_reserve(
        &usage.additional_rate_limits[0]
    ));

    let mut unlabeled = usage.clone();
    unlabeled.additional_rate_limits[0].limit_name = None;
    assert!(!additional_rate_limit_is_luna_reserve(
        &unlabeled.additional_rate_limits[0]
    ));

    usage.additional_rate_limits[0]
        .rate_limit
        .primary_window
        .as_mut()
        .unwrap()
        .used_percent = Some(100);
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));
}

#[test]
fn unlabeled_additional_bucket_does_not_grant_luna_capacity() {
    let mut usage = main_windows(0, 1_700_001_800, 0, 1_700_259_200);
    usage
        .additional_rate_limits
        .push(additional_limit(90, 1_700_003_600, 95, 1_700_086_400));
    usage.additional_rate_limits[0].limit_name = None;
    usage.additional_rate_limits[0].metered_feature = Some("opaque_bucket".to_string());

    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.3-codex")
    ));
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));
}

#[test]
fn unrecognized_model_uses_only_regular_quota() {
    let usage = main_windows(80, 1_700_001_800, 95, 1_700_259_200);

    assert!(openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-unsupported")
    ));
    assert!(openai_usage_supports_model(
        &usage,
        false,
        Some("gpt-unsupported")
    ));
    assert!(std::ptr::eq(
        openai_quota_runtime_window_pair_for_model(&usage, Some("gpt-unsupported")).unwrap(),
        usage.rate_limit.as_ref().unwrap()
    ));
}

#[test]
fn luna_reserve_is_model_specific_and_kept_separate_from_regular_quota() {
    let mut usage = main_windows(0, 1_700_001_800, 0, 1_700_259_200);
    let mut reserve = additional_limit(70, 1_700_003_600, 80, 1_700_086_400);
    reserve.limit_name = Some("Luna Reserve".to_string());
    reserve.metered_feature = None;
    usage.additional_rate_limits.push(reserve);

    assert!(additional_rate_limit_is_luna_reserve(
        usage.additional_rate_limits.first().unwrap()
    ));
    assert!(openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));
    assert_eq!(
        openai_quota_runtime_window_pair_for_model(&usage, Some("gpt-5.6-luna"))
            .and_then(|pair| find_main_window(pair, "5h"))
            .and_then(|window| window.used_percent),
        Some(30)
    );
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-sol")
    ));
}

#[test]
fn regular_luna_quota_beats_luna_reserve_when_both_are_ready() {
    let mut usage = main_windows(60, 1_700_001_800, 70, 1_700_259_200);
    let mut reserve = additional_limit(90, 1_700_003_600, 90, 1_700_086_400);
    reserve.limit_name = Some("Luna Reserve".to_string());
    reserve.metered_feature = None;
    usage.additional_rate_limits.push(reserve);

    assert_eq!(
        openai_quota_runtime_window_pair_for_model(&usage, Some("luna"))
            .and_then(|pair| find_main_window(pair, "5h"))
            .and_then(|window| window.used_percent),
        Some(40)
    );
}
