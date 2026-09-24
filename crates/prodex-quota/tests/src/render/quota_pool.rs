use super::*;

#[test]
fn quota_pool_available_count_excludes_blocked_profiles() {
    let reports = vec![
        openai_report("ready", main_windows(80, 1_700_001_800, 95, 1_700_259_200)),
        openai_report("blocked", main_windows(0, 1_700_003_600, 80, 1_700_086_400)),
    ];

    let output = render_quota_reports_with_layout(&reports, true, None, 90);

    assert!(output.contains("Available:"));
    assert!(output.contains("1/2 profile"));
    assert!(output.contains("Usable now:"));
    assert!(output.contains("5h 80% | weekly 95% across 1 ready profile(s)"));
}

#[cfg(feature = "mojo")]
#[test]
fn openai_quota_pool_summary_handles_more_than_1024_profiles() {
    let reports = (0..1_025)
        .map(|index| {
            openai_report(
                &format!("profile-{index}"),
                main_windows(80, i64::MAX, 90, i64::MAX),
            )
        })
        .collect::<Vec<_>>();
    let fields = quota_pool_summary_fields(&reports);

    assert_eq!(
        fields[0],
        ("Available".to_string(), "1025/1025 profile".to_string())
    );
    assert_eq!(
        fields[2],
        (
            "Usable now".to_string(),
            "5h 82000% | weekly 92250% across 1025 ready profile(s)".to_string(),
        )
    );
    assert_eq!(
        fields[3],
        (
            "5h remaining pool".to_string(),
            "82000% across 1025 profile(s)".to_string(),
        )
    );
}
