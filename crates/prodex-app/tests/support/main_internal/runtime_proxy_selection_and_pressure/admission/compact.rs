use super::*;
use super::helpers::*;

#[test]
fn session_affinity_prefers_bound_profile_for_compact_requests() {
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
        session_id_bindings: BTreeMap::from([(
            "sess-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
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
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        auto_redeem_enabled: false,
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
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-second".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let _response = proxy_runtime_standard_request(1, &request, &shared)
        .expect("session-bound compact request should succeed");

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_pressure_mode_sheds_fresh_compact_requests_before_upstream() {
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
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
    let pressure_until = Local::now().timestamp().saturating_add(60).max(0) as u64;
    let shared = RuntimeRotationProxyShared {
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        auto_redeem_enabled: false,
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
        local_overload_backoff_until: Arc::new(AtomicU64::new(pressure_until)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(1, &request, &shared)
        .expect("fresh compact request should receive a local response");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 503);
    assert!(
        body.contains("Fresh compact requests are temporarily deferred"),
        "unexpected compact pressure response body: {body}"
    );
    assert!(
        backend.responses_accounts().is_empty(),
        "fresh compact request should be shed before reaching upstream"
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("compact_final_failure exit=pressure reason=pressure"),
        "compact pressure failure should emit a terminal marker: {log}"
    );
}

#[test]
fn compact_smart_context_prepare_fallback_passes_original_body_to_upstream() {
    let backend = RuntimeProxyBackend::start();
    let harness =
        RuntimeProxyProfileHarnessBuilder::single_openai_profile(
            "main",
            "main-account",
            "main@example.com",
        )
        .upstream_base_url(backend.base_url())
        .build();
    let shared = harness.shared();
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    let body = serde_json::json!({
        "input": [
            {
                "type": "function_call_output",
                "call_id": "call_big",
                "output": "large compact payload\n".repeat(128)
            }
        ],
        "instructions": "compact",
        "session_id": "sess-main"
    })
    .to_string();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-main".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: body.as_bytes().to_vec(),
    };

    let response = {
        let _fault = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE", "1");
        proxy_runtime_standard_request(44, &request, shared)
            .expect("compact request should survive smart-context prepare fallback")
    };
    assert!(!runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE"
    ));
    let (status, response_body) = tiny_http_response_status_and_body(response);
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(status, 200, "unexpected compact response: {response_body}");
    assert_eq!(backend.responses_accounts(), vec!["main-account".to_string()]);
    assert_eq!(backend.responses_bodies(), vec![body]);
    assert!(
        log.contains("smart_context_prepare_fallback")
            && log.contains("route=compact")
            && log.contains("decision=pass_through"),
        "smart-context prepare fallback should be logged as compact pass-through: {log}"
    );
    assert!(
        log.contains("upstream_start"),
        "compact request should still reach upstream after smart-context fallback: {log}"
    );
}

#[test]
fn compact_transport_timeout_rotates_fresh_request_to_next_profile() {
    let _compact_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS", "300");
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new(
        [RuntimeProxyBackendFaultStep::stalled_json(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
            Duration::from_millis(400),
        )],
    ));
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86_400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    let shared = harness.shared();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(45, &request, shared)
        .expect("fresh compact transport failure should rotate before returning");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(
        status, 200,
        "unexpected compact response body: {body}; log: {log}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains(
            "compact_transport_failure profile=main route=compact stage=compact_forward_response"
        ) && log.contains("profile_transport_backoff")
            && log.contains("route=compact")
            && log.contains("compact_committed profile=second"),
        "compact transport timeout should back off main and commit second: {log}"
    );
}

#[test]
fn compact_final_failure_logs_overload_terminal_reason() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let now = Local::now().timestamp();
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
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 3600,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 80,
                    weekly_reset_at: now + 86_400,
                },
            )]),
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
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(41, &request, &shared)
        .expect("compact overload request should return upstream failure");
    let (status, _body) = tiny_http_response_status_and_body(response);
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(status, 500);
    let overload_terminal_marker =
        log.contains("compact_final_failure exit=candidate_exhausted reason=overload")
            || log.contains(
                "compact_final_failure exit=precommit_budget_exhausted reason=overload",
            );
    assert!(
        overload_terminal_marker,
        "compact overload terminal marker should identify overload exhaustion: {log}"
    );
    assert!(
        log.contains("last_failure=overload"),
        "compact overload terminal marker should preserve the last failure class: {log}"
    );
}

#[test]
fn compact_final_failure_logs_quota_terminal_reason() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
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
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::from([(
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )]),
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
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-main".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(42, &request, &shared)
        .expect("hard-affinity compact quota request should return upstream failure");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(status, 429);
    assert!(
        body.contains("The usage limit has been reached"),
        "unexpected compact quota response body: {body}"
    );
    assert!(
        log.contains("compact_final_failure exit=quota_fallback_exhausted reason=quota"),
        "compact quota terminal marker should identify quota exhaustion: {log}"
    );
}

#[test]
fn session_affinity_compact_quota_rotates_to_ready_profile() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
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
            session_id_bindings: BTreeMap::from([(
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )]),
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
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-main".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(43, &request, &shared)
        .expect("compact quota request should rotate to another ready profile");
    let (status, body) = tiny_http_response_status_and_body(response);
    let responses_accounts = backend.responses_accounts();
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(status, 200);
    assert!(
        !body.contains("usage limit"),
        "compact quota error should not leak after rotating to another ready profile: {body}"
    );
    assert_eq!(
        responses_accounts,
        vec!["main-account".to_string(), "second-account".to_string()],
        "compact should try the bound owner first, then rotate to the ready profile"
    );
    assert!(
        log.contains("quota_blocked_affinity_released profile=main route=compact"),
        "compact quota retry should release the blocked affinity: {log}"
    );
    assert!(
        log.contains("compact_committed profile=second"),
        "compact should commit the later ready profile after quota rotation: {log}"
    );
}

#[test]
fn compact_workspace_credits_error_rotates_to_ready_profile() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_workspace_credits_exhausted();
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
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(46, &request, &shared)
        .expect("workspace credits compact request should rotate to another ready profile");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(status, 200, "unexpected compact response body: {body}");
    assert!(
        !body.contains("workspace is out of credits"),
        "workspace credits error should not leak after rotating to another ready profile: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()],
        "compact should try the active profile first, then rotate to the ready profile"
    );
    assert!(
        log.contains("compact_retryable_failure profile=main reason=quota")
            && log.contains("compact_committed profile=second"),
        "compact should classify workspace credits as quota and commit the ready profile: {log}"
    );
}

#[test]
fn compact_final_failure_logs_local_selection_terminal_reason() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let now = Local::now().timestamp();
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
            profile_usage_snapshots: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                    five_hour_remaining_percent: 0,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 90,
                    weekly_reset_at: now + 86_400,
                },
            )]),
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
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(43, &request, &shared)
        .expect("exhausted compact request should receive a local failure");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(status, 503);
    assert!(
        body.contains(runtime_proxy_local_selection_failure_message()),
        "unexpected compact local-selection response body: {body}"
    );
    assert!(
        log.contains(
            "compact_final_failure exit=candidate_exhausted_fallback reason=local_selection"
        ),
        "compact local-selection terminal marker should identify fallback-local-selection: {log}"
    );
    assert!(
        log.contains("last_failure=none"),
        "compact local-selection terminal marker should preserve that no upstream failure existed: {log}"
    );
}

#[test]
fn compact_final_failure_logs_inflight_saturation_terminal_reason() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
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
            upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
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
                        five_hour_status: RuntimeQuotaWindowStatus::Ready,
                        five_hour_remaining_percent: 80,
                        five_hour_reset_at: now + 3600,
                        weekly_status: RuntimeQuotaWindowStatus::Ready,
                        weekly_remaining_percent: 80,
                        weekly_reset_at: now + 86_400,
                    },
                ),
                (
                    "second".to_string(),
                    RuntimeProfileUsageSnapshot {
                        checked_at: now,
                        five_hour_status: RuntimeQuotaWindowStatus::Ready,
                        five_hour_remaining_percent: 75,
                        five_hour_reset_at: now + 3600,
                        weekly_status: RuntimeQuotaWindowStatus::Ready,
                        weekly_remaining_percent: 78,
                        weekly_reset_at: now + 86_400,
                    },
                ),
            ]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::from([
                ("main".to_string(), hard_limit),
                ("second".to_string(), hard_limit),
            ]),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(44, &request, &shared)
        .expect("saturated compact request should receive a local failure");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");

    assert_eq!(status, 503);
    assert!(
        body.contains("temporarily saturated"),
        "unexpected compact inflight saturation response body: {body}"
    );
    assert!(
        log.contains("compact_final_failure exit=candidate_exhausted reason=inflight_saturation"),
        "compact saturation terminal marker should identify inflight saturation: {log}"
    );
    assert!(
        log.contains("saw_inflight_saturation=true"),
        "compact saturation terminal marker should preserve the saturation flag: {log}"
    );
}
