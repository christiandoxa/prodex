#[cfg(feature = "mojo")]
#[test]
fn compiled_quota_report_batch_sorts_large_live_report_sets_and_stable_duplicates() {
    let report = |name: String, fetched_at: i64| QuotaReport {
        name,
        active: false,
        auth: AuthSummary {
            label: "external".to_string(),
            quota_compatible: false,
        },
        workspace_id: None,
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::External(ExternalQuotaInfo {
            provider: "local".to_string(),
            account: None,
            plan: None,
            status: "unknown".to_string(),
            main: "unknown".to_string(),
            reset: None,
            available: None,
            details: Vec::new(),
        })),
        fetched_at,
    };
    let mut reports = (0..2_049_i64)
        .rev()
        .map(|index| report(format!("quota-{index:04}"), index))
        .collect::<Vec<_>>();
    // These rows compare equal in every sort mode and must retain original
    // input indices even after a large compiled heap-sort permutation.
    reports[7].name = "duplicate".to_string();
    reports[1_023].name = "duplicate".to_string();
    reports[2_048].name = "duplicate".to_string();
    for mode in [
        QuotaReportSort::Current,
        QuotaReportSort::Remaining,
        QuotaReportSort::Profile,
        QuotaReportSort::Auth,
        QuotaReportSort::Account,
        QuotaReportSort::Plan,
    ] {
        let indices = sorted_quota_report_indexes_by(&reports, mode);
        assert_eq!(indices.len(), reports.len(), "mode {mode:?}");
        let mut seen = vec![false; reports.len()];
        for index in indices.iter().copied() {
            assert!(!std::mem::replace(&mut seen[index], true));
        }
        let duplicates = indices
            .iter()
            .copied()
            .filter(|index| reports[*index].name == "duplicate")
            .collect::<Vec<_>>();
        assert_eq!(duplicates, vec![7, 1_023, 2_048], "mode {mode:?}");
    }
    let profile_indices = sorted_quota_report_indexes_by(&reports, QuotaReportSort::Profile);
    let names = profile_indices
        .iter()
        .map(|index| reports[*index].name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names.first(), Some(&"duplicate"));
    assert_eq!(names.last(), Some(&"quota-2048"));
}

