use super::*;

fn selection_trace(
    shared: &RuntimeRotationProxyShared,
) -> runtime_proxy_crate::RuntimeRouteDecisionTrace {
    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let log = read_runtime_proxy_test_log(&shared.log_path);
    let trace_line = log
        .lines()
        .find(|line| line.contains(" route_decision "))
        .expect("selection should emit a route decision trace");
    let trace_json = runtime_proxy_crate::runtime_proxy_log_fields(trace_line)
        .remove("trace")
        .expect("route decision trace should contain typed JSON");
    serde_json::from_str(&trace_json).expect("route decision trace should be valid")
}

#[test]
fn response_selection_preserves_bound_previous_response_affinity_despite_quota() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(
        &temp_dir,
        BTreeMap::from([(
            "resp_123".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    );

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection {
            pinned_profile: Some("main"),
            previous_response_id: Some("resp_123"),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("main"));
}

#[test]
fn compact_followup_honors_raw_turn_state_owner() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .turn_state_bindings
        .insert(
            "turn-raw".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        );

    assert_eq!(
        runtime_compact_route_followup_bound_profile(&shared, Some("turn-raw"), None)
            .expect("compact turn-state lookup should succeed"),
        Some(("main".to_string(), "turn_state"))
    );
}

#[test]
fn response_selection_fails_closed_for_unavailable_bound_previous_response() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(
        &temp_dir,
        BTreeMap::from([(
            "resp-missing".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "missing".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    );

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection {
            discover_previous_response_owner: true,
            previous_response_id: Some("resp-missing"),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .expect("selection should succeed without an executable owner");

    assert_eq!(selected, None);
}

#[test]
fn response_selection_fails_closed_for_conflicting_supplied_affinity_keys() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection {
            pinned_profile: Some("main"),
            turn_state_profile: Some("second"),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .expect("selection should succeed without an executable owner");

    assert_eq!(selected, None);
}

#[test]
fn response_selection_fails_closed_for_known_auth_failed_bound_owner() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(
        &temp_dir,
        BTreeMap::from([(
            "resp-auth-failed".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    );
    let now = Local::now().timestamp();
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_health
        .insert(
            runtime_profile_auth_failure_key("main"),
            RuntimeProfileHealth {
                score: 1,
                updated_at: now,
            },
        );

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection {
            discover_previous_response_owner: true,
            previous_response_id: Some("resp-auth-failed"),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .expect("selection should succeed without an executable owner");

    assert_eq!(selected, None);
}

#[test]
fn exact_binding_checks_only_the_requested_continuation_owner() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    let current_identity = {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        runtime_profile_binding_identity(&runtime, "main").expect("main identity should resolve")
    };
    let stale_identity = prodex_provider_core::RuntimeProviderBindingIdentity::from_profile(
        prodex_provider_core::ProviderId::OpenAi,
        "main",
        "https://stale.example.com/v1",
    )
    .expect("stale endpoint identity should resolve");
    {
        let mut runtime = shared.runtime.lock().expect("runtime lock should succeed");
        runtime.state.response_profile_bindings.extend([
            (
                "resp-current".to_string(),
                ResponseProfileBinding {
                    binding_identity: Some(current_identity),
                    profile_name: "main".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            ),
            (
                "resp-stale".to_string(),
                ResponseProfileBinding {
                    binding_identity: Some(stale_identity),
                    profile_name: "main".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            ),
        ]);
    }

    let current = runtime_response_bound_profile(
        &shared,
        "resp-current",
        RuntimeRouteKind::Responses,
    )
    .unwrap();
    assert_eq!(current.as_deref(), Some("main"));
    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection {
            pinned_profile: current.as_deref(),
            previous_response_id: Some("resp-current"),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .unwrap();
    assert_eq!(selected.as_deref(), Some("main"));

    assert_eq!(
        runtime_response_bound_profile(&shared, "resp-stale", RuntimeRouteKind::Responses)
            .unwrap()
            .as_deref(),
        Some(prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE)
    );
}

#[test]
fn previous_response_discovery_skips_blocked_profiles_before_cached_candidate() {
    let response_id = "resp_unbound";

    let quota_dir = TestDir::isolated();
    let quota_shared = runtime_shared_for_affinity_selection(&quota_dir, BTreeMap::new());
    let selected = select_runtime_response_candidate_for_route(
        &quota_shared,
        RuntimeResponseCandidateSelection {
            discover_previous_response_owner: true,
            previous_response_id: Some(response_id),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .expect("quota-aware discovery should succeed");
    assert_eq!(selected.as_deref(), Some("second"));
    let trace = selection_trace(&quota_shared);
    assert!(trace.candidates.iter().any(|candidate| {
        candidate.eligibility == runtime_proxy_crate::RuntimeRouteCandidateEligibility::Rejected
            && candidate.rejection_stage
                == Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Quota)
    }));

    let negative_dir = TestDir::isolated();
    let negative_shared = runtime_shared_for_affinity_selection(&negative_dir, BTreeMap::new());
    let now = Local::now().timestamp();
    {
        let mut runtime = negative_shared
            .runtime
            .lock()
            .expect("runtime lock should succeed");
        runtime
            .profile_probe_cache
            .get_mut("main")
            .expect("main probe should exist")
            .result = Ok(usage_with_main_windows(95, 18_000, 95, 604_800));
        runtime.profile_health.insert(
            runtime_previous_response_negative_cache_key(
                response_id,
                "main",
                RuntimeRouteKind::Responses,
            ),
            RuntimeProfileHealth {
                score: RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD,
                updated_at: now,
            },
        );
    }
    let selected = select_runtime_response_candidate_for_route(
        &negative_shared,
        RuntimeResponseCandidateSelection {
            discover_previous_response_owner: true,
            previous_response_id: Some(response_id),
            ..RuntimeResponseCandidateSelection::fresh(
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )
        },
    )
    .expect("negative-cache-aware discovery should succeed");
    assert_eq!(selected.as_deref(), Some("second"));
    let trace = selection_trace(&negative_shared);
    assert!(trace.candidates.iter().any(|candidate| {
        candidate
            .reason
            .as_ref()
            .is_some_and(|reason| reason.as_str() == "negative_cache")
            && candidate.rejection_stage
                == Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Affinity)
    }));
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

    let selected = select_runtime_response_candidate_for_route(
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
    }
    shared.lane_admission.set_profile_inflight("second", 1);
    let prompt_cache_key = "workspace-cache-bound-inflight";
    remember_runtime_prompt_cache_profile(
        &shared,
        "second",
        Some(prompt_cache_key),
        RuntimeRouteKind::Responses,
    );

    let selected = select_runtime_response_candidate_for_route(
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
fn session_only_affinity_rotates_on_quota_outside_compact() {
    for (route_kind, hard) in [
        (RuntimeRouteKind::Responses, false),
        (RuntimeRouteKind::Websocket, false),
        (RuntimeRouteKind::Compact, true),
    ] {
        let affinity = RuntimeCandidateAffinity::new(
            route_kind,
            "main",
            None,
            None,
            None,
            Some("main"),
            false,
        );
        assert_eq!(runtime_candidate_has_hard_affinity(affinity), hard);
        assert_eq!(
            runtime_quota_blocked_affinity_is_releasable(affinity, false, None),
            !hard,
        );
    }
}

#[test]
fn session_only_affinity_rotates_past_excluded_owner_outside_compact() {
    for (route_kind, expected) in [
        (RuntimeRouteKind::Responses, Some("second")),
        (RuntimeRouteKind::Websocket, Some("second")),
        (RuntimeRouteKind::Compact, None),
    ] {
        let temp_dir = TestDir::isolated();
        let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
        let excluded = BTreeSet::from(["main".to_string()]);
        let selected = select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                session_profile: Some("main"),
                ..RuntimeResponseCandidateSelection::fresh(&excluded, route_kind)
            },
        )
        .expect("selection should succeed");

        assert_eq!(selected.as_deref(), expected);
    }
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

    let selected = select_runtime_response_candidate_for_route(
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
fn soft_session_affinity_respects_route_selection_backoff() {
    for route_kind in [RuntimeRouteKind::Responses, RuntimeRouteKind::Websocket] {
        let temp_dir = TestDir::isolated();
        let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
        shared
            .runtime
            .lock()
            .expect("runtime lock should succeed")
            .profile_probe_cache
            .get_mut("main")
            .expect("main probe should exist")
            .result = Ok(usage_with_main_windows(80, 18_000, 80, 604_800));
        apply_local_selection_penalties(&shared, "main", route_kind);

        let selected = select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                session_profile: Some("main"),
                ..RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), route_kind)
            },
        )
        .expect("selection should succeed");

        assert_eq!(selected.as_deref(), Some("second"));
    }
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
            binding_identity: None,
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
            RuntimeResponseCandidateSelection {
                strict_affinity_profile: case.strict_affinity_profile,
                pinned_profile: case.pinned_profile,
                turn_state_profile: case.turn_state_profile,
                session_profile: case.session_profile,
                previous_response_id: case.previous_response_id,
                ..RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), case.route_kind)
            },
        )
        .expect("selection should succeed");

        assert_eq!(
            selected.as_deref(),
            Some("main"),
            "{} hard affinity should beat transport backoff, health, and inflight heuristics",
            case.label
        );
        runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
        let log = read_runtime_proxy_test_log(&shared.log_path);
        let trace_lines = log
            .lines()
            .filter(|line| line.contains(" route_decision "))
            .collect::<Vec<_>>();
        assert_eq!(
            trace_lines.len(),
            1,
            "{} selection must emit exactly one trace: {log}",
            case.label
        );
        let trace_json = runtime_proxy_crate::runtime_proxy_log_fields(trace_lines[0])
            .remove("trace")
            .expect("hard-affinity selection should emit a route decision trace");
        let trace =
            serde_json::from_str::<runtime_proxy_crate::RuntimeRouteDecisionTrace>(&trace_json)
                .expect("route decision trace should be typed JSON");
        assert!(
            trace.affinity.hard,
            "{} trace must retain hard affinity",
            case.label
        );
        assert_eq!(
            trace.affinity.outcome,
            runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained
        );
        assert!(trace.candidates.iter().any(|candidate| candidate.selected));
        assert!(!trace_json.contains("main"));
    }
}
