    use ratatui::style::Color;

    #[test]
    fn quota_watch_overview_height_has_no_extra_spacer_line() {
        assert_eq!(quota_watch_overview_height(5, 20), 5);
        assert_eq!(quota_watch_overview_height(5, 4), 4);
    }

    #[test]
    fn quota_watch_separator_line_connects_outer_borders() {
        assert_eq!(quota_watch_separator_line(0), "");
        assert_eq!(quota_watch_separator_line(1), "─");
        assert_eq!(quota_watch_separator_line(2), "├┤");
        assert_eq!(quota_watch_separator_line(5), "├───┤");
    }

    #[test]
    fn all_quota_watch_tui_frame_uses_explicit_layout_and_metadata() {
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "2026-06-26 10:00:00 UTC".to_string(),
            profile_count: 1,
            reports: vec![test_openai_quota_report(test_openai_usage_with_windows(
                20,
                10,
                1_700_001_800,
            ))],
        };

        let frame = build_all_quota_watch_tui_frame(
            &snapshot,
            AllQuotaWatchLayout {
                detail: false,
                scroll_offset: 0,
                sort: QuotaReportSort::Current,
                provider_filter: QuotaProviderFilter::All,
                provider_filter_locked: false,
                total_width: 80,
                max_lines: Some(20),
            },
        );

        assert_eq!(frame.title, "Prodex Quota");
        assert_eq!(frame.body, "");
        assert!(frame.overview_fields.iter().any(|(label, value)| {
            label == "Available" && value.contains(" | updated ")
        }));
        assert!(!frame
            .overview_fields
            .iter()
            .any(|(label, _)| label == "Last Updated"));
        assert!(!frame.body.contains("[ Quota Overview ]"));
        assert!(!frame.body.contains("Quota Overview ] ==="));
        assert!(!frame.body.contains("PROFILE"));
        let table = frame
            .table
            .as_ref()
            .expect("all quota TUI frame should include a ratatui table");
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].profile, vec!["main".to_string()]);
        assert_eq!(table.rows[0].account, vec!["main@example.com".to_string()]);
        assert_eq!(table.rows[0].status, vec!["Ready".to_string()]);
        assert_eq!(table.rows[0].remaining, vec!["5h 80% | weekly 90%".to_string()]);
        assert!(!frame.body.contains("sort: current"));
        assert!(frame.footer.contains("u update"));
        assert!(frame.footer.contains("sort current"));
        assert!(frame.footer.contains("filter all"));
    }

    #[test]
    fn all_quota_watch_tui_detail_uses_table_height_for_visible_rows() {
        let reports = (0..9)
            .map(|index| {
                let mut report = test_openai_quota_report(test_openai_usage_with_windows(
                    20,
                    10,
                    1_700_001_800,
                ));
                report.name = format!("profile-{index}");
                report
            })
            .collect::<Vec<_>>();
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
                max_lines: quota_watch_tui_table_lines(30, 5),
            },
        );

        assert_eq!(frame.table.as_ref().expect("table").rows.len(), 6);
    }

    #[test]
    fn all_quota_watch_tui_detail_scroll_reaches_last_profiles() {
        let reports = (0..4)
            .map(|index| {
                let mut report = test_openai_quota_report(test_openai_usage_with_windows(
                    20,
                    10,
                    1_700_001_800,
                ));
                report.name = format!("profile-{index}");
                report
            })
            .collect::<Vec<_>>();
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "2026-06-26 10:00:00 UTC".to_string(),
            profile_count: reports.len(),
            reports,
        };

        let max_scroll = quota_watch_tui_max_scroll_offset_for_snapshot(
            &snapshot,
            true,
            QuotaProviderFilter::All,
            QuotaReportSort::Profile,
            Some(7),
        );
        let frame = build_all_quota_watch_tui_frame(
            &snapshot,
            AllQuotaWatchLayout {
                detail: true,
                scroll_offset: max_scroll,
                sort: QuotaReportSort::Profile,
                provider_filter: QuotaProviderFilter::All,
                provider_filter_locked: false,
                total_width: 100,
                max_lines: Some(7),
            },
        );

        let rows = &frame.table.as_ref().expect("table").rows;
        assert_eq!(max_scroll, 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].profile, vec!["profile-2".to_string()]);
        assert_eq!(rows[1].profile, vec!["profile-3".to_string()]);
    }

    #[test]
    fn quota_human_tui_spans_use_readable_detail_color() {
        let spans = quota_human_tui_spans(
            "  resets: 5h 2026-06-28 01:44:03 | weekly 2026-07-04 10:12:48",
        );

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style.fg, Some(Color::Gray));
    }

    #[test]
    fn all_quota_watch_tui_loading_and_error_use_native_fields() {
        let layout = AllQuotaWatchLayout {
            detail: false,
            scroll_offset: 0,
            sort: QuotaReportSort::Current,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: 80,
            max_lines: Some(20),
        };
        let loading = build_all_quota_watch_tui_frame(
            &AllQuotaWatchSnapshot::Loading {
                updated: "2026-06-26 10:00:00 UTC".to_string(),
            },
            layout,
        );
        assert_eq!(loading.body, "Quota");
        assert!(loading.table.is_none());
        assert!(loading
            .overview_fields
            .iter()
            .any(|(label, value)| label == "Status" && value == "Loading quota data..."));
        assert!(!loading.body.contains("[ Quota ]"));

        let error = build_all_quota_watch_tui_frame(
            &AllQuotaWatchSnapshot::Error {
                updated: "2026-06-26 10:00:01 UTC".to_string(),
                message: "load failed".to_string(),
            },
            layout,
        );
        assert_eq!(error.body, "Quota");
        assert!(error.table.is_none());
        assert!(error
            .overview_fields
            .iter()
            .any(|(label, value)| label == "Status" && value == "load failed"));
        assert!(!error.body.contains("[ Quota ]"));
    }

    #[test]
    fn profile_quota_watch_tui_frame_uses_ratatui_metadata() {
        let frame = build_profile_quota_watch_tui_frame(
            "main",
            "2026-06-26 10:00:00 UTC",
            Ok(test_usage("main@example.com")),
        );

        assert_eq!(frame.title, "Prodex Quota main");
        assert_eq!(frame.body, "Quota main");
        assert!(!frame.body.contains("[ Quota main ]"));
        assert!(frame
            .overview_fields
            .iter()
            .any(|(label, value)| label == "Account" && value == "main@example.com"));
        assert!(frame
            .overview_fields
            .iter()
            .any(|(label, _)| label == "Last Updated"));
        assert!(frame.table.is_none());
        assert_eq!(frame.footer, "refresh 5s | q quit");
    }

    #[test]
    fn profile_quota_watch_tui_error_uses_native_fields() {
        let frame = build_profile_quota_watch_tui_frame(
            "main",
            "2026-06-26 10:00:00 UTC",
            Err("quota failed".to_string()),
        );

        assert_eq!(frame.body, "Quota main");
        assert!(frame.table.is_none());
        assert!(frame
            .overview_fields
            .iter()
            .any(|(label, value)| label == "Status" && value == "quota failed"));
        assert!(!frame.body.contains("[ Quota main ]"));
    }

