use super::*;

#[test]
fn previous_response_owner_discovery_ignores_retry_backoff() {
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
                    codex_home: main_home.clone(),
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
        profile_retry_backoff_until: BTreeMap::from([(
            "second".to_string(),
            Local::now().timestamp().saturating_add(60),
        )]),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
        upstream_no_proxy: false,
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let excluded = BTreeSet::from(["main".to_string()]);

    assert_eq!(
        next_runtime_previous_response_candidate(
            &shared,
            &excluded,
            Some("resp-second"),
            RuntimeRouteKind::Responses,
        )
        .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn previous_response_owner_profile_changes_still_persist() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    create_codex_home_if_missing(&main_home).expect("main profile home should be created");
    create_codex_home_if_missing(&second_home).expect("second profile home should be created");

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
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-2"),
        RuntimeRouteKind::Websocket,
    )
    .expect("first owner verification should succeed");
    wait_for_runtime_background_queues_idle();
    let initial_log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    let initial_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(
        initial_revision, 1,
        "first owner save should persist once: {initial_log}"
    );

    remember_runtime_successful_previous_response_owner(
        &shared,
        "second",
        Some("resp-2"),
        RuntimeRouteKind::Responses,
    )
    .expect("owner change verification should succeed");
    wait_for_runtime_background_queues_idle();

    let updated_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after rebinding");
    assert!(
        updated_log.contains("binding previous_response_owner profile=second response_id=resp-2"),
        "owner changes should still be logged and persisted: {updated_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        initial_revision + 1,
        "owner changes should queue a fresh persistence update: {updated_log}"
    );

    let runtime = shared
        .runtime
        .lock()
        .expect("runtime state should remain lockable");
    assert_eq!(
        runtime
            .state
            .response_profile_bindings
            .get("resp-2")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn duplicate_non_response_continuation_verifies_do_not_requeue_persistence() {
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
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    remember_runtime_turn_state(&shared, "main", Some("turn-1"), RuntimeRouteKind::Responses)
        .expect("turn state verification should succeed");
    remember_runtime_session_id(
        &shared,
        "main",
        Some("session-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session id verification should succeed");
    remember_runtime_compact_lineage(
        &shared,
        "main",
        Some("session-compact"),
        Some("turn-compact"),
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage verification should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after initial bindings");
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(
        first_revision, 3,
        "initial binds should persist once each: {first_log}"
    );
    assert_eq!(
        first_log
            .matches("binding turn_state profile=main value=turn-1")
            .count(),
        1,
        "turn state should be logged once: {first_log}"
    );
    assert_eq!(
        first_log
            .matches("binding session_id profile=main value=session-1")
            .count(),
        1,
        "session id should be logged once: {first_log}"
    );

    remember_runtime_turn_state(&shared, "main", Some("turn-1"), RuntimeRouteKind::Responses)
        .expect("duplicate turn state verification should succeed");
    remember_runtime_session_id(
        &shared,
        "main",
        Some("session-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("duplicate session id verification should succeed");
    remember_runtime_compact_lineage(
        &shared,
        "main",
        Some("session-compact"),
        Some("turn-compact"),
        RuntimeRouteKind::Compact,
    )
    .expect("duplicate compact lineage verification should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate bindings");
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate verifies should not requeue persistence: {second_log}"
    );
    assert_eq!(
        second_log
            .matches("binding turn_state profile=main value=turn-1")
            .count(),
        1,
        "duplicate turn state verifies should not re-log: {second_log}"
    );
    assert_eq!(
        second_log
            .matches("binding session_id profile=main value=session-1")
            .count(),
        1,
        "duplicate session id verifies should not re-log: {second_log}"
    );
}

#[test]
fn previous_response_release_preserves_session_and_compact_session_lineage_for_compact_followups() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
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
        upstream_base_url: backend.base_url(),
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
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "second",
        &[String::from("resp-second")],
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "second",
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn-state affinity should be recorded");
    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact session lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        !release_runtime_previous_response_affinity(
            &shared,
            "second",
            Some("resp-second"),
            Some("turn-second"),
            Some("sess-compact"),
            RuntimeRouteKind::Responses,
        )
        .expect("first not-found should defer release")
    );
    assert!(
        release_runtime_previous_response_affinity(
            &shared,
            "second",
            Some("resp-second"),
            Some("turn-second"),
            Some("sess-compact"),
            RuntimeRouteKind::Responses,
        )
        .expect("second not-found should release response and turn-state affinity")
    );
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            !runtime
                .state
                .response_profile_bindings
                .contains_key("resp-second"),
            "released previous_response affinity should be removed"
        );
        assert!(
            !runtime.turn_state_bindings.contains_key("turn-second"),
            "released turn_state affinity should be removed"
        );
        assert!(
            runtime.session_id_bindings.contains_key("sess-compact"),
            "existing session affinity should survive response/turn-state release"
        );
        assert!(
            runtime
                .state
                .session_profile_bindings
                .contains_key("sess-compact"),
            "persisted session affinity should survive response/turn-state release"
        );
        assert!(
            runtime
                .session_id_bindings
                .contains_key(&compact_session_key),
            "compact session lineage should survive response/turn-state release"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .response
                .get("resp-second")
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .turn_state
                .get("turn-second")
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get("sess-compact")
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Verified)
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Verified)
        );
    }

    assert_eq!(
        runtime_compact_route_followup_bound_profile(&shared, None, Some("sess-compact"))
            .expect("compact session lookup should succeed"),
        Some(("second".to_string(), "session_id"))
    );

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };
    let _response = proxy_runtime_standard_request(1, &request, &shared)
        .expect("compact follow-up request should succeed");
    wait_for_runtime_background_queues_idle();

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
    let log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after compact follow-up");
    assert!(
        log.contains("compact_followup_owner profile=second source=session_id"),
        "compact follow-up should still resolve the preserved session lineage owner: {log}"
    );
}

#[test]
fn fresh_runtime_responses_rotates_after_unrecoverable_401() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_unauthorized_main();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
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
            },
            upstream_base_url: backend.base_url(),
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":"hello","stream":true}"#.to_vec(),
    };

    let response = proxy_runtime_responses_request(2, &request, &shared)
        .expect("fresh responses request should rotate after unrecoverable 401");

    match response {
        RuntimeResponsesReply::Buffered(parts) => assert_eq!(parts.status, 200),
        RuntimeResponsesReply::Streaming(stream) => assert_eq!(stream.status, 200),
    }
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("auth_failed profile=main"),
        "unrecoverable 401 should be logged as an auth failure: {log}"
    );
}

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
        profile_inflight: BTreeMap::new(),
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
        profile_inflight: BTreeMap::new(),
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
        profile_inflight: BTreeMap::new(),
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
