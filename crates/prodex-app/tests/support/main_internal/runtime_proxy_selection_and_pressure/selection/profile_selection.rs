use super::*;

#[test]
fn response_selection_preserves_bound_previous_response_affinity_despite_quota() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(
        &temp_dir,
        BTreeMap::from([(
            "resp_123".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    );

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        &BTreeSet::new(),
        None,
        Some("main"),
        None,
        None,
        false,
        Some("resp_123"),
        RuntimeRouteKind::Responses,
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("main"));
}

#[test]
fn response_selection_skips_soft_pinned_affinity_when_quota_blocks_precommit() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        &BTreeSet::new(),
        None,
        Some("main"),
        None,
        None,
        false,
        Some("resp_unbound"),
        RuntimeRouteKind::Responses,
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("second"));
}

#[test]
fn response_selection_uses_prompt_cache_affinity_for_fresh_ties() {
    let temp_dir = TestDir::isolated();
    clear_runtime_prompt_cache_profile_bindings();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    let now = Local::now().timestamp();
    {
        let mut runtime = shared.runtime.lock().expect("runtime lock should succeed");
        for profile_name in ["main", "second"] {
            runtime.profile_probe_cache.insert(
                profile_name.to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(95, 18_000, 95, 604_800)),
                },
            );
        }
    }
    let prompt_cache_key = (0..256)
        .map(|index| format!("workspace-cache-{index}"))
        .find(|key| {
            runtime_prompt_cache_affinity_sort_key(Some(key.as_str()), "second")
                < runtime_prompt_cache_affinity_sort_key(Some(key.as_str()), "main")
        })
        .expect("test should find a key that prefers second");

    let selected = select_runtime_response_candidate_for_route_with_selection(
        &shared,
        RuntimeResponseCandidateSelection {
            excluded_profiles: &BTreeSet::new(),
            strict_affinity_profile: None,
            pinned_profile: None,
            turn_state_profile: None,
            session_profile: None,
            prompt_cache_key: Some(prompt_cache_key.as_str()),
            discover_previous_response_owner: false,
            previous_response_id: None,
            route_kind: RuntimeRouteKind::Responses,
        },
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("second"));
}

#[test]
fn response_selection_prefers_recorded_prompt_cache_owner_for_fresh_request() {
    let temp_dir = TestDir::isolated();
    clear_runtime_prompt_cache_profile_bindings();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    let now = Local::now().timestamp();
    {
        let mut runtime = shared.runtime.lock().expect("runtime lock should succeed");
        for profile_name in ["main", "second"] {
            runtime.profile_probe_cache.insert(
                profile_name.to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(95, 18_000, 95, 604_800)),
                },
            );
        }
    }
    let prompt_cache_key = (0..256)
        .map(|index| format!("workspace-cache-bound-{index}"))
        .find(|key| {
            runtime_prompt_cache_affinity_sort_key(Some(key.as_str()), "main")
                < runtime_prompt_cache_affinity_sort_key(Some(key.as_str()), "second")
        })
        .expect("test should find a key that would hash-prefer main");
    remember_runtime_prompt_cache_profile(
        &shared,
        "second",
        Some(prompt_cache_key.as_str()),
        RuntimeRouteKind::Responses,
    );

    let selected = select_runtime_response_candidate_for_route_with_selection(
        &shared,
        RuntimeResponseCandidateSelection {
            excluded_profiles: &BTreeSet::new(),
            strict_affinity_profile: None,
            pinned_profile: None,
            turn_state_profile: None,
            session_profile: None,
            prompt_cache_key: Some(prompt_cache_key.as_str()),
            discover_previous_response_owner: false,
            previous_response_id: None,
            route_kind: RuntimeRouteKind::Responses,
        },
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("second"));
}

#[test]
fn response_selection_keeps_inflight_pressure_ahead_of_prompt_cache_owner() {
    let temp_dir = TestDir::isolated();
    clear_runtime_prompt_cache_profile_bindings();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    let now = Local::now().timestamp();
    {
        let mut runtime = shared.runtime.lock().expect("runtime lock should succeed");
        for profile_name in ["main", "second"] {
            runtime.profile_probe_cache.insert(
                profile_name.to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(95, 18_000, 95, 604_800)),
                },
            );
        }
        runtime.profile_inflight.insert("second".to_string(), 1);
    }
    let prompt_cache_key = "workspace-cache-bound-inflight";
    remember_runtime_prompt_cache_profile(
        &shared,
        "second",
        Some(prompt_cache_key),
        RuntimeRouteKind::Responses,
    );

    let selected = select_runtime_response_candidate_for_route_with_selection(
        &shared,
        RuntimeResponseCandidateSelection {
            excluded_profiles: &BTreeSet::new(),
            strict_affinity_profile: None,
            pinned_profile: None,
            turn_state_profile: None,
            session_profile: None,
            prompt_cache_key: Some(prompt_cache_key),
            discover_previous_response_owner: false,
            previous_response_id: None,
            route_kind: RuntimeRouteKind::Responses,
        },
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("main"));
}

#[test]
fn hard_affinity_selection_matrix_ignores_local_penalties() {
    struct Case {
        label: &'static str,
        route_kind: RuntimeRouteKind,
        response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
        strict_affinity_profile: Option<&'static str>,
        pinned_profile: Option<&'static str>,
        turn_state_profile: Option<&'static str>,
        session_profile: Option<&'static str>,
        previous_response_id: Option<&'static str>,
    }

    let now = Local::now().timestamp();
    let bound_response = BTreeMap::from([(
        "resp_123".to_string(),
        ResponseProfileBinding {
            profile_name: "main".to_string(),
            bound_at: now,
        },
    )]);
    let cases = [
        Case {
            label: "strict",
            route_kind: RuntimeRouteKind::Responses,
            response_profile_bindings: BTreeMap::new(),
            strict_affinity_profile: Some("main"),
            pinned_profile: None,
            turn_state_profile: None,
            session_profile: None,
            previous_response_id: None,
        },
        Case {
            label: "previous_response",
            route_kind: RuntimeRouteKind::Responses,
            response_profile_bindings: bound_response,
            strict_affinity_profile: None,
            pinned_profile: Some("main"),
            turn_state_profile: None,
            session_profile: None,
            previous_response_id: Some("resp_123"),
        },
        Case {
            label: "turn_state",
            route_kind: RuntimeRouteKind::Responses,
            response_profile_bindings: BTreeMap::new(),
            strict_affinity_profile: None,
            pinned_profile: None,
            turn_state_profile: Some("main"),
            session_profile: None,
            previous_response_id: None,
        },
        Case {
            label: "compact_session",
            route_kind: RuntimeRouteKind::Compact,
            response_profile_bindings: BTreeMap::new(),
            strict_affinity_profile: None,
            pinned_profile: None,
            turn_state_profile: None,
            session_profile: Some("main"),
            previous_response_id: None,
        },
    ];

    for case in cases {
        let temp_dir = TestDir::isolated();
        let shared =
            runtime_shared_for_affinity_selection(&temp_dir, case.response_profile_bindings);
        apply_local_selection_penalties(&shared, "main", case.route_kind);

        let selected = select_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            case.strict_affinity_profile,
            case.pinned_profile,
            case.turn_state_profile,
            case.session_profile,
            false,
            case.previous_response_id,
            case.route_kind,
        )
        .expect("selection should succeed");

        assert_eq!(
            selected.as_deref(),
            Some("main"),
            "{} hard affinity should beat transport backoff, health, and inflight heuristics",
            case.label
        );
    }
}

#[test]
fn quota_blocked_previous_response_fresh_fallback_blocks_tool_output_only() {
    assert!(
        !runtime_quota_blocked_previous_response_fresh_fallback_allowed(
            Some("resp_123"),
            true,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
        ),
        "tool-output-only requests still need chain-scoped call context"
    );
}

#[test]
fn quota_blocked_previous_response_fresh_fallback_blocks_session_scoped_requests() {
    assert!(
        !runtime_quota_blocked_previous_response_fresh_fallback_allowed(
            Some("resp_123"),
            true,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        )
    );
    assert!(
        !runtime_quota_blocked_previous_response_fresh_fallback_allowed(
            Some("resp_123"),
            true,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        )
    );
}

#[test]
fn quota_blocked_affinity_release_blocks_nonreplayable_tool_outputs() {
    assert!(!runtime_quota_blocked_affinity_is_releasable(
        RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        ),
        true,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
    ));
}

#[test]
fn quota_blocked_affinity_release_blocks_nonreplayable_message_followups() {
    assert!(!runtime_quota_blocked_affinity_is_releasable(
        RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        ),
        false,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
    ));
}

#[test]
fn quota_blocked_affinity_release_blocks_session_scoped_empty_inputs() {
    assert!(!runtime_quota_blocked_affinity_is_releasable(
        RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        ),
        true,
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
    ));
}

#[test]
fn runtime_quota_summary_distinguishes_window_health() {
    let summary = runtime_quota_summary_for_route(
        &usage_with_main_windows(4, 18_000, 12, 604_800),
        RuntimeRouteKind::Responses,
    );

    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Critical);
    assert_eq!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical);
    assert_eq!(summary.weekly.status, RuntimeQuotaWindowStatus::Thin);
    assert_eq!(summary.five_hour.remaining_percent, 4);
    assert_eq!(summary.weekly.remaining_percent, 12);
}

#[test]
fn ready_profile_ranking_prefers_larger_reserve_when_resets_match() {
    let candidates = vec![
        ReadyProfileCandidate {
            name: "thin".to_string(),
            usage: usage_with_main_windows(65, 18_000, 70, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "deep".to_string(),
            usage: usage_with_main_windows(95, 18_000, 98, 604_800),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let mut ranked = candidates.clone();
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "deep");
}

#[test]
fn active_profile_selection_order_prefers_openai_pool_before_other_providers() {
    let state = AppState {
        active_profile: Some("copilot".to_string()),
        profiles: BTreeMap::from([
            (
                "copilot".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/copilot"),
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
                    codex_home: PathBuf::from("/tmp/openai-main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "openai-second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/openai-second"),
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

    assert_eq!(
        active_profile_selection_order(&state, "copilot"),
        vec![
            "openai-main".to_string(),
            "openai-second".to_string(),
            "copilot".to_string(),
        ]
    );
}

#[test]
fn ready_profile_candidates_prefer_openai_pool_before_other_providers() {
    let state = AppState {
        active_profile: Some("copilot".to_string()),
        profiles: BTreeMap::from([
            (
                "copilot".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/copilot"),
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
                    codex_home: PathBuf::from("/tmp/openai-main"),
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
    let reports = vec![
        RunProfileProbeReport {
            name: "copilot".to_string(),
            order_index: 0,
            auth: AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(100, 3_600, 100, 86_400)),
        },
        RunProfileProbeReport {
            name: "openai-main".to_string(),
            order_index: 1,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(80, 3_600, 80, 86_400)),
        },
    ];

    let ranked = ready_profile_candidates(&reports, false, Some("copilot"), &state, None);
    assert_eq!(ranked[0].name, "openai-main");
    assert_eq!(ranked[1].name, "copilot");
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

    let names = sort_quota_reports_for_display(&reports)
        .into_iter()
        .map(|report| report.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["ready-early", "ready-late", "blocked", "error"]);
}

#[test]
fn quota_watch_defaults_to_live_refresh_for_regular_views() {
    let profile_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: false,
        auth: None,
        base_url: None,
    };
    assert!(quota_watch_enabled(&profile_args));

    let overview_args = QuotaArgs {
        all: true,
        ..profile_args
    };
    assert!(quota_watch_enabled(&overview_args));
}

#[test]
fn quota_watch_respects_once_and_raw_modes() {
    let once_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: true,
        auth: None,
        base_url: None,
    };
    assert!(!quota_watch_enabled(&once_args));

    let raw_args = QuotaArgs {
        raw: true,
        watch: true,
        once: false,
        ..once_args
    };
    assert!(!quota_watch_enabled(&raw_args));
}

#[test]
fn quota_command_accepts_once_flag() {
    let command = parse_cli_command_from(["prodex", "quota", "--once"]).expect("quota command");
    let Commands::Quota(args) = command else {
        panic!("expected quota command");
    };
    assert!(args.once);
    assert!(!quota_watch_enabled(&args));
}

#[test]
fn quota_command_accepts_auth_filter_for_all_profiles() {
    let command = parse_cli_command_from(["prodex", "quota", "--all", "--auth", "no-auth"])
        .expect("quota command");
    let Commands::Quota(args) = command else {
        panic!("expected quota command");
    };
    assert_eq!(args.auth.as_deref(), Some("no-auth"));
}

#[test]
fn audit_command_accepts_filters_and_json() {
    let command = parse_cli_command_from([
        "prodex",
        "audit",
        "--tail",
        "50",
        "--component",
        "profile",
        "--action",
        "use",
        "--json",
    ])
    .expect("audit command");
    let Commands::Audit(args) = command else {
        panic!("expected audit command");
    };
    assert_eq!(args.tail, 50);
    assert_eq!(args.component.as_deref(), Some("profile"));
    assert_eq!(args.action.as_deref(), Some("use"));
    assert!(args.json);
}

#[test]
fn bare_prodex_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex"]).expect("bare prodex should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert!(args.profile.is_none());
    assert!(!args.auto_rotate);
    assert!(!args.no_auto_rotate);
    assert!(!args.skip_quota_check);
    assert!(args.base_url.is_none());
    assert!(args.codex_args.is_empty());
}

#[test]
fn bare_prodex_with_codex_args_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex", "exec", "review this repo"])
        .expect("bare prodex codex args should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn cleanup_command_does_not_default_to_run() {
    let command = parse_cli_command_from(["prodex", "cleanup"]).expect("cleanup command");
    assert!(matches!(command, Commands::Cleanup(_)));
}

#[test]
fn session_command_does_not_default_to_run() {
    let command = parse_cli_command_from(["prodex", "session", "list"]).expect("session command");
    assert!(matches!(
        command,
        Commands::Session(SessionCommands::List(_))
    ));
}

#[test]
fn logout_command_accepts_positional_profile_name() {
    let command = parse_cli_command_from(["prodex", "logout", "second"]).expect("logout command");
    let Commands::Logout(args) = command else {
        panic!("expected logout command");
    };
    assert_eq!(args.profile_name.as_deref(), Some("second"));
    assert_eq!(args.selected_profile(), Some("second"));
    assert!(args.profile.is_none());
}

#[test]
fn logout_command_accepts_profile_flag() {
    let command = parse_cli_command_from(["prodex", "logout", "--profile", "second"])
        .expect("logout command");
    let Commands::Logout(args) = command else {
        panic!("expected logout command");
    };
    assert_eq!(args.profile.as_deref(), Some("second"));
    assert_eq!(args.selected_profile(), Some("second"));
    assert!(args.profile_name.is_none());
}

#[test]
fn profile_remove_command_accepts_all_flag() {
    let command =
        parse_cli_command_from(["prodex", "profile", "remove", "--all"]).expect("remove command");
    let Commands::Profile(ProfileCommands::Remove(args)) = command else {
        panic!("expected profile remove command");
    };
    assert!(args.all);
    assert!(args.name.is_none());
    assert!(!args.delete_home);
}

#[test]
fn profile_remove_command_accepts_profile_name() {
    let command =
        parse_cli_command_from(["prodex", "profile", "remove", "main"]).expect("remove command");
    let Commands::Profile(ProfileCommands::Remove(args)) = command else {
        panic!("expected profile remove command");
    };
    assert!(!args.all);
    assert_eq!(args.name.as_deref(), Some("main"));
    assert!(!args.delete_home);
}

#[test]
fn bare_prodex_accepts_run_options_before_codex_args() {
    let command =
        parse_cli_command_from(["prodex", "--profile", "second", "exec", "review this repo"])
            .expect("bare prodex should accept run options");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(args.profile.as_deref(), Some("second"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn launch_commands_accept_no_proxy_flag() {
    let run = parse_cli_command_from(["prodex", "run", "--no-proxy", "exec", "hello"])
        .expect("run no-proxy should parse");
    let Commands::Run(args) = run else {
        panic!("expected run command");
    };
    assert!(args.no_proxy);

    let caveman = parse_cli_command_from(["prodex", "caveman", "--no-proxy", "exec", "hello"])
        .expect("caveman no-proxy should parse");
    let Commands::Caveman(args) = caveman else {
        panic!("expected caveman command");
    };
    assert!(args.no_proxy);

    let super_command = parse_cli_command_from(["prodex", "super", "--no-proxy", "exec", "hello"])
        .expect("super no-proxy should parse");
    let Commands::Super(args) = super_command else {
        panic!("expected super command");
    };
    assert!(args.no_proxy);
    assert!(args.into_caveman_args().no_proxy);

    let claude = parse_cli_command_from(["prodex", "claude", "--no-proxy", "--", "-p", "hello"])
        .expect("claude no-proxy should parse");
    let Commands::Claude(args) = claude else {
        panic!("expected claude command");
    };
    assert!(args.no_proxy);
}

#[test]
fn caveman_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "caveman",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("caveman command should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn caveman_command_accepts_full_access_shortcut_after_mem_prefix() {
    let command = parse_cli_command_from([
        "prodex",
        "caveman",
        "mem",
        "--full-access",
        "exec",
        "review this repo",
    ])
    .expect("caveman full-access shortcut should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };
    assert!(!args.full_access);
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("mem"),
            OsString::from("--full-access"),
            OsString::from("exec"),
            OsString::from("review this repo")
        ]
    );

    let (mem_mode, codex_args) = runtime_mem_extract_mode(&args.codex_args);
    assert!(mem_mode);
    let (launch_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    assert_eq!(
        launch_args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("review this repo")
        ]
    );
    assert!(!include_code_review);
}

#[test]
fn super_command_parses_as_distinct_subcommand_and_expands_to_caveman_mem_rtk_full_access() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));

    let args = args.into_caveman_args();
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert!(args.full_access);
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("mem"),
            OsString::from("rtk"),
            OsString::from("exec"),
            OsString::from("review this repo")
        ]
    );
}

#[test]
fn super_command_mem_full_expands_to_full_mem_prefix() {
    let command = parse_cli_command_from(["prodex", "super", "--mem-full", "exec", "review"])
        .expect("super mem-full command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };

    let args = args.into_caveman_args();
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("mem-full"),
            OsString::from("rtk"),
            OsString::from("exec"),
            OsString::from("review")
        ]
    );
    let (mem_mode, rtk_enabled, codex_args) =
        runtime_caveman_extract_launch_prefixes(&args.codex_args);
    assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Full));
    assert!(rtk_enabled);
    assert_eq!(codex_args, vec![OsString::from("exec"), OsString::from("review")]);
}

#[test]
fn super_command_accepts_s_alias() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("super alias command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn super_command_url_expands_to_local_openai_provider_config() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "local/qwen",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.url.as_deref(), Some("http://127.0.0.1:8131"));
    assert_eq!(args.local_model.as_deref(), Some("local/qwen"));

    let args = args.into_caveman_args();
    assert!(args.full_access);
    assert!(args.skip_quota_check);

    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(rendered.first().map(String::as_str), Some("mem"));
    assert_eq!(rendered.get(1).map(String::as_str), Some("rtk"));
    assert!(rendered.contains(&"model_provider=\"prodex-local\"".to_string()));
    assert!(rendered.contains(&"model=\"local/qwen\"".to_string()));
    assert!(rendered.contains(
        &"model_providers.prodex-local.base_url=\"http://127.0.0.1:8131/v1\"".to_string()
    ));
    assert!(rendered.contains(&"model_providers.prodex-local.wire_api=\"responses\"".to_string()));
    assert!(
        rendered.contains(&"model_providers.prodex-local.supports_websockets=false".to_string())
    );
    assert!(rendered.contains(&"model_context_window=16384".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=14000".to_string()));
    assert!(rendered.contains(&"web_search=\"disabled\"".to_string()));
    assert!(rendered.contains(&"features.js_repl=false".to_string()));
    assert!(rendered.contains(&"features.image_generation=false".to_string()));
    assert_eq!(
        &rendered[rendered.len() - 2..],
        ["exec", "review this repo"]
    );

    let (_, _, codex_args) = runtime_caveman_extract_launch_prefixes(&args.codex_args);
    assert_eq!(
        codex_cli_config_override_value(&codex_args, "model_provider").as_deref(),
        Some("prodex-local")
    );
}

#[test]
fn super_command_url_keeps_v1_path_when_provided() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://host.docker.internal:11434/v1/",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };

    let args = args.into_caveman_args();
    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        rendered.contains(
            &"model_providers.prodex-local.base_url=\"http://host.docker.internal:11434/v1\""
                .to_string()
        )
    );
}

#[test]
fn super_command_url_rejects_invalid_or_empty_values() {
    for url in ["", "not-a-url", "file:///tmp/model.sock", "http:///v1"] {
        let err = parse_cli_command_from(["prodex", "super", "--url", url, "exec", "hello"])
            .expect_err("invalid super local provider URL should fail");
        let message = err.to_string();
        assert!(
            message.contains("invalid --url"),
            "expected clear --url error for {url:?}, got {message}"
        );
    }
}

#[test]
fn super_command_url_accepts_local_context_overrides() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://127.0.0.1:8131",
        "--context-window",
        "32768",
        "--auto-compact-token-limit",
        "30000",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };

    let args = args.into_caveman_args();
    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(rendered.contains(&"model_context_window=32768".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=30000".to_string()));
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
