use super::*;

#[test]
fn scheduler_prefers_rested_profile_within_near_optimal_band() {
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::from([
            ("fresh".to_string(), now),
            ("rested".to_string(), now - 3_600),
        ]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "fresh".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "rested".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, None);
    assert_eq!(ranked[0].name, "rested");
}

#[test]
fn scheduler_preserves_order_index_tiebreaker_for_identical_candidates() {
    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let usage = usage_with_main_windows(96, 18_000, 96, 604_800);
    let candidates = vec![
        ReadyProfileCandidate {
            name: "later".to_string(),
            usage: usage.clone(),
            order_index: 2,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "earlier".to_string(),
            usage,
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, None);
    assert_eq!(ranked[0].name, "earlier");
    assert_eq!(ranked[1].name, "later");
}

#[test]
fn scheduler_allows_switch_when_preferred_profile_is_in_cooldown() {
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::from([("active".to_string(), now)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "better".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "active".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: true,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
    assert_eq!(ranked[0].name, "better");
}

#[test]
fn run_profile_probe_is_ready_only_when_live_quota_is_clear() {
    let ready = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(90, 3_600, 95, 86_400)),
    };
    let blocked = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(0, 300, 95, 86_400)),
    };
    let failed = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("timeout".to_string()),
    };

    assert!(run_profile_probe_is_ready(&ready, false));
    assert!(!run_profile_probe_is_ready(&blocked, false));
    assert!(!run_profile_probe_is_ready(&failed, false));
}

#[test]
fn quota_overview_sort_prioritizes_status_then_nearest_reset() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            provider: ProfileProvider::Openai,
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
            provider: ProfileProvider::Openai,
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
            provider: ProfileProvider::Openai,
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
            provider: ProfileProvider::Openai,
            workspace_id: None,
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let names = sort_quota_reports_for_display(&reports)
        .into_iter()
        .map(|report| report.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["ready-early", "ready-late", "blocked", "error"]);
}

#[test]
fn runtime_launch_selection_resolve_falls_back_from_active_copilot_to_openai() {
    let root = TestDir::isolated();
    let copilot_home = root.path.join("copilot");
    let openai_home = root.path.join("openai-main");
    fs::create_dir_all(&copilot_home).expect("create copilot home");
    fs::create_dir_all(&openai_home).expect("create openai home");

    let state = AppState {
        active_profile: Some("copilot".to_string()),
        profiles: BTreeMap::from([
            (
                "copilot".to_string(),
                ProfileEntry {
                    codex_home: copilot_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Copilot {
                        host: "https://github.com".to_string(),
                        login: "copilot-user".to_string(),
                        api_url: "https://api.business.githubcopilot.com".to_string(),
                        access_type_sku: None,
                        copilot_plan: None,
                    },
                },
            ),
            (
                "openai-main".to_string(),
                ProfileEntry {
                    codex_home: openai_home.clone(),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let selected =
        resolve_runtime_launch_profile_name(&state, None).expect("resolve runtime launch name");
    assert_eq!(selected, "openai-main");
}

#[test]
fn scheduler_keeps_preferred_profile_when_gain_is_small() {
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "better".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "active".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: true,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
    assert_eq!(ranked[0].name, "active");
}

#[test]
fn ready_profile_candidates_use_persisted_snapshot_when_probe_is_unavailable() {
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/second"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    let reports = vec![
        RunProfileProbeReport {
            name: "main".to_string(),
            order_index: 0,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("runtime quota snapshot unavailable".to_string()),
        },
        RunProfileProbeReport {
            name: "second".to_string(),
            order_index: 1,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("runtime quota snapshot unavailable".to_string()),
        },
    ];
    let now = Local::now().timestamp();
    let persisted = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 70,
                weekly_reset_at: now + 86_400,
            },
        ),
        (
            "second".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 90,
                five_hour_reset_at: now + 3_600,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 95,
                weekly_reset_at: now + 604_800,
            },
        ),
    ]);

    let ranked = ready_profile_candidates(&reports, false, Some("main"), &state, Some(&persisted));
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].name, "second");
    assert_eq!(
        ranked[0].quota_source,
        RuntimeQuotaSource::PersistedSnapshot
    );
}

#[test]
fn run_preflight_reports_with_current_first_preserves_current_and_rotation_order() {
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/second"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/third"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let current_report = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(90, 3_600, 95, 86_400)),
    };

    let reports = run_preflight_reports_with_current_first(
        &state,
        "main",
        current_report.clone(),
        None,
        false,
    );

    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].name, "main");
    assert_eq!(reports[0].order_index, 0);
    assert_eq!(
        reports[0]
            .result
            .as_ref()
            .ok()
            .map(format_main_windows_compact),
        current_report
            .result
            .as_ref()
            .ok()
            .map(format_main_windows_compact)
    );
    assert_eq!(reports[1].name, "second");
    assert_eq!(reports[1].order_index, 1);
    assert_eq!(reports[2].name, "third");
    assert_eq!(reports[2].order_index, 2);
}
