#[test]
fn merge_runtime_continuation_store_keeps_compact_session_release_tombstone() {
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let key = runtime_compact_session_lineage_key("sess-compact");

    let existing = RuntimeContinuationStore {
        session_id_bindings: BTreeMap::from([(
            key.clone(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        statuses: RuntimeContinuationStatuses {
            session_id: BTreeMap::from([(
                key.clone(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Verified,
                    confidence: 2,
                    last_touched_at: Some(now - 30),
                    last_verified_at: Some(now - 30),
                    last_verified_route: Some("compact".to_string()),
                    last_not_found_at: None,
                    not_found_streak: 0,
                    success_count: 1,
                    failure_count: 0,
                },
            )]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };
    let incoming = RuntimeContinuationStore {
        statuses: RuntimeContinuationStatuses {
            session_id: BTreeMap::from([(
                key.clone(),
                RuntimeContinuationBindingStatus {
                    last_verified_route: Some("compact".to_string()),
                    ..dead_continuation_status(now)
                },
            )]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };

    let merged = merge_runtime_continuation_store(&existing, &incoming, &profiles);
    assert!(
        !merged.session_id_bindings.contains_key(&key),
        "compact session binding should be removed when a newer release tombstone exists"
    );
    assert_eq!(
        merged
            .statuses
            .session_id
            .get(&key)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_proxy_pressure_mode_shrinks_precommit_budget() {
    let started_at = Instant::now()
        .checked_sub(Duration::from_millis(
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS + 5,
        ))
        .expect("checked_sub should succeed");

    assert!(runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, true
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, false
    ));
}

#[test]
fn turn_state_affinity_prefers_bound_profile() {
    let temp_dir = TestDir::new();
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
        turn_state_bindings: BTreeMap::from([(
            "turn-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let turn_state_profile = runtime_turn_state_bound_profile(&shared, "turn-second")
        .expect("turn-state lookup should succeed");

    assert_eq!(
        select_runtime_response_candidate(
            &shared,
            &BTreeSet::new(),
            None,
            turn_state_profile.as_deref(),
            None,
            false,
        )
        .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn turn_state_affinity_ignores_inflight_and_health_penalties() {
    let temp_dir = TestDir::new();
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
        turn_state_bindings: BTreeMap::from([(
            "turn-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::from([(
            "second".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT + 1,
        )]),
        profile_health: BTreeMap::from([(
            "second".to_string(),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_HEALTH_MAX_SCORE,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let turn_state_profile = runtime_turn_state_bound_profile(&shared, "turn-second")
        .expect("turn-state lookup should succeed");

    assert_eq!(
        select_runtime_response_candidate(
            &shared,
            &BTreeSet::new(),
            None,
            turn_state_profile.as_deref(),
            None,
            false,
        )
        .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn runtime_binding_touch_waits_for_full_interval_with_second_precision() {
    let now = Local::now().timestamp();

    assert!(
        !runtime_binding_touch_should_persist(
            now - RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS,
            now,
        ),
        "second-precision timestamps should not persist exactly on the coarse boundary"
    );
    assert!(
        runtime_binding_touch_should_persist(
            now - RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS - 1,
            now,
        ),
        "touch persistence should still trigger once the full interval has clearly elapsed"
    );
}

#[test]
fn response_affinity_touch_persists_recent_use_for_housekeeping() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let stale_touch = now - RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS - 2;
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
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: stale_touch,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    state
        .save(&paths)
        .expect("initial state save should succeed");

    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: state.clone(),
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
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner.as_deref(), Some("main"));

    let persisted = wait_for_state(&paths, |state| {
        state
            .response_profile_bindings
            .get("resp-main")
            .is_some_and(|binding| binding.bound_at > stale_touch)
    });
    assert!(
        persisted
            .response_profile_bindings
            .get("resp-main")
            .is_some_and(|binding| binding.bound_at > stale_touch)
    );
}

#[test]
fn response_affinity_skips_recent_negative_cache_for_same_route() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
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
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
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
                runtime_previous_response_negative_cache_key(
                    "resp-main",
                    "main",
                    RuntimeRouteKind::Responses,
                ),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now,
                },
            )]),
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner, None);
}

#[test]
fn response_affinity_skips_dead_continuation_status() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
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
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Dead,
                        confidence: 0,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now - 30),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: Some(now - 5),
                        not_found_streak: 2,
                        success_count: 1,
                        failure_count: 2,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner, None);
}

#[test]
fn response_affinity_keeps_stale_verified_continuation_status_bound() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let stale_at = now - RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS - 1;
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
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: stale_at,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 3,
                        last_touched_at: Some(stale_at),
                        last_verified_at: Some(stale_at),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner.as_deref(), Some("main"));
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime lock should succeed")
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Warm)
    );
}

#[test]
fn session_affinity_skips_stale_verified_continuation_status() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let stale_at = now - RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS - 1;
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
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: stale_at,
            },
        )]),
    };

    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::from([(
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_at,
                },
            )]),
            continuation_statuses: RuntimeContinuationStatuses {
                session_id: BTreeMap::from([(
                    "sess-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 3,
                        last_touched_at: Some(stale_at),
                        last_verified_at: Some(stale_at),
                        last_verified_route: Some("compact".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    let owner =
        runtime_session_bound_profile(&shared, "sess-main").expect("session lookup should succeed");
    assert_eq!(owner, None);
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime lock should succeed")
            .continuation_statuses
            .session_id
            .get("sess-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Warm)
    );
}

fn previous_response_affinity_test_shared(
    temp_dir: &TestDir,
    response_id: &str,
    now: i64,
    profile_health: BTreeMap<String, RuntimeProfileHealth>,
) -> RuntimeRotationProxyShared {
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
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
        response_profile_bindings: BTreeMap::from([(
            response_id.to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
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
            profile_health,
        })),
    }
}

#[test]
fn previous_response_affinity_release_requires_repeated_not_found() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let shared =
        previous_response_affinity_test_shared(&temp_dir, "resp-main", now, BTreeMap::new());

    assert!(
        !release_runtime_previous_response_affinity(
            &shared,
            "main",
            Some("resp-main"),
            None,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("first not-found should defer hard release")
    );
    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime lock")
            .state
            .response_profile_bindings
            .contains_key("resp-main")
    );

    assert!(
        release_runtime_previous_response_affinity(
            &shared,
            "main",
            Some("resp-main"),
            None,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("second not-found should release affinity")
    );
    assert!(
        !shared
            .runtime
            .lock()
            .expect("runtime lock")
            .state
            .response_profile_bindings
            .contains_key("resp-main")
    );
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime lock")
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn previous_response_affinity_release_triggers_at_negative_cache_threshold() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let response_id = "resp-main";
    let negative_cache_key = runtime_previous_response_negative_cache_key(
        response_id,
        "main",
        RuntimeRouteKind::Responses,
    );
    let seeded_failures = RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD - 1;
    let shared = previous_response_affinity_test_shared(
        &temp_dir,
        response_id,
        now,
        BTreeMap::from([(
            negative_cache_key.clone(),
            RuntimeProfileHealth {
                score: seeded_failures,
                updated_at: now,
            },
        )]),
    );

    assert!(
        release_runtime_previous_response_affinity(
            &shared,
            "main",
            Some(response_id),
            None,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("threshold-reaching not-found should release affinity")
    );

    let runtime = shared.runtime.lock().expect("runtime lock");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(response_id),
        "binding should release once negative cache hits threshold"
    );
    assert_eq!(
        runtime
            .profile_health
            .get(&negative_cache_key)
            .map(|health| health.score),
        Some(RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get(response_id)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn previous_response_affinity_ignores_expired_negative_cache_until_threshold_rebuilds() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let response_id = "resp-main";
    let negative_cache_key = runtime_previous_response_negative_cache_key(
        response_id,
        "main",
        RuntimeRouteKind::Responses,
    );
    let shared = previous_response_affinity_test_shared(
        &temp_dir,
        response_id,
        now,
        BTreeMap::from([(
            negative_cache_key.clone(),
            RuntimeProfileHealth {
                score: RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD - 1,
                updated_at: now - RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS - 1,
            },
        )]),
    );

    assert!(
        !release_runtime_previous_response_affinity(
            &shared,
            "main",
            Some(response_id),
            None,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("expired negative cache should not force early release")
    );

    {
        let runtime = shared.runtime.lock().expect("runtime lock");
        assert!(
            runtime
                .state
                .response_profile_bindings
                .contains_key(response_id),
            "expired failures should not release affinity on first fresh miss"
        );
        assert_eq!(
            runtime
                .profile_health
                .get(&negative_cache_key)
                .map(|health| health.score),
            Some(1)
        );
    }

    assert!(
        release_runtime_previous_response_affinity(
            &shared,
            "main",
            Some(response_id),
            None,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("freshly rebuilt threshold should still release on second miss")
    );

    let runtime = shared.runtime.lock().expect("runtime lock");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(response_id),
        "binding should release after fresh misses rebuild threshold"
    );
    assert_eq!(
        runtime
            .profile_health
            .get(&negative_cache_key)
            .map(|health| health.score),
        Some(RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD)
    );
}

#[test]
fn runtime_continuation_status_pruning_uses_evidence_over_age() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let stale_bound_at = now - 365 * 24 * 60 * 60;
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
        Some(RuntimeRouteKind::Responses),
    ));
    assert_eq!(
        statuses
            .response
            .get("resp-main")
            .and_then(|status| status.last_verified_route.as_deref()),
        Some("responses")
    );
    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now + 1,
    ));

    let retained = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_bound_at,
                },
            )]),
            statuses: statuses.clone(),
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );
    assert!(retained.response_profile_bindings.contains_key("resp-main"));
    assert_eq!(
        retained
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Suspect)
    );

    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now + 2,
    ));

    let pruned = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_bound_at,
                },
            )]),
            statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );
    assert!(!pruned.response_profile_bindings.contains_key("resp-main"));
    assert_eq!(
        pruned
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_dead_continuation_tombstone_blocks_stale_binding_resurrection() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut tombstone_statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut tombstone_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 2,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut tombstone_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
    ));

    let merged = merge_runtime_continuation_store(
        &RuntimeContinuationStore {
            statuses: tombstone_statuses,
            ..RuntimeContinuationStore::default()
        },
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now - 1,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert!(
        !merged.response_profile_bindings.contains_key("resp-main"),
        "dead tombstone should prune stale resurrected binding"
    );
    assert_eq!(
        merged
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_dead_continuation_tombstone_overrides_same_second_verified_status() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut existing_statuses = RuntimeContinuationStatuses::default();
    let mut incoming_statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut existing_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut incoming_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
    ));

    let merged = merge_runtime_continuation_store(
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: existing_statuses,
            ..RuntimeContinuationStore::default()
        },
        &RuntimeContinuationStore {
            statuses: incoming_statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert!(
        !merged.response_profile_bindings.contains_key("resp-main"),
        "same-second dead tombstone should still prune the released binding"
    );
    assert_eq!(
        merged
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_newer_binding_overrides_older_dead_tombstone() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 5,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 3,
    ));

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted
            .response_profile_bindings
            .get("resp-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert!(
        !compacted.statuses.response.contains_key("resp-main"),
        "older dead tombstone should not suppress a newer binding"
    );
}

#[test]
fn runtime_continuation_store_compaction_prunes_response_bindings_to_limit() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut response_profile_bindings = BTreeMap::new();
    for index in 0..(RESPONSE_PROFILE_BINDING_LIMIT + 1) {
        response_profile_bindings.insert(
            format!("resp-{index:06}"),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - (RESPONSE_PROFILE_BINDING_LIMIT + 1) as i64 + index as i64,
            },
        );
    }
    let hot_response_id = "resp-000000".to_string();
    let displaced_response_id = "resp-000001".to_string();

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    hot_response_id.clone(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 3,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted.response_profile_bindings.len(),
        RESPONSE_PROFILE_BINDING_LIMIT
    );
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&hot_response_id),
        "verified hot response binding should survive compaction even when it is oldest"
    );
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&format!("resp-{0:06}", RESPONSE_PROFILE_BINDING_LIMIT)),
        "newest cold response binding should still be retained"
    );
    assert!(
        !compacted
            .response_profile_bindings
            .contains_key(&displaced_response_id),
        "colder bindings should be pruned ahead of the oldest verified binding"
    );
}

#[test]
fn runtime_continuation_store_compaction_prunes_response_statuses_to_limit() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut response_profile_bindings = BTreeMap::new();
    let mut response_statuses = BTreeMap::new();
    for index in 0..(RUNTIME_CONTINUATION_RESPONSE_STATUS_LIMIT + 3) {
        let response_id = format!("resp-{index:06}");
        response_profile_bindings.insert(
            response_id.clone(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 1_000 + index as i64,
            },
        );
        response_statuses.insert(
            response_id,
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Warm,
                confidence: 1,
                last_touched_at: Some(now - 1_000 + index as i64),
                last_verified_at: None,
                last_verified_route: None,
                last_not_found_at: None,
                not_found_streak: 0,
                success_count: 0,
                failure_count: 0,
            },
        );
    }
    response_statuses.insert(
        "resp-hot".to_string(),
        RuntimeContinuationBindingStatus {
            state: RuntimeContinuationBindingLifecycle::Verified,
            confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
            last_touched_at: Some(now),
            last_verified_at: Some(now),
            last_verified_route: Some("responses".to_string()),
            last_not_found_at: None,
            not_found_streak: 0,
            success_count: 5,
            failure_count: 0,
        },
    );
    response_profile_bindings.insert(
        "resp-hot".to_string(),
        ResponseProfileBinding {
            profile_name: "main".to_string(),
            bound_at: now - 2_000,
        },
    );

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            statuses: RuntimeContinuationStatuses {
                response: response_statuses,
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted.statuses.response.len(),
        RUNTIME_CONTINUATION_RESPONSE_STATUS_LIMIT
    );
    assert!(
        compacted.statuses.response.contains_key("resp-hot"),
        "verified status should survive hard status cap"
    );
    assert!(
        !compacted.statuses.response.contains_key("resp-000000"),
        "coldest status should be dropped deterministically"
    );
}

#[test]
fn runtime_continuation_store_compaction_keeps_verified_hot_binding_over_newer_cold_binding() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([
                (
                    "resp-hot".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 3,
                    },
                ),
                (
                    "resp-cold-1".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 2,
                    },
                ),
                (
                    "resp-cold-2".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 1,
                    },
                ),
            ]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-hot".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
                        success_count: 3,
                        failure_count: 0,
                        not_found_streak: 0,
                        last_touched_at: Some(now),
                        last_not_found_at: None,
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    let mut retained = compacted
        .response_profile_bindings
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    retained.sort();

    assert_eq!(retained.len(), 3);
    assert!(retained.contains(&"resp-hot".to_string()));

    let mut pruned = RuntimeContinuationStore {
        response_profile_bindings: compacted.response_profile_bindings,
        statuses: compacted.statuses,
        ..RuntimeContinuationStore::default()
    };
    prune_runtime_continuation_response_bindings(
        &mut pruned.response_profile_bindings,
        &pruned.statuses.response,
        2,
    );
    assert!(pruned.response_profile_bindings.contains_key("resp-hot"));
    assert!(
        !pruned.response_profile_bindings.contains_key("resp-cold-1"),
        "the cold binding should be pruned before the older verified binding"
    );
}

#[test]
fn session_affinity_prefers_bound_profile_for_compact_requests() {
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
fn compact_final_failure_logs_overload_terminal_reason() {
    let temp_dir = TestDir::new();
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
    assert!(
        log.contains("compact_final_failure exit=candidate_exhausted reason=overload"),
        "compact overload terminal marker should identify overload exhaustion: {log}"
    );
    assert!(
        log.contains("last_failure=overload"),
        "compact overload terminal marker should preserve the last failure class: {log}"
    );
}

#[test]
fn compact_final_failure_logs_quota_terminal_reason() {
    let temp_dir = TestDir::new();
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
fn compact_final_failure_logs_local_selection_terminal_reason() {
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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

#[test]
fn runtime_sse_tap_reader_keeps_response_affinity_when_prelude_splits_event() {
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
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
    };
    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state,
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
    };
    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    let prelude =
            b"event: response.created\r\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second";
    let remainder =
            b"\"}}\r\n\r\nevent: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second\"}}\r\n\r\n";
    let mut reader = RuntimeSseTapReader::new(
        Cursor::new(prelude.to_vec()).chain(Cursor::new(remainder.to_vec())),
        shared.clone(),
        "second".to_string(),
        prelude,
        &[],
        None,
    );
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .expect("split SSE payload should be readable");

    let persisted = wait_for_state(&paths, |state| {
        state
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main")
    });
    assert!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main"),
        "stale previous_response_id should not stay pinned to the wrong owner"
    );
}

#[test]
fn section_headers_use_cli_width() {
    assert_eq!(
        text_width(&section_header_with_width("Quota Overview", CLI_WIDTH)),
        CLI_WIDTH
    );
}

#[test]
fn field_lines_do_not_exceed_cli_width() {
    let label_width = panel_label_width(
            &[(
                "Path".to_string(),
                "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered"
                    .to_string(),
            )],
            CLI_WIDTH,
        );
    let fields = format_field_lines_with_layout(
        "Path",
        "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered",
        CLI_WIDTH,
        label_width,
    );

    assert!(fields.iter().all(|line| text_width(line) <= CLI_WIDTH));
}

#[test]
fn section_headers_expand_to_requested_width() {
    assert_eq!(text_width(&section_header_with_width("Doctor", 72)), 72);
}

#[test]
fn field_lines_respect_requested_width() {
    let width = 72;
    let fields = vec![(
        "Profiles root".to_string(),
        "/tmp/some/really/long/path/that/needs/to/wrap/narrower".to_string(),
    )];
    let label_width = panel_label_width(&fields, width);
    let lines = format_field_lines_with_layout(
        "Profiles root",
        "/tmp/some/really/long/path/that/needs/to/wrap/narrower",
        width,
        label_width,
    );

    assert!(lines.iter().all(|line| text_width(line) <= width));
}

#[test]
fn shared_panel_builder_matches_render_panel_output() {
    let mut panel = PanelBuilder::new("Doctor");
    panel.push("Profile", "main");
    panel.extend([("Status".to_string(), "Ready".to_string())]);

    let expected = render_panel(
        "Doctor",
        &[
            ("Profile".to_string(), "main".to_string()),
            ("Status".to_string(), "Ready".to_string()),
        ],
    );

    assert_eq!(panel.render(), expected);
}

#[test]
fn runtime_proxy_injects_codex_backend_overrides() {
    let args = runtime_proxy_codex_args(
        "127.0.0.1:4455".parse().expect("socket addr"),
        &[OsString::from("exec"), OsString::from("hello")],
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(rendered[0], "-c");
    assert!(rendered[1].contains("chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""));
    assert_eq!(rendered[2], "-c");
    assert_eq!(
        rendered[3],
        format!(
            "openai_base_url=\"http://127.0.0.1:4455{}\"",
            RUNTIME_PROXY_OPENAI_MOUNT_PATH
        )
    );
    assert_eq!(&rendered[4..], ["exec", "hello"]);
}

#[test]
fn runtime_proxy_maps_openai_prefix_to_upstream_backend_api() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            &format!("{}/responses", RUNTIME_PROXY_OPENAI_MOUNT_PATH)
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_injects_custom_openai_mount_path_overrides() {
    let args = runtime_proxy_codex_args_with_mount_path(
        "127.0.0.1:4455".parse().expect("socket addr"),
        "/backend-api/prodex/v0.2.99",
        &[OsString::from("exec")],
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        rendered[3],
        "openai_base_url=\"http://127.0.0.1:4455/backend-api/prodex/v0.2.99\""
    );
}

#[test]
fn runtime_proxy_passthrough_args_preserve_user_args_without_proxy() {
    let user_args = vec![OsString::from("exec"), OsString::from("hello")];
    assert_eq!(
        runtime_proxy_codex_passthrough_args(None, &user_args),
        user_args
    );
}

#[test]
fn runtime_proxy_passthrough_args_follow_endpoint_mount_path() {
    let temp_dir = TestDir::new();
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:4455".parse().expect("listen addr"),
        openai_mount_path: "/backend-api/prodex/v0.2.99".to_string(),
        lease_dir: temp_dir.path.join("leases"),
        _lease: None,
    };

    let rendered = runtime_proxy_codex_passthrough_args(
        Some(&endpoint),
        &[OsString::from("exec"), OsString::from("hello")],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert_eq!(rendered[0], "-c");
    assert_eq!(
        rendered[3],
        "openai_base_url=\"http://127.0.0.1:4455/backend-api/prodex/v0.2.99\""
    );
    assert_eq!(&rendered[4..], ["exec", "hello"]);
}

#[test]
fn runtime_proxy_maps_legacy_versioned_openai_prefix_to_upstream_backend_api() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/v0.2.99/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_accepts_legacy_openai_prefix() {
    assert!(is_runtime_responses_path("/backend-api/codex/responses"));
    assert!(is_runtime_compact_path(
        "/backend-api/codex/responses/compact"
    ));
    assert!(is_runtime_realtime_call_path(
        "/backend-api/codex/realtime/calls"
    ));
    assert!(is_runtime_realtime_websocket_path(
        "/backend-api/codex/realtime?call_id=call-123"
    ));
    assert!(is_runtime_responses_path(
        "/backend-api/prodex/v0.2.99/responses"
    ));
    assert!(is_runtime_compact_path(
        "/backend-api/prodex/v0.2.99/responses/compact"
    ));
    assert!(is_runtime_realtime_call_path(
        "/backend-api/prodex/v0.2.99/realtime/calls"
    ));
    assert!(is_runtime_realtime_websocket_path(
        "/backend-api/prodex/v0.2.99/realtime?call_id=call-123"
    ));
    assert!(!is_runtime_realtime_websocket_path(
        "/backend-api/prodex/v0.2.99/realtime/calls"
    ));
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/codex/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/realtime/calls"
        ),
        "https://chatgpt.com/backend-api/codex/realtime/calls"
    );
}

#[test]
fn runtime_request_session_id_accepts_realtime_header() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/realtime/calls".to_string(),
        headers: vec![("x-session-id".to_string(), " sess-realtime ".to_string())],
        body: Vec::new(),
    };

    assert_eq!(
        runtime_request_session_id(&request),
        Some("sess-realtime".to_string())
    );
}

#[test]
fn runtime_request_explicit_session_id_ignores_body_session_id() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"session_id":"sess-body","input":[]}"#.to_vec(),
    };

    assert_eq!(runtime_request_explicit_session_id(&request), None);
    assert_eq!(
        runtime_request_session_id(&request),
        Some("sess-body".to_string())
    );
}

#[test]
fn runtime_request_explicit_session_id_ignores_turn_metadata_session_id() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: vec![(
            "x-codex-turn-metadata".to_string(),
            r#"{"source":"resume","session_id":"sess-metadata"}"#.to_string(),
        )],
        body: Vec::new(),
    };

    assert_eq!(runtime_request_explicit_session_id(&request), None);
    assert_eq!(
        runtime_request_session_id(&request),
        Some("sess-metadata".to_string())
    );
}

#[test]
fn runtime_proxy_broker_key_uses_stable_mount_path() {
    let current_key = runtime_broker_key("https://chatgpt.com/backend-api", false);

    let stable_key = {
        let mut hasher = DefaultHasher::new();
        "https://chatgpt.com/backend-api".hash(&mut hasher);
        false.hash(&mut hasher);
        "/backend-api/prodex".hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    let versioned_key = {
        let mut hasher = DefaultHasher::new();
        "https://chatgpt.com/backend-api".hash(&mut hasher);
        false.hash(&mut hasher);
        "/backend-api/prodex/v0.2.99".hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    assert_eq!(current_key, stable_key);
    assert_ne!(current_key, versioned_key);
}

#[test]
fn runtime_doctor_summary_counts_recent_runtime_markers() {
    let summary = summarize_runtime_log_tail(
            br#"[2026-03-20 12:00:00.000 +07:00] request=1 transport=http first_upstream_chunk bytes=128
[2026-03-20 12:00:00.010 +07:00] request=1 transport=http first_local_chunk profile=main bytes=128 elapsed_ms=10
[2026-03-20 12:00:00.015 +07:00] runtime_proxy_admission_wait_started transport=http path=/backend-api/codex/responses budget_ms=120 poll_ms=10 reason=responses
[2026-03-20 12:00:00.020 +07:00] profile_transport_backoff profile=main route=responses until=123 seconds=15 context=stream_read_error
[2026-03-20 12:00:00.030 +07:00] profile_health profile=main score=4 delta=4 reason=stream_read_error
[2026-03-20 12:00:00.035 +07:00] selection_skip_affinity route=responses affinity=session profile=main reason=quota_exhausted quota_source=persisted_snapshot
[2026-03-20 12:00:00.040 +07:00] runtime_proxy_active_limit_reached transport=http path=/backend-api/codex/responses active=12 limit=12
[2026-03-20 12:00:00.050 +07:00] runtime_proxy_lane_limit_reached transport=http path=/backend-api/codex/responses lane=responses active=9 limit=9
[2026-03-20 12:00:00.055 +07:00] runtime_proxy_admission_recovered transport=http path=/backend-api/codex/responses waited_ms=20
[2026-03-20 12:00:00.060 +07:00] profile_inflight_saturated profile=main hard_limit=8
[2026-03-20 12:00:00.070 +07:00] runtime_proxy_queue_overloaded transport=http path=/backend-api/codex/responses reason=long_lived_queue_full
[2026-03-20 12:00:00.072 +07:00] runtime_proxy_queue_wait_started transport=http path=/backend-api/codex/responses budget_ms=120 poll_ms=10 reason=long_lived_queue_full
[2026-03-20 12:00:00.074 +07:00] runtime_proxy_queue_recovered transport=http path=/backend-api/codex/responses waited_ms=18
[2026-03-20 12:00:00.075 +07:00] state_save_queued revision=2 reason=session_id:main backlog=3 ready_in_ms=5
[2026-03-20 12:00:00.078 +07:00] continuation_journal_save_queued reason=session_id:main backlog=2
[2026-03-20 12:00:00.080 +07:00] profile_probe_refresh_queued profile=second reason=queued backlog=4
[2026-03-20 12:00:00.085 +07:00] state_save_skipped revision=2 reason=session_id:main lag_ms=7
[2026-03-20 12:00:00.086 +07:00] continuation_journal_save_ok saved_at=123 reason=session_id:main lag_ms=11
[2026-03-20 12:00:00.090 +07:00] profile_probe_refresh_start profile=second
[2026-03-20 12:00:00.095 +07:00] profile_probe_refresh_ok profile=second lag_ms=13
[2026-03-20 12:00:00.100 +07:00] profile_probe_refresh_error profile=third lag_ms=8 error=timeout
[2026-03-20 12:00:00.094 +07:00] profile_circuit_open profile=main route=responses until=123 reason=stream_read_error score=4
[2026-03-20 12:00:00.095 +07:00] profile_circuit_half_open_probe profile=main route=responses until=128 health=3
[2026-03-20 12:00:00.096 +07:00] websocket_reuse_watchdog profile=main event=read_error elapsed_ms=33 committed=true
[2026-03-20 12:00:00.097 +07:00] request=2 transport=http route=responses previous_response_not_found profile=second response_id=resp-second retry_index=0
[2026-03-20 12:00:00.098 +07:00] request=3 transport=websocket route=websocket previous_response_not_found profile=second response_id=resp-second retry_index=0
[2026-03-20 12:00:00.099 +07:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_retried_owner profile=second previous_response_id=resp-second delay_ms=20 reason=previous_response_not_found_locked_affinity via=-
[2026-03-20 12:00:00.100 +07:00] request=3 transport=websocket route=websocket websocket_session=sess-1 stale_continuation reason=previous_response_not_found_locked_affinity profile=second
[2026-03-20 12:00:00.101 +07:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_dead_upstream_confirmed profile=second previous_response_id=resp-second reason=previous_response_not_found_locked_affinity via=- event=-
[2026-03-20 12:00:00.102 +07:00] local_writer_error request=1 transport=http profile=main stage=chunk_flush chunks=1 bytes=128 elapsed_ms=20 error=broken_pipe
[2026-03-20 12:00:00.105 +07:00] runtime_proxy_startup_audit missing_managed_dirs=1 stale_response_bindings=2 stale_session_bindings=1 active_profile_missing_dir=false
"#,
        );

    assert!((30..=31).contains(&summary.line_count));
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_overloaded"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_active_limit_reached"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_lane_limit_reached"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_admission_wait_started"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_admission_recovered"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_inflight_saturated"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_transport_backoff"),
        1
    );
    assert_eq!(runtime_doctor_marker_count(&summary, "profile_health"), 1);
    assert_eq!(
        runtime_doctor_marker_count(&summary, "selection_skip_affinity"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_wait_started"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_recovered"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_upstream_chunk"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_local_chunk"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_start"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_ok"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "state_save_queued"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "continuation_journal_save_queued"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "continuation_journal_save_ok"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "state_save_skipped"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_error"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_open"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_half_open_probe"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_reuse_watchdog"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "previous_response_not_found"),
        2
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "chain_retried_owner"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "stale_continuation"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "chain_dead_upstream_confirmed"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "local_writer_error"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_startup_audit"),
        1
    );
    assert_eq!(
        runtime_doctor_top_facet(&summary, "quota_source").as_deref(),
        Some("persisted_snapshot (1)")
    );
    assert_eq!(summary.state_save_queue_backlog, Some(3));
    assert_eq!(summary.state_save_lag_ms, Some(7));
    assert_eq!(summary.continuation_journal_save_backlog, Some(2));
    assert_eq!(summary.continuation_journal_save_lag_ms, Some(11));
    assert_eq!(summary.profile_probe_refresh_backlog, Some(4));
    assert_eq!(summary.profile_probe_refresh_lag_ms, Some(13));
    assert_eq!(
        summary.failure_class_counts,
        BTreeMap::from([
            ("admission".to_string(), 6),
            ("continuation".to_string(), 5),
            ("persistence".to_string(), 1),
            ("quota".to_string(), 5),
            ("transport".to_string(), 1),
        ])
    );
    assert_eq!(
        summary.previous_response_not_found_by_route,
        BTreeMap::from([("responses".to_string(), 1), ("websocket".to_string(), 1)])
    );
    assert_eq!(
        summary.previous_response_not_found_by_transport,
        BTreeMap::from([("http".to_string(), 1), ("websocket".to_string(), 1)])
    );
    assert_eq!(
        summary
            .previous_response_not_found_by_route
            .get("websocket"),
        Some(&1)
    );
    assert_eq!(
        summary.latest_stale_continuation_reason.as_deref(),
        Some("previous_response_not_found_locked_affinity")
    );
    assert_eq!(
        summary.stale_continuation_by_reason,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
    );
    assert!(
        summary
            .latest_chain_event
            .as_deref()
            .is_some_and(|event| event.contains("chain_dead_upstream_confirmed")
                && event.contains("previous_response_id=resp-second"))
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("state_save_queued")
            .and_then(|fields| fields.get("backlog"))
            .map(String::as_str),
        Some("3")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("continuation_journal_save_ok")
            .and_then(|fields| fields.get("lag_ms"))
            .map(String::as_str),
        Some("11")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("profile_probe_refresh_error")
            .and_then(|fields| fields.get("lag_ms"))
            .map(String::as_str),
        Some("8")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("selection_skip_affinity")
            .and_then(|fields| fields.get("affinity"))
            .map(String::as_str),
        Some("session")
    );
    assert!(
        summary
            .last_marker_line
            .as_deref()
            .is_some_and(|line| line.contains("runtime_proxy_startup_audit"))
    );
}

#[test]
fn attempt_runtime_responses_request_skips_exhausted_profile_before_send() {
    let temp_dir = TestDir::new();
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
        paths: paths.clone(),
        state,
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
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 81,
                five_hour_reset_at: Local::now().timestamp() + 3600,
                weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                weekly_remaining_percent: 0,
                weekly_reset_at: Local::now().timestamp() + 300,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"input":[]}"#.to_vec(),
    };

    match attempt_runtime_responses_request(1, &request, &shared, "main", None)
        .expect("responses attempt should succeed")
    {
        RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name,
            reason,
        } => {
            assert_eq!(profile_name, "main");
            assert_eq!(reason, "quota_exhausted_before_send");
        }
        _ => panic!("expected exhausted pre-send responses skip"),
    }
}

#[test]
fn attempt_runtime_standard_request_skips_exhausted_profile_before_send() {
    let temp_dir = TestDir::new();
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
        paths: paths.clone(),
        state,
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
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: Local::now().timestamp() + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 90,
                weekly_reset_at: Local::now().timestamp() + 86_400,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![("session_id".to_string(), "sess-123".to_string())],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    match attempt_runtime_standard_request(1, &request, &shared, "main", false)
        .expect("standard attempt should succeed")
    {
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            assert_eq!(profile_name, "main");
        }
        _ => panic!("expected exhausted pre-send compact skip"),
    }
}
