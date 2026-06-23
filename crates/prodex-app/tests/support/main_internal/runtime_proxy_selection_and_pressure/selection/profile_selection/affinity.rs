use super::*;

#[test]
fn fresh_response_selection_uses_current_candidate_path_and_pool_member() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());
    let profile_pool = {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        runtime
            .state
            .profiles
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
    };

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), RuntimeRouteKind::Responses),
    )
    .expect("fresh selection should succeed")
    .expect("fresh selection should return a profile");

    assert!(
        profile_pool.contains(&selected),
        "fresh selection must return a profile from the configured pool: selected={selected:?} pool={profile_pool:?}"
    );
    let log = std::fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("selection_plan route=responses")
            && log.contains(format!("selection_pick route=responses profile={selected}").as_str()),
        "fresh selection should leave observable evidence that it used the runtime candidate path: {log}"
    );
}

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
        RuntimeResponseCandidateSelection {
            pinned_profile: Some("main"),
            previous_response_id: Some("resp_123"),
            ..RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), RuntimeRouteKind::Responses)
        },
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("main"));
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
        runtime.profile_inflight.insert("second".to_string(), 1);
    }
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
    }
}
