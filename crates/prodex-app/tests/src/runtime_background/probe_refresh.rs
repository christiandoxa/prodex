use super::*;

fn test_runtime_probe_refresh_queue() -> RuntimeProbeRefreshQueue {
    RuntimeProbeRefreshQueue {
        pending: Mutex::new(BTreeMap::new()),
        wake: Condvar::new(),
        active: Arc::new(AtomicUsize::new(0)),
        wait: Arc::new((Mutex::new(()), Condvar::new())),
        revision: Arc::new(AtomicU64::new(0)),
    }
}

fn test_runtime_probe_refresh_shared() -> RuntimeRotationProxyShared {
    let root =
        std::env::temp_dir().join(format!("prodex-probe-refresh-unit-{}", std::process::id()));
    RuntimeRotationProxyShared {
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: root.join("prodex"),
                state_file: root.join("prodex/state.json"),
                managed_profiles_root: root.join("prodex/profiles"),
                shared_codex_root: root.join("shared"),
                legacy_shared_codex_root: root.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
        log_path: root.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 1,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 1,
            compact: 1,
            websocket: 1,
            standard: 1,
        }),
    }
}

fn test_runtime_probe_refresh_job(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> RuntimeProbeRefreshJob {
    RuntimeProbeRefreshJob {
        shared: shared.clone(),
        profile_name: profile_name.to_string(),
        codex_home: PathBuf::from(format!("/tmp/{profile_name}")),
        upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
        queued_at: Instant::now(),
    }
}

#[test]
fn runtime_probe_refresh_error_text_redacts_secret_like_material() {
    let message =
        runtime_probe_refresh_error_text("failed: Authorization: Bearer probe-refresh-token");

    assert!(message.contains("Authorization: Bearer <redacted>"));
    assert!(!message.contains("probe-refresh-token"));
}

#[test]
fn runtime_probe_refresh_state_update_error_redacts_secret_like_chain() {
    let err = anyhow::anyhow!("failed: Authorization: Bearer probe-state-token")
        .context("probe state update failed");

    let message = runtime_probe_refresh_state_update_error(&err);

    assert!(message.starts_with("state_update:"));
    assert!(message.contains("probe state update failed"));
    assert!(message.contains("Authorization: Bearer <redacted>"));
    assert!(!message.contains("probe-state-token"));
}

#[test]
fn runtime_probe_refresh_take_next_job_leaves_remaining_backlog_for_other_workers() {
    let shared = test_runtime_probe_refresh_shared();
    let queue = test_runtime_probe_refresh_queue();
    {
        let mut pending = queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending.insert(
            (PathBuf::from("/tmp/state.json"), "alpha".to_string()),
            test_runtime_probe_refresh_job(&shared, "alpha"),
        );
        pending.insert(
            (PathBuf::from("/tmp/state.json"), "beta".to_string()),
            test_runtime_probe_refresh_job(&shared, "beta"),
        );
    }

    let first = runtime_probe_refresh_take_next_job(&queue);
    assert_eq!(first.profile_name, "alpha");
    assert_eq!(
        queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len(),
        1,
        "one dequeue should leave the rest of the backlog available to other workers"
    );

    let second = runtime_probe_refresh_take_next_job(&queue);
    assert_eq!(second.profile_name, "beta");
    assert!(
        queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty(),
        "second worker should be able to claim the remaining queued job"
    );
}
