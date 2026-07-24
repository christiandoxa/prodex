use super::*;

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
fn ready_profile_ranking_uses_order_index_as_final_deterministic_tiebreaker() {
    let usage = usage_with_main_windows(90, 18_000, 90, 604_800);
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

    let mut ranked = candidates;
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "earlier");
    assert_eq!(ranked[1].name, "later");
}

#[test]
fn ready_profile_ranking_applies_plan_capacity_pressure_weight() {
    let mut pro_usage = usage_with_main_windows(70, 18_000, 70, 604_800);
    pro_usage.plan_type = Some("prolite".to_string());
    let mut plus_usage = usage_with_main_windows(80, 18_000, 80, 604_800);
    plus_usage.plan_type = Some("plus".to_string());
    let candidates = vec![
        ReadyProfileCandidate {
            name: "plus".to_string(),
            usage: plus_usage,
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "pro".to_string(),
            usage: pro_usage,
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let mut ranked = candidates;
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "pro");
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
fn response_selection_skips_soft_pinned_affinity_when_quota_blocks_precommit() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection {
            pinned_profile: Some("main"),
            previous_response_id: Some("resp_unbound"),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("second"));
}

#[test]
fn response_selection_logs_plan_counts_before_pick() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), RuntimeRouteKind::Responses),
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("second"));
    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains(
            "selection_plan route=responses pressure_mode=false sync_probe_pressure=false"
        ),
        "selection plan should log route and pressure state: {log}"
    );
    assert!(
        log.contains("reports=2 ready=1 fallback=1 excluded_count=0"),
        "selection plan should log candidate counts: {log}"
    );
    assert!(
        log.contains("selection_pick route=responses profile=second"),
        "selection pick should still follow the plan: {log}"
    );
    let trace_lines = log
        .lines()
        .filter(|line| line.contains(" route_decision "))
        .collect::<Vec<_>>();
    assert_eq!(
        trace_lines.len(),
        1,
        "selection must emit exactly one trace: {log}"
    );
    let trace_json = runtime_proxy_crate::runtime_proxy_log_fields(trace_lines[0])
        .remove("trace")
        .expect("selection should emit one route decision trace");
    let trace = serde_json::from_str::<runtime_proxy_crate::RuntimeRouteDecisionTrace>(&trace_json)
        .expect("route decision trace should be typed JSON");
    assert_eq!(
        trace.terminal_outcome,
        runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::Selected
    );
    assert!(trace.selected_candidate.is_some());
    assert!(trace.candidates.iter().any(|candidate| candidate.selected));
    assert!(!trace_json.contains("second"));
}

#[test]
fn response_selection_trace_preserves_optimistic_current_circuit_rejection() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_route_circuit_open_until
        .insert(
            runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
            Local::now().timestamp() + 60,
        );

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), RuntimeRouteKind::Responses),
    )
    .expect("selection should succeed");
    assert_eq!(selected.as_deref(), Some("second"));

    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    let trace_lines = log
        .lines()
        .filter(|line| line.contains(" route_decision "))
        .collect::<Vec<_>>();
    assert_eq!(
        trace_lines.len(),
        1,
        "selection must emit exactly one trace: {log}"
    );
    let trace_json = runtime_proxy_crate::runtime_proxy_log_fields(trace_lines[0])
        .remove("trace")
        .expect("selection should emit one route decision trace");
    let trace = serde_json::from_str::<runtime_proxy_crate::RuntimeRouteDecisionTrace>(&trace_json)
        .expect("route decision trace should be typed JSON");
    let current = trace
        .candidates
        .iter()
        .find(|candidate| {
            candidate.class == runtime_proxy_crate::RuntimeRouteCandidateClass::Current
        })
        .expect("trace should preserve the optimistic current decision");
    assert_eq!(
        current
            .reason
            .as_ref()
            .map(runtime_proxy_crate::RuntimeRouteDecisionReason::as_str),
        Some(runtime_proxy_crate::RuntimeRouteDecisionReasonKind::RouteCircuitOpen.as_str())
    );
    assert_eq!(
        current.circuit_state,
        Some(runtime_proxy_crate::RuntimeRouteCircuitState::Open)
    );
    assert!(!current.selected);
    assert!(trace.candidates.iter().any(|candidate| candidate.selected));
}

#[test]
fn response_selection_emits_one_no_candidate_trace() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    let excluded = BTreeSet::from(["main".to_string(), "second".to_string()]);

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection::fresh(&excluded, RuntimeRouteKind::Responses),
    )
    .expect("selection should succeed");
    assert_eq!(selected, None);

    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    let trace_lines = log
        .lines()
        .filter(|line| line.contains(" route_decision "))
        .collect::<Vec<_>>();
    assert_eq!(
        trace_lines.len(),
        1,
        "selection must emit exactly one trace: {log}"
    );
    let trace_json = runtime_proxy_crate::runtime_proxy_log_fields(trace_lines[0])
        .remove("trace")
        .expect("selection should emit one route decision trace");
    let trace = serde_json::from_str::<runtime_proxy_crate::RuntimeRouteDecisionTrace>(&trace_json)
        .expect("route decision trace should be typed JSON");
    assert_eq!(
        trace.terminal_outcome,
        runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::NoCandidate
    );
    assert_eq!(trace.selected_candidate, None);
}

#[test]
fn direct_hard_selection_trace_has_explicit_affinity() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());

    runtime_selection_trace_log_direct(
        &shared,
        42,
        RuntimeSelectionTraceDirect {
            requested_model: Some("gpt-test"),
            route_kind: RuntimeRouteKind::Standard,
            candidate_key: "main",
            class: runtime_proxy_crate::RuntimeRouteCandidateClass::Current,
            affinity_kind: Some(runtime_proxy_crate::RuntimeRouteAffinityKind::Strict),
            hard_affinity: true,
        },
    );

    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    let trace_json = log
        .lines()
        .find(|line| line.contains(" route_decision "))
        .and_then(|line| runtime_proxy_crate::runtime_proxy_log_fields(line).remove("trace"))
        .expect("direct selection should emit a trace");
    let trace = serde_json::from_str::<runtime_proxy_crate::RuntimeRouteDecisionTrace>(&trace_json)
        .expect("route decision trace should be typed JSON");
    assert!(trace.affinity.hard);
    assert_eq!(
        trace.affinity.kind,
        runtime_proxy_crate::RuntimeRouteAffinityKind::Strict
    );
    assert_eq!(
        trace.affinity.outcome,
        runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained
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
