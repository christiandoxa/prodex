    use super::*;
    use ratatui::style::Color;

    fn test_quota_report(
        name: &str,
        result: std::result::Result<ProviderQuotaSnapshot, String>,
    ) -> QuotaReport {
        test_quota_report_with_provider(name, ProfileProvider::Openai, result)
    }

    fn test_quota_report_with_provider(
        name: &str,
        provider: ProfileProvider,
        result: std::result::Result<ProviderQuotaSnapshot, String>,
    ) -> QuotaReport {
        QuotaReport {
            name: name.to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            provider,
            workspace_id: None,
            workspace_name: None,
            result,
            fetched_at: 1_700_000_000,
        }
    }

    fn test_usage(email: &str) -> ProviderQuotaSnapshot {
        ProviderQuotaSnapshot::OpenAi(UsageResponse {
            email: Some(email.to_string()),
            plan_type: Some("plus".to_string()),
            rate_limit: None,
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: Vec::new(),
        })
    }

    fn test_openai_usage_with_windows(
        five_hour_used_percent: i64,
        weekly_used_percent: i64,
        reset_at: i64,
    ) -> UsageResponse {
        UsageResponse {
            email: Some("main@example.com".to_string()),
            plan_type: Some("plus".to_string()),
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(five_hour_used_percent),
                    reset_at: Some(reset_at),
                    limit_window_seconds: Some(5 * 60 * 60),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(weekly_used_percent),
                    reset_at: Some(reset_at),
                    limit_window_seconds: Some(7 * 24 * 60 * 60),
                }),
            }),
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: Vec::new(),
        }
    }

    fn test_openai_quota_report(usage: UsageResponse) -> QuotaReport {
        test_quota_report("main", Ok(ProviderQuotaSnapshot::OpenAi(usage)))
    }
    fn assert_refresh_interval_near(actual: Duration, expected_seconds: u64) {
        let actual_seconds = actual.as_secs();
        let jitter = (expected_seconds / 10).max(1);
        assert!(
            actual_seconds >= expected_seconds.saturating_sub(jitter)
                && actual_seconds <= expected_seconds.saturating_add(jitter),
            "expected {actual_seconds}s to be within +/-{jitter}s of {expected_seconds}s"
        );
    }

    fn test_gemini_quota(email: &str) -> ProviderQuotaSnapshot {
        ProviderQuotaSnapshot::Gemini(GeminiQuotaInfo {
            email: Some(email.to_string()),
            plan: Some("pro".to_string()),
            project_id: Some("test-project".to_string()),
            buckets: Vec::new(),
        })
    }

    #[test]
    fn quota_watch_footer_strips_duplicate_scroll_notice() {
        let output = quota_watch_without_interactive_scroll_notice(
            "quota body\n\npress Up/Down to scroll profiles (2-3 of 4; 1 above, 1 below)",
        );

        assert_eq!(output, "quota body");
    }

    #[test]
    fn quota_watch_scroll_range_uses_window_metadata() {
        let range = quota_watch_scroll_range(&RenderedQuotaReportWindow {
            output: String::new(),
            shown_profiles: 3,
            total_profiles: 12,
            start_profile: 9,
            hidden_before: 9,
            hidden_after: 0,
        });

        assert_eq!(range.as_deref(), Some("10-12/12; 9 above, 0 below"));
    }

    #[test]
    fn quota_watch_overview_height_has_no_extra_spacer_line() {
        assert_eq!(quota_watch_overview_height(5, 20), 6);
        assert_eq!(quota_watch_overview_height(5, 4), 4);
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
        assert_eq!(frame.body, "Quota Overview");
        assert!(frame
            .overview_fields
            .iter()
            .any(|(label, _)| label == "Last Updated"));
        assert!(frame
            .overview_fields
            .iter()
            .any(|(label, _)| label == "Available"));
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

    #[test]
    fn quota_watch_max_scroll_offset_stops_at_last_visible_window() {
        let reports = (0..4)
            .map(|index| test_quota_report(&format!("profile-{index}"), Ok(test_usage("main"))))
            .collect::<Vec<_>>();

        let max_scroll = quota_watch_max_scroll_offset_for_reports_with_layout(
            &reports,
            false,
            QuotaProviderFilter::All,
            QuotaReportSort::Remaining,
            Some(14),
            100,
        );
        let filtered_reports = filter_quota_reports_by_provider(&reports, QuotaProviderFilter::All);
        let window = render_quota_reports_window_with_sort(
            &filtered_reports,
            false,
            Some(14),
            100,
            max_scroll,
            true,
            QuotaReportSort::Remaining,
        );

        assert_eq!(window.hidden_after, 0);
        assert!(window.hidden_before > 0);
        assert!(matches!(
            apply_quota_watch_command(QuotaWatchCommand::Down, max_scroll, max_scroll),
            QuotaWatchCommandOutcome::Continue(next) if next == max_scroll
        ));
    }

    #[test]
    fn all_quota_watch_refresh_keeps_single_background_refresh_in_flight() {
        let mut refresh = AllQuotaWatchRefresh::new();
        let (release_sender, release_receiver) = mpsc::channel();

        assert!(refresh.try_start(move || {
            release_receiver
                .recv()
                .expect("test refresh should be released");
            AllQuotaWatchSnapshot::Empty {
                updated: "done".to_string(),
            }
        }));
        assert!(!refresh.try_start(|| AllQuotaWatchSnapshot::Empty {
            updated: "second".to_string(),
        }));
        assert!(refresh.take_latest().is_none());

        release_sender
            .send(())
            .expect("test refresh release should send");
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut completed = None;
        while Instant::now() < deadline {
            if let Some(snapshot) = refresh.take_latest() {
                completed = Some(snapshot);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(matches!(
            completed,
            Some(AllQuotaWatchSnapshot::Empty { .. })
        ));
        assert!(refresh.try_start(|| AllQuotaWatchSnapshot::Empty {
            updated: "third".to_string(),
        }));
    }

    #[test]
    fn all_quota_watch_refresh_interval_slows_stable_detail_views() {
        let now = Local::now().timestamp();
        let reset_at = now + 60 * 60;
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "stable".to_string(),
            profile_count: 1,
            reports: vec![test_openai_quota_report(test_openai_usage_with_windows(
                10, 20, reset_at,
            ))],
        };

        assert_refresh_interval_near(
            all_quota_watch_refresh_interval(&snapshot, true, now),
            ALL_QUOTA_WATCH_DETAIL_STABLE_INTERVAL_SECONDS,
        );
        assert_refresh_interval_near(
            all_quota_watch_refresh_interval(&snapshot, false, now),
            ALL_QUOTA_WATCH_STABLE_INTERVAL_SECONDS,
        );
    }

    #[test]
    fn all_quota_watch_refresh_interval_stays_fast_for_blocked_accounts() {
        let now = Local::now().timestamp();
        let reset_at = now + 60 * 60;
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "blocked".to_string(),
            profile_count: 1,
            reports: vec![test_openai_quota_report(test_openai_usage_with_windows(
                100, 20, reset_at,
            ))],
        };

        assert_refresh_interval_near(
            all_quota_watch_refresh_interval(&snapshot, true, now),
            ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS,
        );
    }

    #[test]
    fn all_quota_watch_refresh_interval_stays_fast_for_reset_credits() {
        let now = Local::now().timestamp();
        let reset_at = now + 60 * 60;
        let mut usage = test_openai_usage_with_windows(10, 20, reset_at);
        usage.rate_limit_reset_credits = Some(prodex_quota::RateLimitResetCreditsSummary {
            available_count: 1,
        });
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "credits".to_string(),
            profile_count: 1,
            reports: vec![test_openai_quota_report(usage)],
        };

        assert_refresh_interval_near(
            all_quota_watch_refresh_interval(&snapshot, true, now),
            ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS,
        );
    }

    #[test]
    fn all_quota_watch_refresh_interval_stays_fast_near_reset() {
        let now = Local::now().timestamp();
        let reset_at = now + ALL_QUOTA_WATCH_IMMINENT_RESET_SECONDS - 1;
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "near-reset".to_string(),
            profile_count: 1,
            reports: vec![test_openai_quota_report(test_openai_usage_with_windows(
                10, 20, reset_at,
            ))],
        };

        assert_eq!(
            all_quota_watch_refresh_interval(&snapshot, true, now),
            Duration::from_secs(ALL_QUOTA_WATCH_IMMINENT_INTERVAL_SECONDS)
        );
    }

    #[test]
    fn all_quota_watch_refresh_interval_scales_with_profile_count() {
        let now = Local::now().timestamp();
        let reset_at = now + 60 * 60;
        let reports = (0..50)
            .map(|index| {
                test_quota_report(
                    &format!("profile-{index}"),
                    Ok(ProviderQuotaSnapshot::OpenAi(test_openai_usage_with_windows(
                        10, 20, reset_at,
                    ))),
                )
            })
            .collect::<Vec<_>>();
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "many".to_string(),
            profile_count: reports.len(),
            reports,
        };

        assert_refresh_interval_near(
            all_quota_watch_refresh_interval(&snapshot, true, now),
            50 * ALL_QUOTA_WATCH_PROFILE_SCALE_SECONDS,
        );
    }

    #[test]
    fn quota_watch_update_command_forces_refresh_action() {
        assert!(matches!(
            apply_quota_watch_command(QuotaWatchCommand::Update, 0, 0),
            QuotaWatchCommandOutcome::Update
        ));
    }

    #[test]
    fn quota_watch_quit_key_accepts_ctrl_c_and_ctrl_z() {
        assert!(quota_watch_quit_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(quota_watch_quit_key(KeyEvent::new(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL,
        )));
        assert!(quota_watch_quit_key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(quota_watch_quit_key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )));
        assert!(!quota_watch_quit_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::NONE,
        )));
        assert!(!quota_watch_quit_key(KeyEvent::new(
            KeyCode::Char('z'),
            KeyModifiers::NONE,
        )));
    }

    #[test]
    fn all_quota_watch_merge_preserves_previous_successful_report_on_transient_refresh_error() {
        let previous = AllQuotaWatchSnapshot::Reports {
            updated: "before".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report(
                "main",
                Ok(test_usage("main@example.com")),
            )],
        };
        let next = AllQuotaWatchSnapshot::Reports {
            updated: "after".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report("main", Err("HTTP 503".to_string()))],
        };

        let merged = merge_all_quota_watch_snapshot(&previous, next);

        let AllQuotaWatchSnapshot::Reports {
            updated, reports, ..
        } = merged
        else {
            panic!("expected report snapshot");
        };
        assert_eq!(updated, "after");
        assert!(reports[0].result.is_ok());
    }

    #[test]
    fn all_quota_watch_merge_does_not_preserve_previous_success_on_auth_error() {
        let previous = AllQuotaWatchSnapshot::Reports {
            updated: "before".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report(
                "main",
                Ok(test_usage("main@example.com")),
            )],
        };
        let next = AllQuotaWatchSnapshot::Reports {
            updated: "after".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report("main", Err("HTTP 401".to_string()))],
        };

        let merged = merge_all_quota_watch_snapshot(&previous, next);

        let AllQuotaWatchSnapshot::Reports { reports, .. } = merged else {
            panic!("expected report snapshot");
        };
        assert!(reports[0].result.as_ref().unwrap_err().contains("401"));
    }

    #[test]
    fn quota_watch_auth_backoff_overlay_replaces_stale_success() {
        let snapshot = AllQuotaWatchSnapshot::Reports {
            updated: "2026-06-26 10:00:00 UTC".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report("main", Ok(test_usage("main@example.com")))],
        };

        let overlay = quota_watch_snapshot_with_auth_backoff(
            &snapshot,
            &std::collections::BTreeSet::from(["main".to_string()]),
        );

        let AllQuotaWatchSnapshot::Reports { reports, .. } = overlay else {
            panic!("expected reports snapshot");
        };
        assert!(reports[0].result.as_ref().unwrap_err().contains("unauthorized"));
        assert!(
            reports[0]
                .result
                .as_ref()
                .unwrap_err()
                .contains("prodex login main")
        );
    }

    #[test]
    fn all_quota_watch_merge_keeps_previous_snapshot_on_full_refresh_error() {
        let previous = AllQuotaWatchSnapshot::Reports {
            updated: "before".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report(
                "main",
                Ok(test_usage("main@example.com")),
            )],
        };

        let merged = merge_all_quota_watch_snapshot(
            &previous,
            AllQuotaWatchSnapshot::Error {
                updated: "after".to_string(),
                message: "load failed".to_string(),
            },
        );

        let AllQuotaWatchSnapshot::Reports {
            updated, reports, ..
        } = merged
        else {
            panic!("expected previous report snapshot");
        };
        assert_eq!(updated, "before");
        assert!(reports[0].result.is_ok());
    }

    #[test]
    fn quota_watch_sort_command_cycles_sort_mode() {
        assert!(matches!(
            apply_quota_watch_command(QuotaWatchCommand::Sort, 3, 10),
            QuotaWatchCommandOutcome::Sort
        ));
        assert_eq!(QuotaReportSort::Current.next(), QuotaReportSort::Remaining);
        assert_eq!(QuotaReportSort::Remaining.next(), QuotaReportSort::Profile);
        assert_eq!(QuotaReportSort::Plan.next(), QuotaReportSort::Current);
    }

    #[test]
    fn all_quota_watch_current_sort_places_active_profile_first() {
        let mut reports = vec![
            test_quota_report("low-reset", Ok(test_usage("low@example.com"))),
            test_quota_report("active-late", Ok(test_usage("active@example.com"))),
        ];
        reports[1].active = true;

        let output = render_all_quota_watch_report_output(
            &reports,
            false,
            0,
            QuotaReportSort::Current,
            QuotaProviderFilter::All,
            false,
        );

        assert!(output.contains("sort: current"));
        assert!(output.contains("\n\nsort: current"));
        assert!(
            output.find("active-late").unwrap() < output.find("low-reset").unwrap(),
            "active profile should be rendered before other profiles"
        );
        assert!(output.contains(" * "));
    }

    #[test]
    fn quota_watch_filter_command_cycles_provider_filter() {
        assert!(matches!(
            apply_quota_watch_command(QuotaWatchCommand::Filter, 3, 10),
            QuotaWatchCommandOutcome::Filter
        ));
        assert_eq!(QuotaProviderFilter::All.next(), QuotaProviderFilter::OpenAi);
        assert_eq!(
            QuotaProviderFilter::OpenAi.next(),
            QuotaProviderFilter::Gemini
        );
        assert_eq!(
            QuotaProviderFilter::Gemini.next(),
            QuotaProviderFilter::Anthropic
        );
        assert_eq!(
            QuotaProviderFilter::Anthropic.next(),
            QuotaProviderFilter::Copilot
        );
        assert_eq!(
            QuotaProviderFilter::Copilot.next(),
            QuotaProviderFilter::DeepSeek
        );
        assert_eq!(
            QuotaProviderFilter::DeepSeek.next(),
            QuotaProviderFilter::Local
        );
        assert_eq!(
            QuotaProviderFilter::Local.next(),
            QuotaProviderFilter::Agy
        );
        assert_eq!(QuotaProviderFilter::Agy.next(), QuotaProviderFilter::All);
    }

    #[test]
    fn quota_provider_filter_parses_aliases_and_matches_all_profile_providers() {
        assert_eq!(
            QuotaProviderFilter::parse("openai").unwrap(),
            QuotaProviderFilter::OpenAi
        );
        assert_eq!(
            QuotaProviderFilter::parse("google-gemini").unwrap(),
            QuotaProviderFilter::Gemini
        );
        assert_eq!(
            QuotaProviderFilter::parse("claude").unwrap(),
            QuotaProviderFilter::Anthropic
        );
        assert_eq!(
            QuotaProviderFilter::parse("github-copilot").unwrap(),
            QuotaProviderFilter::Copilot
        );
        assert_eq!(
            QuotaProviderFilter::parse("kiro").unwrap(),
            QuotaProviderFilter::Kiro
        );
        assert_eq!(
            QuotaProviderFilter::parse("deepseek").unwrap(),
            QuotaProviderFilter::DeepSeek
        );
        assert_eq!(
            QuotaProviderFilter::parse("openai-compatible").unwrap(),
            QuotaProviderFilter::Local
        );

        assert!(QuotaProviderFilter::OpenAi.matches(&ProfileProvider::Openai));
        assert!(
            QuotaProviderFilter::Gemini.matches(&ProfileProvider::Gemini {
                email: "gemini@example.com".to_string(),
                project_id: None,
            })
        );
        assert!(
            QuotaProviderFilter::Anthropic.matches(&ProfileProvider::Anthropic {
                account: Some("claude@example.com".to_string()),
                auth_method: Some("oauth".to_string()),
            })
        );
        assert!(
            QuotaProviderFilter::Copilot.matches(&ProfileProvider::Copilot {
                host: "github.com".to_string(),
                login: "octo".to_string(),
                api_url: "https://api.githubcopilot.com".to_string(),
                access_type_sku: None,
                copilot_plan: None,
            })
        );
        assert!(
            QuotaProviderFilter::Kiro.matches(&ProfileProvider::Kiro {
                auth_key: "codewhisperer:odic:token".to_string(),
                auth_kind: Some("builder-id".to_string()),
                profile_arn: Some("arn:aws:codewhisperer:us-east-1:123:profile/test".to_string()),
                profile_name: Some("test".to_string()),
                start_url: Some("https://view.awsapps.com/start".to_string()),
                region: Some("us-east-1".to_string()),
            })
        );
    }

    #[test]
    fn all_quota_watch_output_filters_reports_by_provider() {
        let reports = vec![
            test_quota_report_with_provider(
                "openai-main",
                ProfileProvider::Openai,
                Ok(test_usage("main@example.com")),
            ),
            test_quota_report_with_provider(
                "gemini-main",
                ProfileProvider::Gemini {
                    email: "gemini@example.com".to_string(),
                    project_id: Some("test-project".to_string()),
                },
                Ok(test_gemini_quota("gemini@example.com")),
            ),
        ];

        let output = render_all_quota_watch_report_output(
            &reports,
            true,
            0,
            QuotaReportSort::Remaining,
            QuotaProviderFilter::Gemini,
            false,
        );

        println!("DEBUG OUTPUT:\n{}", output);

        assert!(output.contains("filter: gemini"));
        assert!(output.contains("gemini-main"));
        assert!(!output.contains("openai-main"));
    }

    #[test]
    fn all_quota_watch_output_filters_imported_copilot_profiles() {
        let reports = vec![
            test_quota_report_with_provider(
                "openai-main",
                ProfileProvider::Openai,
                Ok(test_usage("main@example.com")),
            ),
            test_quota_report_with_provider(
                "copilot-main",
                ProfileProvider::Copilot {
                    host: "github.com".to_string(),
                    login: "octo".to_string(),
                    api_url: "https://api.githubcopilot.com".to_string(),
                    access_type_sku: None,
                    copilot_plan: None,
                },
                Err("quota unavailable in test".to_string()),
            ),
        ];

        let output = render_all_quota_watch_report_output(
            &reports,
            true,
            0,
            QuotaReportSort::Remaining,
            QuotaProviderFilter::Copilot,
            true,
        );

        assert!(output.contains("filter: copilot"));
        assert!(output.contains("provider fixed"));
        assert!(output.contains("copilot-main"));
        assert!(!output.contains("openai-main"));
    }
