use super::*;

#[cfg(feature = "mojo")]
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

#[test]
fn main_quota_pool_uses_mojo_aggregation_for_present_and_absent_rows() {
    let absent = QuotaReport {
        name: "copilot-empty".to_string(),
        active: false,
        auth: AuthSummary {
            label: "copilot".to_string(),
            quota_compatible: false,
        },
        workspace_id: None,
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::Copilot(CopilotQuotaInfo {
            login: None,
            access_type_sku: None,
            copilot_plan: None,
            limited_user_quotas: BTreeMap::new(),
            monthly_quotas: BTreeMap::new(),
            limited_user_reset_date: None,
        })),
        fetched_at: 1_700_000_200,
    };
    let failed = QuotaReport {
        name: "failed".to_string(),
        active: false,
        auth: AuthSummary {
            label: "copilot".to_string(),
            quota_compatible: false,
        },
        workspace_id: None,
        workspace_name: None,
        result: Err("unavailable".to_string()),
        fetched_at: 1_700_000_300,
    };
    let gemini = QuotaReport {
        name: "gemini-main".to_string(),
        active: false,
        auth: AuthSummary {
            label: "gemini".to_string(),
            quota_compatible: true,
        },
        workspace_id: None,
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::Gemini(GeminiQuotaInfo {
            email: None,
            plan: None,
            project_id: None,
            buckets: vec![GeminiQuotaBucket {
                remaining_amount: None,
                remaining_fraction: Some(0.5),
                reset_time: None,
                token_type: None,
                model_id: Some("models/gemini-test".to_string()),
            }],
        })),
        fetched_at: 1_700_000_400,
    };

    let fields = quota_pool_summary_fields(&[copilot_report(None), gemini, absent, failed]);

    assert_eq!(
        fields[0],
        ("Available".to_string(), "3/4 profile".to_string())
    );
    assert_eq!(
        fields[2],
        (
            "Remaining pool".to_string(),
            "140% across 2 profile(s)".to_string(),
        )
    );
}
