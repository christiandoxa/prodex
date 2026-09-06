use super::*;

#[test]
fn model_specific_capacity_does_not_cross_regular_and_spark_buckets() {
    let mut regular_ready = main_windows(80, 1_700_001_800, 95, 1_700_259_200);
    regular_ready
        .additional_rate_limits
        .push(spark_limit(0, 1_700_003_600, 0, 1_700_086_400));
    assert!(openai_usage_supports_model(
        &regular_ready,
        false,
        Some("gpt-5.6-luna")
    ));
    assert!(openai_usage_supports_model(
        &regular_ready,
        false,
        Some("gpt-5.6-sol")
    ));
    assert!(!openai_usage_supports_model(
        &regular_ready,
        false,
        Some("gpt-5.3-codex-spark")
    ));

    let mut spark_ready = main_windows(0, 1_700_001_800, 0, 1_700_259_200);
    spark_ready
        .additional_rate_limits
        .push(spark_limit(80, 1_700_003_600, 95, 1_700_086_400));
    assert!(!openai_quota_has_ready_limit_for_model(
        &spark_ready,
        Some("gpt-5.6-luna")
    ));
    assert!(!openai_usage_supports_model(
        &spark_ready,
        false,
        Some("gpt-5.6-luna")
    ));
    assert!(!openai_usage_supports_model(
        &spark_ready,
        false,
        Some("gpt-5.6-sol")
    ));
    assert!(!openai_usage_supports_model(
        &spark_ready,
        false,
        Some("gpt-5.3-codex")
    ));
    assert!(openai_usage_supports_model(
        &spark_ready,
        false,
        Some("gpt-5.3-codex-spark")
    ));
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

    let mut reserve = spark_limit(90, 1_700_003_600, 95, 1_700_086_400);
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
fn unlabeled_additional_bucket_does_not_grant_luna_or_spark() {
    let mut usage = main_windows(0, 1_700_001_800, 0, 1_700_259_200);
    usage
        .additional_rate_limits
        .push(spark_limit(90, 1_700_003_600, 95, 1_700_086_400));
    usage.additional_rate_limits[0].limit_name = None;
    usage.additional_rate_limits[0].metered_feature = Some("opaque_bucket".to_string());

    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.3-codex-spark")
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
    let mut reserve = spark_limit(70, 1_700_003_600, 80, 1_700_086_400);
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
    let mut reserve = spark_limit(90, 1_700_003_600, 90, 1_700_086_400);
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
