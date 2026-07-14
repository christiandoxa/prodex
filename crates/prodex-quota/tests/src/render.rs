use super::*;
use crate::{AdditionalRateLimit, AuthSummary, RateLimitResetCreditsSummary};
use std::collections::BTreeMap;

fn main_windows(
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
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

fn spark_limit(
    five_hour_remaining: i64,
    five_hour_reset_at: i64,
    weekly_remaining: i64,
    weekly_reset_at: i64,
) -> AdditionalRateLimit {
    AdditionalRateLimit {
        limit_name: Some("GPT-5.3-Codex-Spark".to_string()),
        metered_feature: Some("codex_bengalfox".to_string()),
        rate_limit: WindowPair {
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
        },
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
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::OpenAi(usage)),
        fetched_at: 1_700_000_000,
    }
}

fn copilot_report(reset_date: Option<&str>) -> QuotaReport {
    QuotaReport {
        name: "copilot-main".to_string(),
        active: false,
        auth: AuthSummary {
            label: "copilot".to_string(),
            quota_compatible: false,
        },
        workspace_id: None,
        workspace_name: None,
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
            limited_user_reset_date: reset_date.map(str::to_string),
        })),
        fetched_at: 1_700_000_101,
    }
}

fn sorted_names_by(reports: &[QuotaReport], sort: QuotaReportSort) -> Vec<String> {
    sorted_quota_report_indexes_by(reports, sort)
        .into_iter()
        .map(|index| reports[index].name.clone())
        .collect()
}

#[path = "render/spark.rs"]
mod spark;

#[test]
fn labels_standard_windows() {
    assert_eq!(window_label(Some(18_000)), "5h");
    assert_eq!(window_label(Some(604_800)), "weekly");
    assert_eq!(window_label(Some(2_592_000)), "monthly");
}

#[test]
fn missing_main_window_does_not_block_available_window() {
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
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    assert!(collect_blocked_limits(&usage, false).is_empty());
    assert_eq!(format_openai_quota_status(&usage), "Ready");
}

#[test]
fn missing_five_hour_window_does_not_block_available_weekly_window() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: None,
            secondary_window: Some(UsageWindow {
                used_percent: Some(0),
                reset_at: Some(1_700_259_200),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    for plan in [
        "free",
        "plus",
        "pro",
        "team",
        "business",
        "enterprise",
        "edu",
        "future-openai-plan",
    ] {
        let mut plan_usage = usage.clone();
        plan_usage.plan_type = Some(plan.to_string());
        assert!(
            collect_blocked_limits(&plan_usage, false).is_empty(),
            "weekly-only quota should be ready for plan {plan}"
        );
        assert_eq!(format_openai_quota_status(&plan_usage), "Ready");
    }
    let fields = quota_pool_summary_fields(&[openai_report("weekly-only", usage)]);
    assert!(fields.contains(&("Available".to_string(), "1/1 profile".to_string())));
    assert!(fields.contains(&(
        "Usable now".to_string(),
        "weekly 100% across 1 ready profile(s)".to_string(),
    )));
    assert!(fields.contains(&("5h remaining pool".to_string(), "Unavailable".to_string(),)));
    assert!(fields.iter().any(|(label, value)| {
        label == "Weekly remaining pool" && value.starts_with("100% across 1 profile(s)")
    }));
}

#[test]
fn unknown_five_hour_usage_does_not_block_known_weekly_quota() {
    let usage = UsageResponse {
        email: None,
        plan_type: Some("plus".to_string()),
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: None,
                reset_at: None,
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(1),
                reset_at: Some(1_700_259_200),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    assert_eq!(format_openai_quota_status(&usage), "Ready");
    assert!(collect_blocked_limits(&usage, false).is_empty());
}

#[test]
fn unavailable_quota_is_not_reported_as_blocked() {
    let usage = UsageResponse {
        email: None,
        plan_type: Some("plus".to_string()),
        rate_limit: None,
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    assert_eq!(format_openai_quota_status(&usage), "Unavailable");
}

#[test]
fn openai_quota_status_uses_short_blocked_labels() {
    for (usage, expected) in [
        (
            main_windows(0, 1_700_001_800, 80, 1_700_259_200),
            "Blocked 5h",
        ),
        (
            main_windows(80, 1_700_001_800, 0, 1_700_259_200),
            "Blocked weekly",
        ),
    ] {
        assert_eq!(format_openai_quota_status(&usage), expected);
    }
    assert_eq!(
        format_quota_error_status("HTTP 401"),
        "Blocked unauthorized"
    );
    assert_eq!(
        format_quota_error_detail("HTTP 401 unauthorized"),
        "error: unauthorized"
    );
    assert_eq!(
        format_quota_error_status("quota unavailable in test"),
        "Error unavailable"
    );
    assert_eq!(
        format_quota_error_detail("quota unavailable in test"),
        "error: unavailable"
    );
    for (input, expected) in [
        ("HTTP 500 internal server error", "Error server"),
        ("dns lookup failed", "Error network"),
        ("invalid token in auth file", "Error invalid auth"),
        ("failed to parse quota json", "Error parse"),
        ("missing config.toml", "Error config"),
        ("proxy handshake failed", "Error proxy"),
        ("empty quota response body", "Error empty"),
        ("request cancelled", "Error cancelled"),
        ("", "Error unknown"),
    ] {
        assert_eq!(format_quota_error_status(input), expected);
    }
}

#[test]
fn compact_window_format_uses_scale_of_100() {
    let window = UsageWindow {
        used_percent: Some(37),
        reset_at: None,
        limit_window_seconds: Some(18_000),
    };

    assert_eq!(format_window_status_compact(&window), "5h 63%");
    assert!(format_window_status(&window).contains("63% left"));
    assert!(format_window_status(&window).contains("37% used"));
}

#[test]
fn openai_quota_renders_rate_limit_reset_credits() {
    let mut usage = main_windows(0, 1_700_001_800, 80, 1_700_259_200);
    usage.rate_limit_reset_credits = Some(RateLimitResetCreditsSummary { available_count: 2 });

    let panel =
        render_profile_quota_snapshot("main", &ProviderQuotaSnapshot::OpenAi(usage.clone()));
    assert!(panel.contains("Reset credits"));
    assert!(panel.contains("2 available"));

    let overview =
        render_quota_reports_with_layout(&[openai_report("main", usage)], true, None, 160);
    assert!(overview.contains("reset credits: 2 available"));
}

#[test]
fn quota_reports_put_status_in_column_and_resets_on_left_detail_line() {
    let usage = main_windows(20, 1_700_001_800, 10, 1_700_259_200);

    let output = render_quota_reports_with_layout(&[openai_report("main", usage)], true, None, 100);
    let mut lines = output.lines();
    let header = lines.find(|line| line.contains("PROFILE")).expect("header");
    let row = lines.find(|line| line.contains("main")).expect("row");
    let resets = lines
        .find(|line| line.starts_with("resets:"))
        .expect("resets");

    assert!(header.contains("STATUS"));
    assert!(row.contains("Ready"));
    assert!(!output.contains("status: Ready"));
    assert!(resets.contains("5h"));
}

#[test]
fn openai_quota_deserializes_rate_limit_reset_credits() {
    let camel_usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "email": "user@example.com",
        "plan_type": "plus",
        "rate_limit": null,
        "code_review_rate_limit": null,
        "rate_limit_reset_credits": {
            "availableCount": 3
        }
    }))
    .expect("usage response should deserialize reset credits");

    let camel_credits = camel_usage
        .rate_limit_reset_credits
        .as_ref()
        .expect("camel reset credits");
    assert_eq!(camel_credits.available_count, 3);

    let snake_usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "rate_limit_reset_credits": {
            "available_count": 4
        }
    }))
    .expect("usage response should deserialize backend reset credits");

    let snake_credits = snake_usage
        .rate_limit_reset_credits
        .as_ref()
        .expect("snake reset credits");
    assert_eq!(snake_credits.available_count, 4);
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
        rate_limit_reset_credits: None,
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

#[test]
fn profile_quota_render_contains_core_fields() {
    let usage = UsageResponse {
        email: Some("me@example.com".to_string()),
        plan_type: Some("plus".to_string()),
        rate_limit: None,
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    let rendered = render_profile_quota_with_width("main", &usage, 80);
    assert!(rendered.contains("Quota main"));
    assert!(rendered.contains("me@example.com"));
    assert!(rendered.contains("Unavailable"));
}

#[test]
fn profile_quota_render_shows_monthly_workspace_limits() {
    let mut usage = main_windows(82, 1_700_001_800, 91, 1_700_259_200);
    usage.additional_rate_limits.push(AdditionalRateLimit {
        limit_name: Some("Workspace credits".to_string()),
        metered_feature: Some("codex".to_string()),
        rate_limit: WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(27),
                reset_at: Some(1_702_592_000),
                limit_window_seconds: Some(2_592_000),
            }),
            secondary_window: None,
        },
    });

    let rendered = render_profile_quota_with_width("main", &usage, 120);

    assert!(rendered.contains("Workspace credits monthly"));
    assert!(rendered.contains("73% left"));
}

#[test]
fn quota_report_sort_modes_order_by_selected_columns() {
    let mut beta_usage = main_windows(90, 1_700_007_200, 95, 1_700_172_800);
    beta_usage.email = Some("alpha@example.com".to_string());
    beta_usage.plan_type = Some("basic".to_string());
    let mut alpha_usage = main_windows(90, 1_700_001_800, 95, 1_700_259_200);
    alpha_usage.email = Some("zeta@example.com".to_string());
    alpha_usage.plan_type = Some("plus".to_string());
    let mut reports = vec![
        openai_report("beta", beta_usage),
        openai_report("alpha", alpha_usage),
    ];
    reports[0].auth.label = "zzz".to_string();
    reports[1].auth.label = "aaa".to_string();
    reports[0].active = true;

    assert_eq!(
        sorted_names_by(&reports, QuotaReportSort::Current),
        vec!["beta", "alpha"]
    );
    assert_eq!(
        sorted_names_by(&reports, QuotaReportSort::Remaining),
        vec!["alpha", "beta"]
    );
    assert_eq!(
        sorted_names_by(&reports, QuotaReportSort::Profile),
        vec!["alpha", "beta"]
    );
    assert_eq!(
        sorted_names_by(&reports, QuotaReportSort::Auth),
        vec!["alpha", "beta"]
    );
    assert_eq!(
        sorted_names_by(&reports, QuotaReportSort::Account),
        vec!["beta", "alpha"]
    );
    assert_eq!(
        sorted_names_by(&reports, QuotaReportSort::Plan),
        vec!["beta", "alpha"]
    );
}

#[test]
fn current_sort_places_blocked_weekly_below_blocked_five_hour() {
    let reports = vec![
        openai_report("ready", main_windows(80, 1_700_000_300, 70, 1_700_000_400)),
        openai_report(
            "weekly-blocked",
            main_windows(67, 1_700_000_100, 0, 1_700_000_050),
        ),
        openai_report(
            "five-hour-blocked",
            main_windows(0, 1_700_000_200, 84, 1_700_000_300),
        ),
    ];

    assert_eq!(
        sorted_names_by(&reports, QuotaReportSort::Current),
        vec!["ready", "five-hour-blocked", "weekly-blocked"]
    );
}

#[test]
fn quota_reports_respect_line_budget_while_preserving_sort_order() {
    let reports = vec![
        openai_report("blocked", main_windows(0, 1_700_003_600, 80, 1_700_086_400)),
        openai_report(
            "ready-late",
            main_windows(90, 1_700_007_200, 95, 1_700_172_800),
        ),
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            workspace_name: None,
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        openai_report(
            "ready-early",
            main_windows(90, 1_700_001_800, 95, 1_700_259_200),
        ),
    ];

    let output = render_quota_reports_with_layout(&reports, false, Some(15), 90);

    assert!(output.contains("ready-early"));
    assert!(output.contains("ready-late"));
    assert!(!output.contains("blocked"));
    assert!(!output.contains("error"));
    assert!(output.contains("\n\nshowing top 2 of 4 profiles"));
}

#[test]
fn quota_reports_window_supports_scroll_offset_and_hint() {
    let reports = vec![
        openai_report("blocked", main_windows(0, 1_700_003_600, 80, 1_700_086_400)),
        openai_report(
            "ready-late",
            main_windows(90, 1_700_007_200, 95, 1_700_172_800),
        ),
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            workspace_name: None,
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        openai_report(
            "ready-early",
            main_windows(90, 1_700_001_800, 95, 1_700_259_200),
        ),
    ];

    let window = render_quota_reports_window_with_layout(&reports, false, Some(16), 100, 1, true);

    assert_eq!(window.start_profile, 1);
    assert_eq!(window.total_profiles, 4);
    assert_eq!(window.shown_profiles, 3);
    assert_eq!(window.hidden_before, 1);
    assert_eq!(window.hidden_after, 0);
    assert!(window.output.contains("ready-late"));
    assert!(window.output.contains("blocked"));
    assert!(window.output.contains("error"));
    assert!(!window.output.contains("ready-early"));
    assert!(
        window
            .output
            .contains("\n\npress Up/Down to scroll profiles (2-4 of 4; 1 above, 0 below)")
    );
}

#[test]
fn quota_reports_detail_shows_workspace_for_all_openai_profiles() {
    let mut first_usage = main_windows(90, 1_700_007_200, 95, 1_700_172_800);
    first_usage.email = Some("same@example.com".to_string());
    let mut second_usage = main_windows(80, 1_700_003_600, 88, 1_700_086_400);
    second_usage.email = Some("same@example.com".to_string());
    let mut same_workspace_first_usage = main_windows(75, 1_700_003_600, 86, 1_700_086_400);
    same_workspace_first_usage.email = Some("same-workspace@example.com".to_string());
    let mut same_workspace_second_usage = main_windows(74, 1_700_003_600, 85, 1_700_086_400);
    same_workspace_second_usage.email = Some("same-workspace@example.com".to_string());
    let mut solo_usage = main_windows(70, 1_700_001_800, 78, 1_700_259_200);
    solo_usage.email = Some("solo@example.com".to_string());
    let mut reports = vec![
        openai_report("workspace-one", first_usage),
        openai_report("workspace-two", second_usage),
        openai_report("same-workspace-one", same_workspace_first_usage),
        openai_report("same-workspace-two", same_workspace_second_usage),
        openai_report("solo", solo_usage),
    ];
    reports[0].workspace_id = Some("acct_workspace_one_123456789".to_string());
    reports[0].workspace_name = Some("Personal".to_string());
    reports[1].workspace_id = Some("acct_workspace_two_987654321".to_string());
    reports[2].workspace_id = Some("acct_same_workspace".to_string());
    reports[3].workspace_id = Some("acct_same_workspace".to_string());
    reports[4].workspace_id = Some("acct_solo".to_string());

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("workspace: Personal"));
    assert!(!output.contains("workspace: acct_workspa...456789"));
    assert!(output.contains("workspace: acct_workspa...654321"));
    assert!(output.contains("workspace: acct_same_workspace"));
    assert!(output.contains("workspace: acct_solo"));
}

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

#[test]
fn quota_reports_fit_requested_width_in_narrow_layout() {
    let reports = vec![
        openai_report(
            "ready-early",
            main_windows(90, 1_700_001_800, 95, 1_700_259_200),
        ),
        openai_report("blocked", main_windows(0, 1_700_003_600, 80, 1_700_086_400)),
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 72);

    assert!(output.lines().all(|line| text_width(line) <= 72));
}

#[test]
fn quota_reports_do_not_force_sixty_column_width() {
    let output = render_quota_reports_with_layout(
        &[openai_report(
            "表🙂profile",
            main_windows(90, 1_700_007_200, 95, 1_700_172_800),
        )],
        false,
        None,
        40,
    );

    assert!(output.lines().take(4).all(|line| text_width(line) <= 40));
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

#[test]
fn quota_reset_at_from_message_parses_usage_limit_json() {
    let parsed = quota_reset_at_from_message(
        r#"{"type":"error","error":{"type":"usage_limit_reached","message":"The usage limit has been reached","resets_at":1780245646},"headers":{"X-Codex-Primary-Reset-At":"1780241111"}}"#,
    )
    .expect("reset timestamp should parse from usage limit JSON");

    assert_eq!(parsed, 1_780_245_646);
}

#[test]
fn quota_reset_at_from_message_parses_codex_reset_headers() {
    let parsed = quota_reset_at_from_message(
        r#"{"status_code":429,"headers":{"X-Codex-Primary-Used-Percent":"100","X-Codex-Primary-Reset-At":"1780245646","X-Codex-Secondary-Used-Percent":"91","X-Codex-Secondary-Reset-At":"1780243339"}}"#,
    )
    .expect("reset timestamp should parse from Codex reset headers");

    assert_eq!(parsed, 1_780_245_646);
}
