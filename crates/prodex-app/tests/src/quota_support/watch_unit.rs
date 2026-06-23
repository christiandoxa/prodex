    use super::*;

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
    fn all_quota_watch_merge_preserves_previous_successful_report_on_refresh_error() {
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
