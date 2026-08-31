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
    assert!(openai_usage_supports_model(
        &spark_ready,
        false,
        Some("gpt-5.6-luna")
    ));
    assert!(!openai_usage_supports_model(
        &spark_ready,
        false,
        Some("gpt-5.6-sol")
    ));
    assert!(openai_usage_supports_model(
        &spark_ready,
        false,
        Some("gpt-5.3-codex-spark")
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
