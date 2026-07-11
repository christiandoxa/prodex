#[test]
fn quota_watch_table_starts_rows_without_header_padding() {
    let table = AllQuotaWatchTuiTable {
        rows: vec![AllQuotaWatchTuiRow {
            profile: vec!["main".to_string()],
            current: vec!["".to_string()],
            auth: vec!["chatgpt".to_string()],
            account: vec!["main@example.com".to_string()],
            plan: vec!["plus".to_string()],
            status: vec!["Ready".to_string()],
            remaining: vec!["5h 80% | weekly 90%".to_string()],
            detail: Vec::new(),
        }],
    };

    let text = quota_watch_table_text(&table);
    assert!(format!("{:?}", text.lines[0]).contains("PROFILE"));
    assert!(!format!("{:?}", text.lines[0]).contains("|"));
    assert!(format!("{:?}", text.lines[1]).contains("main"));
    assert_eq!(text.lines.len(), 2);
    assert!(text.lines[1]
        .spans
        .iter()
        .all(|span| span.content.as_ref() != " | "));
}

#[test]
fn all_quota_watch_tui_detail_keeps_profile_column_clean() {
    let mut usage = test_openai_usage_with_windows(20, 10, 1_700_086_400);
    usage.rate_limit_reset_credits = Some(prodex_quota::RateLimitResetCreditsSummary {
        available_count: 1,
    });
    let mut reports = vec![test_openai_quota_report(usage)];
    reports[0].name = "workspace-alpha".to_string();
    reports[0].workspace_id = Some("acct_workspace_one_123456789".to_string());
    reports[0].workspace_name = Some("Personal".to_string());
    reports.push(test_openai_quota_report(test_openai_usage_with_windows(
        25,
        15,
        1_700_086_400,
    )));
    reports[1].name = "workspace-beta".to_string();
    reports[1].workspace_id = Some("acct_workspace_two_987654321".to_string());

    let snapshot = AllQuotaWatchSnapshot::Reports {
        updated: "2026-06-26 10:00:00 UTC".to_string(),
        profile_count: reports.len(),
        reports,
    };

    let frame = build_all_quota_watch_tui_frame(
        &snapshot,
        AllQuotaWatchLayout {
            detail: true,
            scroll_offset: 0,
            sort: QuotaReportSort::Current,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: 100,
            max_lines: Some(20),
        },
    );

    let row = &frame.table.as_ref().expect("table").rows[0];
    assert_eq!(row.profile, vec!["workspace-alpha".to_string()]);
    assert_eq!(row.remaining[0], "5h 80% | weekly 90%");
    assert_eq!(row.detail.len(), 1);
    assert!(row.detail[0].starts_with("resets: 5h "));
    assert!(row.detail[0].contains(" | weekly "));
    assert!(row.detail[0].contains(" | workspace: Personal"));
    assert!(!row.detail[0].contains("workspace-alpha"));
    assert!(!row.detail[0].contains("acct_workspace"));
    assert!(row.detail[0].contains(" | reset credits: 1 available"));
    assert!(!row.detail[0].contains("expires"));

    let text = quota_watch_table_text(frame.table.as_ref().expect("table"));
    let main_line = &text.lines[1];
    assert_eq!(main_line.spans[0].style.fg, None);
    assert_eq!(
        main_line
            .spans
            .iter()
            .find(|span| span.content.contains("Ready"))
            .expect("Ready status span")
            .style
            .fg,
        Some(Color::Green)
    );
    let detail_line = &text.lines[2];
    assert_eq!(detail_line.style.fg, Some(Color::Gray));
}

#[test]
fn all_quota_watch_tui_detail_shows_spark_limit() {
    let mut usage = test_openai_usage_with_windows(100, 100, 1_700_086_400);
    usage
        .additional_rate_limits
        .push(prodex_quota::AdditionalRateLimit {
            limit_name: Some("GPT-5.3-Codex-Spark".to_string()),
            metered_feature: Some("codex_bengalfox".to_string()),
            rate_limit: WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(11),
                    reset_at: Some(1_783_413_134),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(3),
                    reset_at: Some(1_783_999_934),
                    limit_window_seconds: Some(604_800),
                }),
            },
        });
    let snapshot = AllQuotaWatchSnapshot::Reports {
        updated: "2026-06-26 10:00:00 UTC".to_string(),
        profile_count: 1,
        reports: vec![test_openai_quota_report(usage)],
    };

    let frame = build_all_quota_watch_tui_frame(
        &snapshot,
        AllQuotaWatchLayout {
            detail: true,
            scroll_offset: 0,
            sort: QuotaReportSort::Current,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: 100,
            max_lines: Some(20),
        },
    );

    let row = &frame.table.as_ref().expect("table").rows[0];
    assert_eq!(row.status, vec!["Ready".to_string()]);
    assert!(row.detail[0].contains("GPT-5.3-Codex-Spark: 5h 89% | weekly 97%"));
    assert!(frame.overview_fields.iter().any(|(label, value)| {
        label == "Spark remaining pool" && value.contains("5h 89% | weekly 97%")
    }));
}

#[test]
fn all_quota_watch_tui_status_stays_ready_when_only_spark_is_blocked() {
    let mut usage = test_openai_usage_with_windows(50, 63, 1_783_413_134);
    usage
        .additional_rate_limits
        .push(prodex_quota::AdditionalRateLimit {
            limit_name: Some("GPT-5.3-Codex-Spark".to_string()),
            metered_feature: Some("codex_bengalfox".to_string()),
            rate_limit: WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(100),
                    reset_at: Some(1_783_413_134),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(31),
                    reset_at: Some(1_783_999_934),
                    limit_window_seconds: Some(604_800),
                }),
            },
        });
    let snapshot = AllQuotaWatchSnapshot::Reports {
        updated: "2026-06-26 10:00:00 UTC".to_string(),
        profile_count: 1,
        reports: vec![test_openai_quota_report(usage)],
    };

    let frame = build_all_quota_watch_tui_frame(
        &snapshot,
        AllQuotaWatchLayout {
            detail: true,
            scroll_offset: 0,
            sort: QuotaReportSort::Current,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: 100,
            max_lines: Some(20),
        },
    );

    let row = &frame.table.as_ref().expect("table").rows[0];
    assert_eq!(row.status, vec!["Ready".to_string()]);
    assert_eq!(row.remaining, vec!["5h 50% | weekly 37%".to_string()]);
    assert!(row.detail[0].contains("GPT-5.3-Codex-Spark: 5h 0% | weekly 69%"));
    assert!(frame.overview_fields.iter().any(|(label, value)| {
        label == "Available" && value.contains("1/1 profile")
    }));
    assert!(frame.overview_fields.iter().any(|(label, value)| {
        label == "Usable now" && value.contains("5h 50% | weekly 37% across 1 ready profile(s)")
    }));
}

#[test]
fn all_quota_watch_tui_error_detail_replaces_resets() {
    let snapshot = AllQuotaWatchSnapshot::Reports {
        updated: "2026-06-26 10:00:00 UTC".to_string(),
        profile_count: 1,
        reports: vec![test_quota_report(
            "broken",
            Err("request failed (HTTP 401): unauthorized".to_string()),
        )],
    };

    let frame = build_all_quota_watch_tui_frame(
        &snapshot,
        AllQuotaWatchLayout {
            detail: true,
            scroll_offset: 0,
            sort: QuotaReportSort::Current,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: 100,
            max_lines: Some(20),
        },
    );

    let row = &frame.table.as_ref().expect("table").rows[0];
    assert_eq!(row.status, vec!["Blocked unauthorized".to_string()]);
    assert_eq!(row.detail, vec!["error: unauthorized".to_string()]);
    assert!(!row.detail[0].contains("resets:"));
}
