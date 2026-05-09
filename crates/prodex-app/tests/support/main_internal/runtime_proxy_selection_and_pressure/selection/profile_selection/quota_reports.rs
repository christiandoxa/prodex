use super::*;

#[test]
fn all_quota_watch_output_omits_watch_header_on_load_error() {
    let output = render_all_quota_watch_output(
        "2026-03-22 10:00:00 WIB",
        Err("load failed".to_string()),
        None,
        false,
    );

    assert!(output.contains("Quota"));
    assert!(!output.contains("Quota Watch"));
    assert!(!output.contains("Updated"));
    assert!(!output.contains("2026-03-22 10:00:00 WIB"));
    assert!(output.contains("load failed"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn quota_reports_detail_shows_workspace_only_for_duplicate_account_email() {
    let mut first_usage = usage_with_main_windows(90, 7_200, 95, 172_800);
    first_usage.email = Some("same@example.com".to_string());
    let mut second_usage = usage_with_main_windows(80, 3_600, 88, 86_400);
    second_usage.email = Some("same@example.com".to_string());
    let mut same_workspace_first_usage = usage_with_main_windows(75, 3_600, 86, 86_400);
    same_workspace_first_usage.email = Some("same-workspace@example.com".to_string());
    let mut same_workspace_second_usage = usage_with_main_windows(74, 3_600, 85, 86_400);
    same_workspace_second_usage.email = Some("same-workspace@example.com".to_string());
    let mut solo_usage = usage_with_main_windows(70, 1_800, 78, 259_200);
    solo_usage.email = Some("solo@example.com".to_string());
    let reports = vec![
        QuotaReport {
            name: "workspace-one".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: Some("acct_workspace_one_123456789".to_string()),
            result: Ok(ProviderQuotaSnapshot::OpenAi(first_usage)),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "workspace-two".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: Some("acct_workspace_two_987654321".to_string()),
            result: Ok(ProviderQuotaSnapshot::OpenAi(second_usage)),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "same-workspace-one".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: Some("acct_same_workspace".to_string()),
            result: Ok(ProviderQuotaSnapshot::OpenAi(same_workspace_first_usage)),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "same-workspace-two".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: Some("acct_same_workspace".to_string()),
            result: Ok(ProviderQuotaSnapshot::OpenAi(same_workspace_second_usage)),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "solo".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: Some("acct_solo".to_string()),
            result: Ok(ProviderQuotaSnapshot::OpenAi(solo_usage)),
            fetched_at: 1_700_000_100,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("workspace: acct_workspa...456789"));
    assert!(output.contains("workspace: acct_workspa...654321"));
    assert!(!output.contains("workspace: acct_same_workspace"));
    assert!(!output.contains("workspace: acct_solo"));
}

#[test]
fn quota_reports_respect_line_budget_while_preserving_sort_order() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                0, 3_600, 80, 86_400,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 7_200, 95, 172_800,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let output = render_quota_reports_with_line_limit(&reports, false, Some(16));

    assert!(output.contains("ready-early"));
    assert!(output.contains("ready-late"));
    assert!(!output.contains("blocked"));
    assert!(!output.contains("error"));
    assert!(output.contains("\n\nshowing top 2 of 4 profiles"));
}

#[test]
fn quota_reports_fit_requested_width_in_narrow_layout() {
    let reports = vec![
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                0, 3_600, 80, 86_400,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 72);

    assert!(output.lines().all(|line| text_width(line) <= 72));
}

#[test]
fn profile_quota_watch_output_renders_snapshot_body_without_watch_header() {
    let output = render_profile_quota_watch_output(
        "main",
        "2026-03-22 10:00:00 WIB",
        Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
            63, 18_000, 12, 604_800,
        ))),
    );

    assert!(output.contains("Quota main"));
    assert!(!output.contains("Quota Watch"));
    assert!(!output.contains("Updated"));
    assert!(!output.contains("2026-03-22 10:00:00 WIB"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn quota_reports_include_pool_summary_lines() {
    let alpha = usage_with_main_windows(90, 7_200, 95, 172_800);
    let beta = usage_with_main_windows(45, 1_800, 40, 86_400);
    let last_update = 1_700_000_123;
    let reports = vec![
        QuotaReport {
            name: "alpha".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(alpha.clone())),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "beta".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(beta.clone())),
            fetched_at: last_update,
        },
        QuotaReport {
            name: "api".to_string(),
            active: false,
            auth: AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            workspace_id: None,
            result: Err("auth mode is not quota-compatible".to_string()),
            fetched_at: 1_700_000_090,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 160);
    let five_hour_reset = required_main_window_snapshot(&beta, "5h")
        .expect("5h snapshot")
        .reset_at;
    let weekly_reset = required_main_window_snapshot(&beta, "weekly")
        .expect("weekly snapshot")
        .reset_at;

    assert!(output.contains("Available:"));
    assert!(output.contains("2/3 profile"));
    assert!(output.contains("Last Updated:"));
    assert!(output.contains(&format_precise_reset_time(Some(last_update))));
    assert!(!output.contains("Unavailable:"));
    assert!(output.contains("5h remaining pool:"));
    assert!(output.contains("Weekly remaining pool:"));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(five_hour_reset))));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(weekly_reset))));
    assert!(output.contains("\n\nPROFILE"));
}

#[test]
fn quota_reports_render_copilot_rows_without_falling_back_to_error() {
    let reports = vec![
        QuotaReport {
            name: "main".to_string(),
            active: true,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 7_200, 95, 172_800,
            ))),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "copilot-main".to_string(),
            active: false,
            auth: AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: false,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::Copilot(CopilotUserInfo {
                login: Some("copilot-user".to_string()),
                access_type_sku: Some("free_limited_copilot".to_string()),
                copilot_plan: Some("individual".to_string()),
                endpoints: None,
                limited_user_quotas: BTreeMap::from([
                    ("chat".to_string(), 450),
                    ("completions".to_string(), 4_000),
                ]),
                monthly_quotas: BTreeMap::from([
                    ("chat".to_string(), 500),
                    ("completions".to_string(), 4_000),
                ]),
                limited_user_reset_date: Some("2026-05-09".to_string()),
            })),
            fetched_at: 1_700_000_101,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("Available:"));
    assert!(output.contains("2/2 profile"));
    assert!(output.contains("copilot-main"));
    assert!(output.contains("copilot-user"));
    assert!(output.contains("individual"));
    assert!(output.contains("chat 450/500 left"));
    assert!(output.contains("comp 4000/4000 left"));
    assert!(output.contains("status: Ready"));
    assert!(output.contains("resets: monthly 2026-05-09"));
    assert!(!output.contains("GitHub Copilot profiles do not expose ChatGPT quota"));
}

#[test]
fn quota_reports_window_supports_scroll_offset_and_hint() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                0, 3_600, 80, 86_400,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 7_200, 95, 172_800,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let window = render_quota_reports_window_with_layout(&reports, false, Some(16), 100, 1, true);

    assert_eq!(window.start_profile, 1);
    assert_eq!(window.total_profiles, 4);
    assert_eq!(window.shown_profiles, 2);
    assert_eq!(window.hidden_before, 1);
    assert_eq!(window.hidden_after, 1);
    assert!(window.output.contains("ready-late"));
    assert!(window.output.contains("blocked"));
    assert!(!window.output.contains("ready-early"));
    assert!(
        window
            .output
            .contains("\n\npress Up/Down to scroll profiles (2-3 of 4; 1 above, 1 below)")
    );
}
