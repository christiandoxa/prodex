use super::*;

#[test]
fn note_runtime_profile_auth_failure_applies_stronger_penalty_for_401_than_403() {
    let temp_dir_401 = TestDir::isolated();
    let temp_dir_403 = TestDir::isolated();
    let profile_home_401 = temp_dir_401.path.join("homes/main");
    let profile_home_403 = temp_dir_403.path.join("homes/main");
    write_auth_json(&profile_home_401.join("auth.json"), "main-account");
    write_auth_json(&profile_home_403.join("auth.json"), "main-account");

    let shared_401 = runtime_rotation_proxy_shared(
        &temp_dir_401,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir_401.path.join("prodex"),
                state_file: temp_dir_401.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir_401.path.join("prodex/profiles"),
                shared_codex_root: temp_dir_401.path.join("shared"),
                legacy_shared_codex_root: temp_dir_401.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: profile_home_401,
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
    let shared_403 = runtime_rotation_proxy_shared(
        &temp_dir_403,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir_403.path.join("prodex"),
                state_file: temp_dir_403.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir_403.path.join("prodex/profiles"),
                shared_codex_root: temp_dir_403.path.join("shared"),
                legacy_shared_codex_root: temp_dir_403.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: profile_home_403,
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

    note_runtime_profile_auth_failure(&shared_401, "main", RuntimeRouteKind::Responses, 401);
    note_runtime_profile_auth_failure(&shared_403, "main", RuntimeRouteKind::Responses, 403);

    let score_401 = shared_401
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_health
        .get(&runtime_profile_auth_failure_key("main"))
        .expect("401 auth failure score should exist")
        .score;
    let score_403 = shared_403
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_health
        .get(&runtime_profile_auth_failure_key("main"))
        .expect("403 auth failure score should exist")
        .score;

    assert!(score_401 > score_403);
    wait_for_runtime_background_queues_idle();
}

#[test]
fn next_runtime_response_candidate_skips_auth_failed_profile() {
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
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 300,
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
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 90,
                    weekly_reset_at: now + 86_400,
                },
            ),
        ]),
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
}
