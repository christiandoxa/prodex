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
