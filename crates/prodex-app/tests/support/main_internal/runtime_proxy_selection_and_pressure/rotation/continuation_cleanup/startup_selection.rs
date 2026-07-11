use super::*;

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
