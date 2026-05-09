use super::*;

#[test]
fn clear_runtime_stale_previous_response_binding_marks_dead_tombstone() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
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

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &[String::from("resp-main")],
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_stale_previous_response_binding(&shared, "main", Some("resp-main"))
            .expect("stale clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "cleared stale binding should be removed from live affinity"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

fn runtime_shared_for_dead_response_binding_cleanup(
    temp_dir: &TestDir,
) -> RuntimeRotationProxyShared {
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    runtime_rotation_proxy_shared(
        temp_dir,
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
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
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
    )
}

#[test]
fn clear_runtime_dead_response_bindings_clears_turn_state_when_last_lineage_dies() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_dead_response_binding_cleanup(&temp_dir);
    let response_ids = vec!["resp-main".to_string(), "resp-main-next".to_string()];
    let first_lineage = runtime_response_turn_state_lineage_key("resp-main", "turn-main");
    let second_lineage = runtime_response_turn_state_lineage_key("resp-main-next", "turn-main");

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &response_ids,
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn-state affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_dead_response_bindings(&shared, "main", &response_ids, "test")
            .expect("dead binding clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "dead response id should be removed from live affinity"
    );
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main-next"),
        "follow-up response id should be removed from live affinity"
    );
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(&first_lineage),
        "dead response-turn_state lineage should be removed"
    );
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(&second_lineage),
        "all response-turn_state lineage should be removed when the chain dies"
    );
    assert!(
        !runtime.turn_state_bindings.contains_key("turn-main"),
        "direct turn_state affinity should be removed once no response lineage remains"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main-next")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .turn_state
            .get("turn-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn clear_runtime_dead_response_bindings_keeps_turn_state_when_other_lineage_survives() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_dead_response_binding_cleanup(&temp_dir);
    let remaining_lineage = runtime_response_turn_state_lineage_key("resp-main-next", "turn-main");

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &[String::from("resp-main"), String::from("resp-main-next")],
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn-state affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_dead_response_bindings(
            &shared,
            "main",
            &[String::from("resp-main")],
            "test",
        )
        .expect("dead binding clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "cleared dead response id should be removed"
    );
    assert!(
        runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main-next"),
        "live sibling response id should survive"
    );
    assert!(
        runtime
            .state
            .response_profile_bindings
            .contains_key(&remaining_lineage),
        "remaining response-turn_state lineage should survive"
    );
    assert!(
        runtime.turn_state_bindings.contains_key("turn-main"),
        "direct turn_state affinity should survive while another live lineage exists"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .turn_state
            .get("turn-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Verified)
    );
}

#[test]
fn clear_runtime_dead_response_bindings_clears_all_removed_turn_state_affinities() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_dead_response_binding_cleanup(&temp_dir);
    let response_ids = vec![String::from("resp-main")];

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &response_ids,
        Some("turn-first"),
        RuntimeRouteKind::Responses,
    )
    .expect("first response-turn_state lineage should be recorded");
    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &response_ids,
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("second response-turn_state lineage should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-first"),
        RuntimeRouteKind::Responses,
    )
    .expect("first direct turn-state affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("second direct turn-state affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_dead_response_bindings(&shared, "main", &response_ids, "test")
            .expect("dead binding clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime.turn_state_bindings.contains_key("turn-first"),
        "first removed turn_state should be cleared"
    );
    assert!(
        !runtime.turn_state_bindings.contains_key("turn-second"),
        "second removed turn_state should be cleared"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .turn_state
            .get("turn-first")
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
}

#[test]
fn websocket_success_without_turn_state_keeps_compact_lineage_alive() {
    let temp_dir = TestDir::isolated();
    let second_home = temp_dir.path.join("homes/second");
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
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
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

    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session id affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let mut previous_response_owner_recorded = false;
    remember_runtime_websocket_response_ids(
        RuntimeWebsocketResponseBindingContext {
            shared: &shared,
            profile_name: "second",
            request_previous_response_id: None,
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            response_turn_state: None,
        },
        &[String::from("resp-after-compact")],
        &mut previous_response_owner_recorded,
    )
    .expect("websocket response ids should be recorded");
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime
                .session_id_bindings
                .contains_key(&compact_session_key),
            "compact session lineage should survive until a successor turn_state exists"
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

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("turn_state_coverage route=websocket profile=second status=missing"),
        "missing websocket turn_state should be logged: {log}"
    );
    assert!(
        !log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage should not be released before a successor turn_state exists: {log}"
    );
}

#[test]
fn http_responses_success_without_turn_state_keeps_compact_lineage_alive() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
    let second_home = temp_dir.path.join("homes/second");
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
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "second".to_string(),
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

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "second",
        &[String::from("resp-second")],
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
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
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
        ],
        body: br#"{"input":[],"stream":true,"previous_response_id":"resp-second"}"#.to_vec(),
    };
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(&shared, "second", "responses_http")
            .expect("responses inflight guard should be acquired");
    let response =
        send_runtime_proxy_upstream_responses_request(1, &request, &shared, "second", None)
            .expect("responses follow-up should reach upstream");
    match prepare_runtime_proxy_responses_success(
        RuntimeResponsesSuccessContext {
            request_id: 1,
            request_model_name: None,
            request_previous_response_id: Some("resp-second"),
            request_prompt_cache_key: None,
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            turn_state_override: None,
            shared: &shared,
            profile_name: "second",
            inflight_guard,
        },
        response,
    )
    .expect("responses follow-up should prepare successfully")
    {
        RuntimeResponsesAttempt::Success { .. } => {}
        _ => panic!("unexpected non-success responses attempt"),
    }
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime
                .session_id_bindings
                .contains_key(&compact_session_key),
            "compact session lineage should survive until a successor turn_state exists"
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

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("turn_state_coverage route=responses profile=second status=missing"),
        "missing responses turn_state should be logged: {log}"
    );
    assert!(
        !log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage should not be released before a successor turn_state exists: {log}"
    );
}

#[test]
fn websocket_success_with_turn_state_releases_compact_lineage() {
    let temp_dir = TestDir::isolated();
    let second_home = temp_dir.path.join("homes/second");
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
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
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

    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session id affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let mut previous_response_owner_recorded = false;
    remember_runtime_websocket_response_ids(
        RuntimeWebsocketResponseBindingContext {
            shared: &shared,
            profile_name: "second",
            request_previous_response_id: None,
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            response_turn_state: Some("turn-successor"),
        },
        &[String::from("resp-after-compact")],
        &mut previous_response_owner_recorded,
    )
    .expect("websocket response ids should be recorded");
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            !runtime
                .session_id_bindings
                .contains_key(&compact_session_key),
            "compact session lineage should be released once a successor turn_state exists"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
    }

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage release should be logged after successor turn_state is established: {log}"
    );
}

#[test]
fn http_responses_success_with_turn_state_releases_compact_lineage() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_sse_headers_array_turn_state();
    let second_home = temp_dir.path.join("homes/second");
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
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "second".to_string(),
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
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
        ],
        body: br#"{"input":[],"stream":true}"#.to_vec(),
    };
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(&shared, "second", "responses_http")
            .expect("responses inflight guard should be acquired");
    let response =
        send_runtime_proxy_upstream_responses_request(1, &request, &shared, "second", None)
            .expect("responses request should reach upstream");
    match prepare_runtime_proxy_responses_success(
        RuntimeResponsesSuccessContext {
            request_id: 1,
            request_model_name: None,
            request_previous_response_id: None,
            request_prompt_cache_key: None,
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            turn_state_override: None,
            shared: &shared,
            profile_name: "second",
            inflight_guard,
        },
        response,
    )
    .expect("responses request should prepare successfully")
    {
        RuntimeResponsesAttempt::Success { .. } => {}
        _ => panic!("unexpected non-success responses attempt"),
    }
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime.turn_state_bindings.contains_key("turn-second"),
            "successor response turn_state should be remembered before compact lineage release"
        );
        assert!(
            !runtime
                .session_id_bindings
                .contains_key(&compact_session_key),
            "compact session lineage should be released once a successor turn_state exists"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
    }

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage release should be logged after successor turn_state is established: {log}"
    );
}

#[test]
fn runtime_rotation_proxy_can_start_even_if_selected_profile_auth_is_not_quota_compatible() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    create_codex_home_if_missing(&main_home).expect("main home should be created");
    write_auth_json(&second_home.join("auth.json"), "second-account");

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

    assert!(should_enable_runtime_rotation_proxy(&state, "main", true));
}

#[test]
fn runtime_rotation_proxy_stays_disabled_without_any_quota_compatible_profile() {
    let temp_dir = TestDir::isolated();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/main"),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/second"),
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

    assert!(!should_enable_runtime_rotation_proxy(&state, "main", true));
}

#[test]
fn optimistic_current_candidate_skips_transport_backoff() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
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

    mark_runtime_profile_transport_backoff(&shared, "main", RuntimeRouteKind::Responses, "test")
        .expect("transport backoff should be recorded");

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn precommit_budget_exhausts_by_attempt_limit_or_elapsed_time() {
    assert!(!runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        0,
        false,
        false
    ));
    assert!(runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        false,
        false
    ));

    let started_at = Instant::now()
        .checked_sub(Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS + 1))
        .expect("elapsed start should be constructible");
    assert!(runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, false
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        true,
        false
    ));
}

#[test]
fn websocket_previous_response_not_found_requires_stale_continuation_without_turn_state() {
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "locked-affinity websocket continuations without replayable turn state must fail locally"
    );
    assert!(
        !runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            true,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "available replay turn state should keep previous_response retries alive"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        ),
        "session metadata is not enough to replay a missing previous_response chain"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "continuation-only websocket requests without turn state must fail locally instead of surfacing raw upstream 400s"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            None,
        ),
        "unknown fallback shape should stay conservative when websocket turn state is missing"
    );
    assert!(
        !runtime_websocket_previous_response_not_found_requires_stale_continuation(
            None,
            false,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "requests without previous_response_id should not be classified as stale continuations"
    );
}

#[test]
fn websocket_reuse_watchdog_fresh_fallback_stays_blocked_for_locked_affinity() {
    assert!(
        !runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
            RuntimeWebsocketReuseWatchdogPreviousResponseFallback {
                profile_name: "second",
                previous_response_id: Some("resp-second"),
                previous_response_fresh_fallback_used: false,
                bound_profile: Some("second"),
                pinned_profile: None,
                request_requires_previous_response_affinity: true,
                trusted_previous_response_affinity: false,
                request_turn_state: None,
                fresh_fallback_shape: Some(
                    RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                ),
            },
        ),
        "watchdog fallback should stay blocked for non-replayable locked-affinity continuations"
    );
}

#[test]
fn noncompact_session_priority_ignores_compact_session_profile() {
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("main"), Some("main")),
        None
    );
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("second"), Some("main")),
        Some("second")
    );
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("main"), None),
        Some("main")
    );
}

#[test]
fn optimistic_current_candidate_requires_quota_evidence_when_alternatives_exist() {
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
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("responses candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
        )
        .expect("websocket candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )
        .expect("compact candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_recently_unhealthy_profile() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
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
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: Local::now().timestamp(),
            },
        )]),
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

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_busy_profile() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
        )]),
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

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_thin_long_lived_quota() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
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
        profile_probe_cache: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: now,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(9, 18_000, 18, 604_800)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
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

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_cached_usage_exhausted_profile() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
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
        profile_probe_cache: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: Local::now().timestamp(),
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(0, 18_000, 50, 604_800)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
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

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn direct_current_fallback_profile_bypasses_local_selection_penalties() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
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
        profile_retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
        profile_transport_backoff_until: BTreeMap::from([(
            runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Standard),
            now + 60,
        )]),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
        )]),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: now,
            },
        )]),
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

    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("direct fallback lookup should succeed"),
        Some("main".to_string())
    );
}

#[test]
fn direct_current_fallback_profile_is_route_aware_for_heavy_routes() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT.saturating_sub(1),
        )]),
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

    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard direct fallback lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("responses direct fallback lookup should succeed"),
        None
    );
}
