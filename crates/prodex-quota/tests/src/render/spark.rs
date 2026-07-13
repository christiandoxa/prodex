use super::*;

#[test]
fn quota_reports_detail_shows_spark_additional_limit() {
    let mut usage = main_windows(65, 1_783_413_134, 48, 1_783_999_934);
    usage
        .additional_rate_limits
        .push(spark_limit(89, 1_783_413_134, 97, 1_783_999_934));

    let output = render_quota_reports_with_layout(&[openai_report("main", usage)], true, None, 160);

    assert!(output.contains("GPT-5.3-Codex-Spark: 5h 89% | weekly 97%; resets:"));
    assert!(output.contains("Spark remaining pool:"));
    assert!(output.contains("5h 89% | weekly 97% across 1 profile(s)"));
}

#[test]
fn quota_reports_show_weekly_only_spark_pool() {
    let mut usage = main_windows(0, 1_700_001_800, 0, 1_700_259_200);
    let mut spark = spark_limit(89, 1_783_413_134, 97, 1_783_999_934);
    spark.rate_limit.primary_window = None;
    usage.additional_rate_limits.push(spark);

    let output = render_quota_reports_with_layout(
        &[openai_report("spark-weekly-only", usage.clone())],
        true,
        None,
        160,
    );

    assert_eq!(format_openai_quota_status(&usage), "Ready");
    assert!(output.contains("GPT-5.3-Codex-Spark: weekly 97%"));
    assert!(output.contains("Spark remaining pool:"));
    assert!(output.contains("weekly 97% across 1 profile(s)"));
}

#[test]
fn quota_reports_status_is_ready_when_spark_remains() {
    let mut usage = main_windows(0, 1_700_001_800, 0, 1_700_259_200);
    usage
        .additional_rate_limits
        .push(spark_limit(89, 1_783_413_134, 97, 1_783_999_934));

    let output = render_quota_reports_with_layout(
        &[openai_report("spark-only", usage.clone())],
        true,
        None,
        160,
    );

    assert_eq!(format_openai_quota_status(&usage), "Ready");
    assert!(collect_blocked_limits(&usage, false).is_empty());
    assert!(output.contains("Available:"));
    assert!(output.contains("1/1 profile"));
    assert!(
        output
            .lines()
            .any(|line| line.contains("spark-only") && line.contains("Ready"))
    );
}

#[test]
fn quota_reports_status_stays_ready_when_main_remains_but_spark_is_blocked() {
    let mut usage = main_windows(50, 1_783_413_134, 37, 1_783_999_934);
    usage
        .additional_rate_limits
        .push(spark_limit(0, 1_783_413_134, 69, 1_783_999_934));

    let output = render_quota_reports_with_layout(
        &[openai_report("spark-blocked", usage.clone())],
        true,
        None,
        160,
    );

    assert_eq!(format_openai_quota_status(&usage), "Ready");
    assert!(collect_blocked_limits(&usage, false).is_empty());
    assert!(output.contains("Available:"));
    assert!(output.contains("1/1 profile"));
    assert!(output.contains("Usable now:"));
    assert!(output.contains("5h 50% | weekly 37% across 1 ready profile(s)"));
    assert!(output.contains("GPT-5.3-Codex-Spark: 5h 0% | weekly 69%"));
    assert!(
        output
            .lines()
            .any(|line| line.contains("spark-blocked") && line.contains("Ready"))
    );
}
