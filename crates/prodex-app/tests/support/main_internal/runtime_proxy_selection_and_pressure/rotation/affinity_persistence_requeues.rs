use super::*;

#[test]
fn duplicate_previous_response_owner_verifies_do_not_requeue_persistence() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    let initial_second = Local::now().timestamp();
    while Local::now().timestamp() == initial_second {
        thread::sleep(Duration::from_millis(5));
    }

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("first verification should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after first bind");
    let binding_marker = "binding previous_response_owner profile=main response_id=resp-1";
    let first_binding_count = first_log.matches(binding_marker).count();
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(
        first_binding_count, 1,
        "first bind should be persisted once: {first_log}"
    );
    assert_eq!(
        first_revision, 1,
        "first bind should persist once: {first_log}"
    );

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("duplicate verification should succeed");
    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("second duplicate verification should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate binds");
    assert_eq!(
        second_log.matches(binding_marker).count(),
        first_binding_count,
        "duplicate verifies should not re-log identical owner bindings: {second_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate verifies should not requeue persistence: {second_log}"
    );
}

#[test]
fn duplicate_response_ids_do_not_requeue_persistence() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    let response_ids = vec!["resp-1".to_string()];
    remember_runtime_response_ids(&shared, "main", &response_ids, RuntimeRouteKind::Responses)
        .expect("first response id bind should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after first bind");
    let binding_marker = "binding response_ids profile=main count=1 first=Some(\"resp-1\")";
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(first_log.matches(binding_marker).count(), 1);
    assert_eq!(
        first_revision, 1,
        "first bind should persist once: {first_log}"
    );

    remember_runtime_response_ids(&shared, "main", &response_ids, RuntimeRouteKind::Responses)
        .expect("duplicate response id bind should succeed");
    remember_runtime_response_ids(&shared, "main", &response_ids, RuntimeRouteKind::Responses)
        .expect("second duplicate response id bind should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate binds");
    assert_eq!(
        second_log.matches(binding_marker).count(),
        1,
        "duplicate response id binds should not re-log: {second_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate response id binds should not requeue persistence: {second_log}"
    );
}

#[test]
fn runtime_affinity_touch_lookups_do_not_requeue_persistence_before_interval() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let compact_session_key = runtime_compact_session_lineage_key("session-compact");
    let compact_turn_state_key = runtime_compact_turn_state_lineage_key("turn-compact");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "session-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
    };
    state.save(&paths).expect("state should save");

    let verified_status = |route: &str| RuntimeContinuationBindingStatus {
        state: RuntimeContinuationBindingLifecycle::Verified,
        confidence: 1,
        last_touched_at: Some(now),
        last_verified_at: Some(now),
        last_verified_route: Some(route.to_string()),
        last_not_found_at: None,
        not_found_streak: 0,
        success_count: 1,
        failure_count: 0,
    };

    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::from([
            (
                "turn-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                compact_turn_state_key.clone(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_id_bindings: BTreeMap::from([
            (
                "session-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                compact_session_key.clone(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        continuation_statuses: RuntimeContinuationStatuses {
            response: BTreeMap::from([("resp-1".to_string(), verified_status("responses"))]),
            turn_state: BTreeMap::from([
                ("turn-1".to_string(), verified_status("responses")),
                (compact_turn_state_key.clone(), verified_status("compact")),
            ]),
            session_id: BTreeMap::from([
                ("session-1".to_string(), verified_status("websocket")),
                (compact_session_key.clone(), verified_status("compact")),
            ]),
        },
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    for _ in 0..3 {
        assert_eq!(
            runtime_response_bound_profile(&shared, "resp-1", RuntimeRouteKind::Responses)
                .expect("response owner lookup should succeed"),
            Some("main".to_string())
        );
        assert_eq!(
            runtime_turn_state_bound_profile(&shared, "turn-1")
                .expect("turn state lookup should succeed"),
            Some("main".to_string())
        );
        assert_eq!(
            runtime_session_bound_profile(&shared, "session-1")
                .expect("session lookup should succeed"),
            Some("main".to_string())
        );
        assert_eq!(
            runtime_compact_route_followup_bound_profile(&shared, Some("turn-compact"), None)
                .expect("compact turn-state lookup should succeed"),
            Some(("main".to_string(), "turn_state"))
        );
        assert_eq!(
            runtime_compact_route_followup_bound_profile(&shared, None, Some("session-compact"))
                .expect("compact session lookup should succeed"),
            Some(("main".to_string(), "session_id"))
        );
    }
    wait_for_runtime_background_queues_idle();

    let log = fs::read_to_string(&shared.log_path).unwrap_or_default();
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        0,
        "fresh touch lookups should not requeue persistence before the interval elapses: {log}"
    );
    assert!(
        !log.contains("response_touch:resp-1")
            && !log.contains("turn_state_touch:turn-1")
            && !log.contains("session_touch:session-1")
            && !log.contains("compact_turn_state_touch:turn-compact")
            && !log.contains("compact_session_touch:session-compact"),
        "touch lookups should stay in-memory until the persistence interval elapses: {log}"
    );
}
