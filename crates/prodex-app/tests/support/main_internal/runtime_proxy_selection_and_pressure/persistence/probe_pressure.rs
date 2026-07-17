use super::*;

#[test]
fn next_runtime_response_candidate_sync_probes_cold_start_when_existing_candidate_is_auth_failed() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
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
    let runtime = RuntimeRotationState {
        paths,
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
        profile_probe_cache: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: now,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(80, 300, 80, 86_400)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_backoff_updated_at: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_auth_failure_key("main"),
            RuntimeProfileHealth {
                score: 1,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        auto_redeem_enabled: false,
        upstream_no_proxy: false,
        compact_client: reqwest::Client::new(),
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
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );

    let usage_accounts = backend.usage_accounts();
    assert!(
        !usage_accounts.is_empty(),
        "cold-start selection should probe the newly selected profile"
    );
    assert!(
        usage_accounts
            .iter()
            .all(|account| account == "second-account"),
        "cold-start selection should only probe the alternate owner: {usage_accounts:?}"
    );
    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("second"),
        "cold-start selection should cache the newly probed profile"
    );
    assert!(
        runtime.profile_usage_snapshots.contains_key("second"),
        "cold-start selection should persist the newly probed usage snapshot"
    );
}

#[test]
fn sync_probe_pressure_mode_is_route_aware_for_background_queue_pressure() {
    assert!(
        !runtime_proxy_sync_probe_pressure_mode_for_route(RuntimeRouteKind::Responses, false, true,),
        "responses sync probing should stay enabled under background queue pressure alone"
    );
    assert!(
        !runtime_proxy_sync_probe_pressure_mode_for_route(RuntimeRouteKind::Websocket, false, true,),
        "websocket sync probing should stay enabled under background queue pressure alone"
    );
    assert!(
        runtime_proxy_sync_probe_pressure_mode_for_route(RuntimeRouteKind::Compact, false, true,),
        "compact sync probing should still defer under background queue pressure"
    );
    assert!(
        runtime_proxy_sync_probe_pressure_mode_for_route(RuntimeRouteKind::Standard, false, true,),
        "standard sync probing should still defer under background queue pressure"
    );
    assert!(
        runtime_proxy_sync_probe_pressure_mode_for_route(RuntimeRouteKind::Responses, true, false,),
        "local overload should still disable sync probing for responses"
    );
}

#[test]
fn compact_and_standard_defer_sync_probe_cold_start_under_background_probe_queue_pressure() {
    for route_kind in [RuntimeRouteKind::Compact, RuntimeRouteKind::Standard] {
        let temp_dir = TestDir::isolated();
        let (listener, base_url) = unresponsive_loopback_backend_listener();
        let shared = runtime_shared_for_cold_start_probe_selection(&temp_dir, base_url);
        let pressure_guard = force_runtime_probe_refresh_backlog(
            &shared,
            RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD,
        );

        assert!(runtime_proxy_background_queue_pressure_active());
        assert!(
            runtime_proxy_sync_probe_pressure_mode_active_for_route(&shared, route_kind),
            "route should defer sync probing under background queue pressure"
        );

        let started_at = Instant::now();
        let _candidate =
            next_runtime_response_candidate_for_route(&shared, &BTreeSet::new(), route_kind)
                .expect("candidate lookup should succeed");
        assert!(
            started_at.elapsed() < ci_timing_upper_bound_ms(80, 250),
            "route should return quickly instead of waiting on a live sync probe"
        );

        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            !runtime.profile_probe_cache.contains_key("second"),
            "deferred routes should not update the cold-start probe cache inline"
        );
        assert!(
            !runtime.profile_usage_snapshots.contains_key("second"),
            "deferred routes should not update cold-start usage snapshots inline"
        );
        drop(runtime);

        let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
        assert!(
            log.contains(&format!(
                "selection_skip_sync_probe route={} reason=pressure_mode",
                runtime_route_kind_label(route_kind)
            )),
            "route should log sync-probe deferral under background queue pressure: {log}"
        );
        assert!(
            log.contains("profile_probe_refresh_queued profile=second reason=queued"),
            "deferred routes should push the cold-start probe to the background queue: {log}"
        );

        drop(pressure_guard);
        drop(listener);
        wait_for_runtime_background_queues_idle();
    }
}

#[test]
fn next_runtime_response_candidate_skips_sync_cold_start_probe_during_pressure_mode() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
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
    let runtime = RuntimeRotationState {
        paths,
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
        profile_probe_cache: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: now,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(80, 300, 80, 86_400)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_backoff_updated_at: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_auth_failure_key("main"),
            RuntimeProfileHealth {
                score: 1,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        auto_redeem_enabled: false,
        upstream_no_proxy: false,
        compact_client: reqwest::Client::new(),
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
        local_overload_backoff_until: Arc::new(AtomicU64::new(
            Local::now().timestamp().max(0) as u64 + 60,
        )),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
    assert!(
        backend.usage_accounts().is_empty(),
        "pressure mode should defer cold-start probing to the background queue"
    );

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime.profile_probe_cache.contains_key("second"),
        "pressure mode should avoid writing a fresh sync probe result"
    );
}

#[test]
fn responses_and_websocket_sync_probe_cold_start_under_background_probe_queue_pressure() {
    for route_kind in [RuntimeRouteKind::Responses, RuntimeRouteKind::Websocket] {
        let temp_dir = TestDir::isolated();
        let backend = RuntimeProxyBackend::start();
        let shared = runtime_shared_for_cold_start_probe_selection(&temp_dir, backend.base_url());
        let pressure_guard = force_runtime_probe_refresh_backlog(
            &shared,
            RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD,
        );

        assert!(runtime_proxy_background_queue_pressure_active());
        assert!(
            !runtime_proxy_sync_probe_pressure_mode_active_for_route(&shared, route_kind),
            "route should keep sync probing enabled under background queue pressure"
        );

        let candidate = if route_kind == RuntimeRouteKind::Responses {
            next_runtime_response_candidate(&shared, &BTreeSet::new())
        } else {
            next_runtime_response_candidate_for_route(&shared, &BTreeSet::new(), route_kind)
        }
        .expect("candidate lookup should succeed");
        assert_eq!(
            candidate,
            Some("second".to_string()),
            "route should still sync-probe and select the cold-start profile"
        );

        let usage_accounts = backend.usage_accounts();
        assert_eq!(
            usage_accounts,
            vec!["second-account".to_string()],
            "sync probing should only hit the newly selected cold-start profile"
        );

        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime.profile_probe_cache.contains_key("second"),
            "sync probing should refresh the second profile cache"
        );
        assert!(
            runtime.profile_usage_snapshots.contains_key("second"),
            "sync probing should refresh the second profile usage snapshot"
        );
        drop(runtime);

        let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
        assert!(
            !log.contains(&format!(
                "selection_skip_sync_probe route={}",
                runtime_route_kind_label(route_kind)
            )),
            "route should not log sync-probe deferral under background queue pressure: {log}"
        );

        drop(pressure_guard);
    }
}
