use super::*;

fn selection_health_shared(
    temp_dir: &TestDir,
    profile_inflight: BTreeMap<String, usize>,
    profile_health: Vec<(String, u32)>,
) -> RuntimeRotationProxyShared {
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
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
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            ("main".to_string(), selection_health_probe_entry(now)),
            ("second".to_string(), selection_health_probe_entry(now)),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight,
        profile_health: profile_health
            .into_iter()
            .map(|(key, score)| {
                (
                    key,
                    RuntimeProfileHealth {
                        score,
                        updated_at: now,
                    },
                )
            })
            .collect(),
    };

    RuntimeRotationProxyShared {
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
    }
}

fn selection_health_probe_entry(checked_at: i64) -> RuntimeProfileProbeCacheEntry {
    RuntimeProfileProbeCacheEntry {
        checked_at,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
    }
}

#[test]
fn compact_bad_pairing_does_not_degrade_responses_selection() {
    let temp_dir = TestDir::isolated();
    let shared = selection_health_shared(
        &temp_dir,
        BTreeMap::new(),
        vec![(
            runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Compact),
            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
        )],
    );

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("responses optimistic candidate should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        next_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )
        .expect("compact candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_prefers_less_loaded_profile() {
    let temp_dir = TestDir::isolated();
    let shared = selection_health_shared(
        &temp_dir,
        BTreeMap::from([("main".to_string(), 2), ("second".to_string(), 0)]),
        Vec::new(),
    );

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_prefers_lower_latency_penalty() {
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
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_performance_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 9,
                updated_at: now,
            },
        )]),
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

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn compact_health_penalty_does_not_degrade_responses_selection() {
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
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Compact),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                updated_at: now,
            },
        )]),
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

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("responses optimistic candidate should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        next_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )
        .expect("compact candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn websocket_bad_pairing_lightly_degrades_responses_selection() {
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
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                updated_at: now,
            },
        )]),
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

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("responses candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_prefers_healthier_quota_window_mix() {
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
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(4, 18_000, 30, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(25, 18_000, 12, 604_800)),
                },
            ),
        ]),
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

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}
