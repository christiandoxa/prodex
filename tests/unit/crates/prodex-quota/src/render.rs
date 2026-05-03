use super::*;
use crate::AuthSummary;
use std::collections::BTreeMap;

fn usage_with_main_windows(
    five_hour_remaining: i64,
    five_hour_reset_at: i64,
    weekly_remaining: i64,
    weekly_reset_at: i64,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: Some("plus".to_string()),
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(100 - five_hour_remaining),
                reset_at: Some(five_hour_reset_at),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(100 - weekly_remaining),
                reset_at: Some(weekly_reset_at),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

fn openai_report(name: &str, usage: UsageResponse) -> QuotaReport {
    QuotaReport {
        name: name.to_string(),
        active: false,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        workspace_id: None,
        result: Ok(ProviderQuotaSnapshot::OpenAi(usage)),
        fetched_at: 1_700_000_000,
    }
}

#[test]
fn labels_standard_windows() {
    assert_eq!(window_label(Some(18_000)), "5h");
    assert_eq!(window_label(Some(604_800)), "weekly");
    assert_eq!(window_label(Some(2_592_000)), "monthly");
}

#[test]
fn blocks_missing_required_main_window() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let blocked = collect_blocked_limits(&usage, false);
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].message, "weekly quota unavailable");
}

#[test]
fn compact_window_format_uses_scale_of_100() {
    let window = UsageWindow {
        used_percent: Some(37),
        reset_at: None,
        limit_window_seconds: Some(18_000),
    };

    assert_eq!(format_window_status_compact(&window), "5h 63% left");
    assert!(format_window_status(&window).contains("63% left"));
    assert!(format_window_status(&window).contains("37% used"));
}

#[test]
fn quota_summary_marks_exhausted_window() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(30),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let summary = quota_summary(&usage);
    assert_eq!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    );
    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Exhausted);
}

#[test]
fn profile_quota_render_contains_core_fields() {
    let usage = UsageResponse {
        email: Some("me@example.com".to_string()),
        plan_type: Some("plus".to_string()),
        rate_limit: None,
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let rendered = render_profile_quota_with_width("main", &usage, 80);
    assert!(rendered.contains("Quota main"));
    assert!(rendered.contains("me@example.com"));
    assert!(rendered.contains("5h quota unavailable"));
}

#[test]
fn quota_reports_respect_line_budget_while_preserving_sort_order() {
    let reports = vec![
        openai_report(
            "blocked",
            usage_with_main_windows(0, 1_700_003_600, 80, 1_700_086_400),
        ),
        openai_report(
            "ready-late",
            usage_with_main_windows(90, 1_700_007_200, 95, 1_700_172_800),
        ),
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
        openai_report(
            "ready-early",
            usage_with_main_windows(90, 1_700_001_800, 95, 1_700_259_200),
        ),
    ];

    let output = render_quota_reports_with_line_limit(&reports, false, Some(16));

    assert!(output.contains("ready-early"));
    assert!(output.contains("ready-late"));
    assert!(!output.contains("blocked"));
    assert!(!output.contains("error"));
    assert!(output.contains("\n\nshowing top 2 of 4 profiles"));
}

#[test]
fn quota_reports_window_supports_scroll_offset_and_hint() {
    let reports = vec![
        openai_report(
            "blocked",
            usage_with_main_windows(0, 1_700_003_600, 80, 1_700_086_400),
        ),
        openai_report(
            "ready-late",
            usage_with_main_windows(90, 1_700_007_200, 95, 1_700_172_800),
        ),
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
        openai_report(
            "ready-early",
            usage_with_main_windows(90, 1_700_001_800, 95, 1_700_259_200),
        ),
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

#[test]
fn quota_reports_detail_shows_workspace_only_for_duplicate_account_email() {
    let mut first_usage = usage_with_main_windows(90, 1_700_007_200, 95, 1_700_172_800);
    first_usage.email = Some("same@example.com".to_string());
    let mut second_usage = usage_with_main_windows(80, 1_700_003_600, 88, 1_700_086_400);
    second_usage.email = Some("same@example.com".to_string());
    let mut same_workspace_first_usage =
        usage_with_main_windows(75, 1_700_003_600, 86, 1_700_086_400);
    same_workspace_first_usage.email = Some("same-workspace@example.com".to_string());
    let mut same_workspace_second_usage =
        usage_with_main_windows(74, 1_700_003_600, 85, 1_700_086_400);
    same_workspace_second_usage.email = Some("same-workspace@example.com".to_string());
    let mut solo_usage = usage_with_main_windows(70, 1_700_001_800, 78, 1_700_259_200);
    solo_usage.email = Some("solo@example.com".to_string());
    let mut reports = vec![
        openai_report("workspace-one", first_usage),
        openai_report("workspace-two", second_usage),
        openai_report("same-workspace-one", same_workspace_first_usage),
        openai_report("same-workspace-two", same_workspace_second_usage),
        openai_report("solo", solo_usage),
    ];
    reports[0].workspace_id = Some("acct_workspace_one_123456789".to_string());
    reports[1].workspace_id = Some("acct_workspace_two_987654321".to_string());
    reports[2].workspace_id = Some("acct_same_workspace".to_string());
    reports[3].workspace_id = Some("acct_same_workspace".to_string());
    reports[4].workspace_id = Some("acct_solo".to_string());

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("workspace: acct_workspa...456789"));
    assert!(output.contains("workspace: acct_workspa...654321"));
    assert!(!output.contains("workspace: acct_same_workspace"));
    assert!(!output.contains("workspace: acct_solo"));
}

#[test]
fn quota_reports_render_copilot_rows_without_falling_back_to_error() {
    let reports = vec![
        openai_report(
            "main",
            usage_with_main_windows(90, 1_700_007_200, 95, 1_700_172_800),
        ),
        QuotaReport {
            name: "copilot-main".to_string(),
            active: false,
            auth: AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: false,
            },
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::Copilot(CopilotQuotaInfo {
                login: Some("copilot-user".to_string()),
                access_type_sku: Some("free_limited_copilot".to_string()),
                copilot_plan: Some("individual".to_string()),
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
fn quota_reports_fit_requested_width_in_narrow_layout() {
    let reports = vec![
        openai_report(
            "ready-early",
            usage_with_main_windows(90, 1_700_001_800, 95, 1_700_259_200),
        ),
        openai_report(
            "blocked",
            usage_with_main_windows(0, 1_700_003_600, 80, 1_700_086_400),
        ),
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 72);

    assert!(output.lines().all(|line| text_width(line) <= 72));
}

#[test]
fn quota_reset_at_from_message_parses_ordinal_date() {
    let parsed = quota_reset_at_from_message(
        "You've hit your usage limit. Try again at Mar 24th, 2026 2:04 AM.",
    )
    .expect("reset timestamp should parse");
    let expected = Local
        .with_ymd_and_hms(2026, 3, 24, 2, 4, 0)
        .single()
        .or_else(|| Local.with_ymd_and_hms(2026, 3, 24, 2, 4, 0).earliest())
        .expect("local timestamp should resolve")
        .timestamp();

    assert_eq!(parsed, expected);
}
