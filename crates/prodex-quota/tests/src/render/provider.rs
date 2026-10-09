use super::*;

#[test]
fn quota_reports_render_copilot_rows_without_falling_back_to_error() {
    let reports = vec![
        openai_report("main", main_windows(90, 1_700_007_200, 95, 1_700_172_800)),
        copilot_report(Some("2026-05-09")),
    ];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("Available:"));
    assert!(output.contains("2/2 profile"));
    assert!(output.contains("copilot-main"));
    assert!(output.contains("copilot-user"));
    assert!(output.contains("individual"));
    assert!(output.contains("chat 450/500"));
    assert!(output.contains("comp 4000/4000"));
    assert!(output.contains("Ready"));
    assert!(!output.contains("status: Ready"));
    assert!(output.contains("resets: monthly 2026-05-09"));
    assert!(!output.contains("GitHub Copilot profiles do not expose ChatGPT quota"));
}

#[test]
fn quota_reports_aggregate_copilot_remaining_pool() {
    let reports = vec![copilot_report(None)];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("Remaining pool:"));
    assert!(output.contains(&format_info_pool_remaining(90, 1, None)));
    assert!(!output.contains("5h remaining pool:"));
    assert!(!output.contains("Weekly remaining pool:"));
}

#[test]
fn quota_reports_render_gemini_code_assist_buckets() {
    let reports = vec![QuotaReport {
        name: "gemini-main".to_string(),
        active: false,
        auth: AuthSummary {
            label: "gemini-oauth".to_string(),
            quota_compatible: true,
        },
        workspace_id: None,
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::Gemini(GeminiQuotaInfo {
            email: Some("gemini-user@example.com".to_string()),
            plan: Some("pro".to_string()),
            project_id: Some("proj-1".to_string()),
            buckets: vec![GeminiQuotaBucket {
                remaining_amount: Some("50".to_string()),
                remaining_fraction: Some(0.5),
                reset_time: Some("2026-05-09T00:00:00Z".to_string()),
                token_type: Some("TOKENS".to_string()),
                model_id: Some("models/gemini-2.5-pro".to_string()),
            }],
        })),
        fetched_at: 1_700_000_101,
    }];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("gemini-main"));
    assert!(output.contains("gemini-user@example.com"));
    assert!(output.contains("pro"));
    assert!(!output.contains("proj-1"));
    assert!(output.contains("gemini 50%"));
    assert!(output.contains("Ready"));
    assert!(!output.contains("status: Ready"));
    assert!(output.contains("resets: 2026-05-09T00:00:00Z"));

    let panel = render_profile_quota_snapshot("gemini-main", reports[0].result.as_ref().unwrap());
    assert!(panel.contains("Quota gemini-main"));
    assert!(panel.contains("Plan"));
    assert!(panel.contains("pro"));
    assert!(panel.contains("Project"));
    assert!(panel.contains("proj-1"));
    assert!(panel.contains("Main"));
    assert!(panel.contains("gemini 50%"));
    assert!(panel.contains("Bucket 1"));
    assert!(panel.contains("gemini-2.5-pro 50/100"));
}

#[test]
fn quota_reports_aggregate_gemini_remaining_buckets() {
    let reports = vec![QuotaReport {
        name: "gemini-main".to_string(),
        active: false,
        auth: AuthSummary {
            label: "gemini-oauth".to_string(),
            quota_compatible: true,
        },
        workspace_id: None,
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::Gemini(GeminiQuotaInfo {
            email: Some("gemini-user@example.com".to_string()),
            plan: Some("pro".to_string()),
            project_id: Some("proj-1".to_string()),
            buckets: vec![
                GeminiQuotaBucket {
                    remaining_amount: Some("100".to_string()),
                    remaining_fraction: Some(1.0),
                    reset_time: None,
                    token_type: Some("TOKENS".to_string()),
                    model_id: Some("models/gemini-2.5-flash".to_string()),
                },
                GeminiQuotaBucket {
                    remaining_amount: Some("80".to_string()),
                    remaining_fraction: Some(0.8),
                    reset_time: None,
                    token_type: Some("TOKENS".to_string()),
                    model_id: Some("models/gemini-2.5-pro".to_string()),
                },
                GeminiQuotaBucket {
                    remaining_amount: Some("95".to_string()),
                    remaining_fraction: Some(0.95),
                    reset_time: None,
                    token_type: Some("TOKENS".to_string()),
                    model_id: Some("models/gemini-2.5-flash-lite".to_string()),
                },
            ],
        })),
        fetched_at: 1_700_000_101,
    }];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("Remaining pool:"));
    assert!(output.contains(&format_info_pool_remaining(80, 1, None)));
    assert!(!output.contains("5h remaining pool:"));
    assert!(!output.contains("Weekly remaining pool:"));
    assert!(output.contains("gemini 80% (3 buckets)"));
    assert!(!output.contains("gemini-2.5-flash 100/100 |"));
}

#[test]
fn quota_reports_render_external_provider_snapshots() {
    let reports = vec![QuotaReport {
        name: "deepseek".to_string(),
        active: false,
        auth: AuthSummary {
            label: "deepseek-key".to_string(),
            quota_compatible: false,
        },
        workspace_id: None,
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::External(ExternalQuotaInfo {
            provider: "DeepSeek".to_string(),
            account: None,
            plan: Some("api-key".to_string()),
            status: "Ready".to_string(),
            main: "USD 12.50".to_string(),
            reset: None,
            available: Some(true),
            details: vec![ExternalQuotaDetail {
                label: "USD balance".to_string(),
                value: "total 12.50; granted 2.50; topped up 10.00".to_string(),
            }],
        })),
        fetched_at: 1_700_000_101,
    }];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("Available:"));
    assert!(output.contains("1/1 profile"));
    assert!(output.contains("deepseek"));
    assert!(output.contains("api-key"));
    assert!(output.contains("USD 12.50"));
    assert!(output.contains("Ready"));
    assert!(!output.contains("status: Ready"));

    let panel = render_profile_quota_snapshot("deepseek", reports[0].result.as_ref().unwrap());
    assert!(panel.contains("Quota deepseek"));
    assert!(panel.contains("Provider"));
    assert!(panel.contains("DeepSeek"));
    assert!(panel.contains("USD balance"));
}
