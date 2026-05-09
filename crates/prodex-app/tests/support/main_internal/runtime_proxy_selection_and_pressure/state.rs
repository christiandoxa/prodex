use super::*;

#[path = "state/app_state.rs"]
mod app_state;
#[path = "state/continuations.rs"]
mod continuations;
#[path = "state/housekeeping.rs"]
mod housekeeping;
#[path = "state/snapshots.rs"]
mod snapshots;

#[test]
fn merge_runtime_usage_snapshots_keeps_newer_entries() {
    let now = Local::now().timestamp();
    let existing = BTreeMap::from([(
        "main".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now - 20,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 70,
            five_hour_reset_at: now + 100,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 80,
            weekly_reset_at: now + 200,
        },
    )]);
    let incoming = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now - 10,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Critical,
                weekly_remaining_percent: 5,
                weekly_reset_at: now + 400,
            },
        ),
        (
            "stale".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now - 10,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: now + 400,
            },
        ),
    ]);
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/main"),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        },
    )]);

    let merged = merge_runtime_usage_snapshots(&existing, &incoming, &profiles);
    assert_eq!(merged.len(), 1);
    assert_eq!(
        merged
            .get("main")
            .expect("main snapshot should exist")
            .checked_at,
        now - 10
    );
    assert_eq!(
        merged
            .get("main")
            .expect("main snapshot should exist")
            .five_hour_status,
        RuntimeQuotaWindowStatus::Exhausted
    );
}

#[test]
fn runtime_profile_selection_jitter_is_deterministic_for_same_sequence() {
    let temp_dir = TestDir::isolated();
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
        request_sequence: Arc::new(AtomicU64::new(42)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    let first = runtime_profile_selection_jitter(&shared, "main", RuntimeRouteKind::Responses);
    let second = runtime_profile_selection_jitter(&shared, "main", RuntimeRouteKind::Responses);
    assert_eq!(first, second);
}

#[test]
fn runtime_profile_transport_health_penalty_weights_connect_failures_higher() {
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::ReadTimeout),
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::ConnectTimeout),
        RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::BrokenPipe),
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    );
}
