use super::*;

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
