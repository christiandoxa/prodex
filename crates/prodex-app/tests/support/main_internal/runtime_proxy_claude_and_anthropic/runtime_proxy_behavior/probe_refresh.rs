use super::*;

#[test]
fn runtime_probe_refresh_apply_waits_for_busy_runtime_state() {
    let _probe_refresh = RuntimeProbeRefreshTestGuard::new();
    let temp_dir = TestDir::isolated();
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
            state: AppState::default(),
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
        },
        usize::MAX,
    );

    let runtime_guard = shared.runtime.lock().expect("runtime lock should succeed");
    let apply_shared = shared.clone();
    let (apply_started_tx, apply_started_rx) = std::sync::mpsc::channel();
    let apply_thread = thread::spawn(move || {
        apply_started_tx
            .send(())
            .expect("apply thread should signal startup");
        apply_runtime_profile_probe_result(
            &apply_shared,
            "main",
            AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            Ok(usage_with_main_windows(90, 3600, 90, 604_800)),
        )
    });
    apply_started_rx
        .recv_timeout(Duration::from_millis(100))
        .expect("apply thread should start promptly");
    thread::sleep(Duration::from_millis(20));

    assert!(
        !apply_thread.is_finished(),
        "probe apply should remain blocked while the runtime lock is held"
    );

    drop(runtime_guard);
    apply_thread
        .join()
        .expect("apply thread should join")
        .expect("probe apply should succeed after the runtime lock is released");
    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("main"),
        "probe apply should update the probe cache after waiting for the runtime lock"
    );
    assert!(
        runtime.profile_usage_snapshots.contains_key("main"),
        "probe apply should update usage snapshots after waiting for the runtime lock"
    );
}

#[test]
fn runtime_probe_inline_execution_logs_context_without_queue_lag() {
    let temp_dir = TestDir::isolated();
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
            state: AppState::default(),
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
        },
        usize::MAX,
    );

    execute_runtime_probe_attempt_inline_for_test(
        &shared,
        "main",
        "startup_probe_warmup",
        Err("timeout".to_string()),
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("startup_probe_warmup_error profile=main error=timeout"),
        "inline probe execution should keep the context-specific marker: {log}"
    );
    assert!(
        !log.contains("startup_probe_warmup_error profile=main lag_ms="),
        "inline probe execution should not report queue lag: {log}"
    );
}

#[test]
fn runtime_probe_queued_execution_logs_refresh_marker_with_queue_lag() {
    let temp_dir = TestDir::isolated();
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
            state: AppState::default(),
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
        },
        usize::MAX,
    );

    execute_runtime_probe_attempt_queued_for_test(
        &shared,
        "main",
        Err("timeout".to_string()),
        Duration::from_millis(50),
        Instant::now(),
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("profile_probe_refresh_error profile=main lag_ms="),
        "queued probe execution should keep the queue-aware marker: {log}"
    );
    assert!(
        log.contains("error=timeout"),
        "queued probe execution should preserve the fetch error: {log}"
    );
}

#[test]
fn runtime_probe_refresh_suppresses_nonlocal_upstream_in_tests_and_wakes_waiters() {
    let probe_refresh = RuntimeProbeRefreshTestGuard::new();
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
                        codex_home: main_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
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
        },
        usize::MAX,
    );

    schedule_runtime_probe_refresh(&shared, "main", &main_home);

    assert_eq!(
        runtime_probe_refresh_queue_backlog(),
        probe_refresh.backlog_before(),
        "suppressed nonlocal probe refresh should not add background queue work"
    );
    assert!(
        wait_for_runtime_probe_refresh_since(
            ci_timing_upper_bound_ms(20, 200),
            probe_refresh.observed_revision(),
        ),
        "suppressed nonlocal probe refresh should still wake probe-refresh waiters"
    );

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime.profile_probe_cache.contains_key("main"),
        "suppressed nonlocal probe refresh should not write probe cache state"
    );
    drop(runtime);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("profile_probe_refresh_suppressed profile=main reason=test_nonlocal_upstream"),
        "suppressed nonlocal probe refresh should be logged"
    );
}

#[test]
fn runtime_probe_refresh_nonlocal_upstream_detection_keeps_loopback_exact() {
    assert!(
        runtime_probe_refresh_nonlocal_upstream_for_test("https://chatgpt.com/backend-api"),
        "nonlocal upstreams should stay suppressed in tests"
    );
    assert!(
        runtime_probe_refresh_nonlocal_upstream_for_test(
            "https://localhost.example.com/backend-api"
        ),
        "host matching should stay exact instead of substring-based"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://localhost:1234/backend-api"),
        "localhost loopback should stay allowed in tests"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://127.0.0.1:1234/backend-api"),
        "IPv4 loopback should stay allowed in tests"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://[::1]:1234/backend-api"),
        "IPv6 loopback should stay allowed in tests"
    );
}

#[test]
fn runtime_probe_refresh_allows_loopback_upstream_in_tests() {
    let probe_refresh = RuntimeProbeRefreshTestGuard::new();
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
                        codex_home: main_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: closed_loopback_backend_base_url(),
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
        },
        usize::MAX,
    );

    schedule_runtime_probe_refresh(&shared, "main", &main_home);

    assert!(
        wait_for_runtime_probe_refresh_since(
            ci_timing_upper_bound_ms(250, 1_000),
            probe_refresh.observed_revision()
        ),
        "loopback upstreams should still run through the background refresh path in tests"
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("main"),
        "loopback probe refresh should still apply a probe result in tests"
    );
    drop(runtime);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        !log.contains(
            "profile_probe_refresh_suppressed profile=main reason=test_nonlocal_upstream"
        ),
        "loopback probe refresh should not be suppressed in tests"
    );
}
