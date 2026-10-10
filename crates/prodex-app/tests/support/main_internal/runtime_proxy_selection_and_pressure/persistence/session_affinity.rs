use super::*;

#[test]
fn responses_compact_followup_affinity_allows_owner_without_runtime_quota_data() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_backoff_updated_at: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                strict_affinity_profile: Some("second"),
                ..RuntimeResponseCandidateSelection::fresh(
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )
            },
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn affinity_candidate_skips_persisted_exhausted_session_owner() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-123".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::from([(
            "sess-123".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    plan_type: None,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                    weekly_remaining_percent: 0,
                    weekly_reset_at: now + 300,
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    plan_type: None,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 95,
                    weekly_reset_at: now + 604_800,
                },
            ),
        ]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_backoff_updated_at: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                session_profile: Some("main"),
                ..RuntimeResponseCandidateSelection::fresh(
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )
            },
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn responses_session_affinity_skips_profiles_without_usable_quota_data() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-unknown".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "second".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::from([(
            "sess-unknown".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([(
            "second".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                plan_type: None,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 90,
                five_hour_reset_at: now + 18_000,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 95,
                weekly_reset_at: now + 604_800,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_backoff_updated_at: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                session_profile: Some("main"),
                ..RuntimeResponseCandidateSelection::fresh(
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )
            },
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn previous_response_discovery_skips_exhausted_current_profile() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    plan_type: None,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                    weekly_remaining_percent: 0,
                    weekly_reset_at: now + 300,
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    plan_type: None,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 95,
                    weekly_reset_at: now + 604_800,
                },
            ),
        ]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_backoff_updated_at: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                discover_previous_response_owner: true,
                previous_response_id: Some("resp-second"),
                ..RuntimeResponseCandidateSelection::fresh(
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )
            },
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn previous_response_discovery_consumer_uses_mojo_order_and_keeps_hard_affinity() {
    let temp_dir = TestDir::isolated();
    let mut runtime = RuntimeProxyFixtureBuilder::new().build_runtime(&temp_dir);
    for (name, account_id) in [
        ("main", "main-account"),
        ("second", "second-account"),
        ("third", "third-account"),
    ] {
        let codex_home = temp_dir.path.join(format!("homes/{name}"));
        write_auth_json(&codex_home.join("auth.json"), account_id);
        runtime.state.profiles.insert(
            name.to_string(),
            ProfileEntry {
                codex_home,
                managed: true,
                email: Some(format!("{name}@example.com")),
                provider: ProfileProvider::Openai,
            },
        );
    }
    runtime.state.active_profile = Some("second".to_string());
    runtime.current_profile = "second".to_string();
    assert!(runtime.state.profiles.len() > 1);
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);
    let mut exercised = 0;

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                discover_previous_response_owner: true,
                previous_response_id: Some("resp-order"),
                ..RuntimeResponseCandidateSelection::fresh(
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )
            },
        )
        .expect("unbound previous-response discovery should succeed"),
        Some("second".to_string()),
        "equal provider priorities must retain the Mojo current-relative order"
    );
    exercised += 1;

    let now = Local::now().timestamp();
    shared
        .runtime
        .lock()
        .expect("runtime lock")
        .state
        .response_profile_bindings
        .insert(
            "resp-hard".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "third".to_string(),
                bound_at: now,
            },
        );
    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                discover_previous_response_owner: true,
                previous_response_id: Some("resp-hard"),
                ..RuntimeResponseCandidateSelection::fresh(
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )
            },
        )
        .expect("hard previous-response discovery should succeed"),
        Some("third".to_string()),
        "a bound continuation must stay on its owner"
    );
    exercised += 1;

    shared
        .runtime
        .lock()
        .expect("runtime lock")
        .state
        .response_profile_bindings
        .insert(
            "resp-malformed".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "missing-profile".to_string(),
                bound_at: now,
            },
        );
    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                discover_previous_response_owner: true,
                previous_response_id: Some("resp-malformed"),
                ..RuntimeResponseCandidateSelection::fresh(
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )
            },
        )
        .expect("malformed binding should fail closed"),
        None
    );
    exercised += 1;
    assert!(exercised > 0);
}
