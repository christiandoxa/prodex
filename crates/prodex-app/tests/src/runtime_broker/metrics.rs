use super::*;
use crate::runtime_broker_test_secret;
use std::time::Duration;

fn temp_log_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "prodex-runtime-broker-metrics-{name}-{}-{}.log",
        std::process::id(),
        Local::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}

fn clear_runtime_broker_continuity_failure_reason_cache() {
    prodex_runtime_broker_log::clear_runtime_broker_continuity_failure_reason_cache_for_test();
}

fn test_runtime_broker_shared(log_path: PathBuf) -> RuntimeRotationProxyShared {
    let root = std::env::temp_dir().join(format!(
        "prodex-runtime-broker-metrics-shared-{}",
        Local::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let runtime = RuntimeRotationState {
        paths: AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
            root,
        },
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/prodex-runtime-broker-metrics-main"),
                    managed: true,
                    email: None,
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
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };

    RuntimeRotationProxyShared {
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        auto_redeem_enabled: false,
        upstream_no_proxy: false,
        async_client: reqwest::Client::builder()
            .build()
            .expect("test async client"),
        async_runtime: Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("test async runtime"),
        ),
        runtime: Arc::new(Mutex::new(runtime)),
        log_path,
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(runtime_proxy_lane_limits(8, 1, 1)),
    }
}

#[test]
fn runtime_broker_metrics_snapshot_merges_live_and_parsed_continuity_failure_counters() {
    let _guard = acquire_test_runtime_lock();
    clear_runtime_broker_continuity_failure_reason_cache();
    clear_all_runtime_proxy_continuity_failure_reason_metrics();
    let log_path = temp_log_path("live-counters");
    clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
    fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=8 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

    let shared = test_runtime_broker_shared(log_path.clone());
    shared
        .lane_admission
        .admission_wait_metrics
        .record_wait(Duration::from_nanos(21));
    shared
        .lane_admission
        .long_lived_queue_wait_metrics
        .record_wait(Duration::from_nanos(34));
    runtime_proxy_record_continuity_failure_reason(
        &shared,
        "chain_dead_upstream_confirmed",
        "previous_response_not_found_locked_affinity",
    );
    runtime_proxy_record_continuity_failure_reason(
        &shared,
        "stale_continuation",
        "websocket_reuse_watchdog_locked_affinity",
    );

    let metrics = runtime_broker_metrics_snapshot(
        &shared,
        &RuntimeBrokerMetadata {
            broker_key: "broker".to_string(),
            listen_addr: "127.0.0.1:12345".to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_id: "instance".to_string(),
            admin_token: runtime_broker_test_secret("secret"),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
        },
    )
    .expect("broker metrics snapshot should succeed");

    assert_eq!(
        metrics
            .continuity_failure_reasons
            .chain_dead_upstream_confirmed,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
    );
    assert_eq!(metrics.admission_wait.wait_total_ns, 21);
    assert_eq!(metrics.admission_wait.wait_count, 1);
    assert_eq!(metrics.long_lived_queue_wait.wait_total_ns, 34);
    assert_eq!(metrics.long_lived_queue_wait.wait_count, 1);
    #[cfg(feature = "allocation-bench-support")]
    assert!(metrics.allocation.is_some());
    #[cfg(not(feature = "allocation-bench-support"))]
    assert_eq!(metrics.allocation, None);
    assert_eq!(
        metrics.continuity_failure_reasons.stale_continuation,
        BTreeMap::from([
            ("previous_response_not_found".to_string(), 1),
            ("websocket_reuse_watchdog_locked_affinity".to_string(), 1),
        ])
    );

    clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
    fs::remove_file(&log_path).expect("test log should clean up");
}

#[test]
fn runtime_broker_metrics_snapshot_counts_pending_live_reasons_once() {
    let _guard = acquire_test_runtime_lock();
    clear_runtime_broker_continuity_failure_reason_cache();
    clear_all_runtime_proxy_continuity_failure_reason_metrics();
    let log_path = temp_log_path("live-pending-same-reason");
    clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
    fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=8 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

    let shared = test_runtime_broker_shared(log_path.clone());
    runtime_proxy_record_continuity_failure_reason(
        &shared,
        "stale_continuation",
        "previous_response_not_found",
    );

    let metrics = runtime_broker_metrics_snapshot(
        &shared,
        &RuntimeBrokerMetadata {
            broker_key: "broker".to_string(),
            listen_addr: "127.0.0.1:12345".to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_id: "instance".to_string(),
            admin_token: runtime_broker_test_secret("secret"),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
        },
    )
    .expect("broker metrics snapshot should succeed");

    assert_eq!(
        metrics.continuity_failure_reasons.stale_continuation,
        BTreeMap::from([("previous_response_not_found".to_string(), 2)])
    );

    clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
    fs::remove_file(&log_path).expect("test log should clean up");
}

#[test]
fn runtime_proxy_continuity_failure_reason_metrics_snapshot_prunes_missing_log_path() {
    let _guard = acquire_test_runtime_lock();
    clear_all_runtime_proxy_continuity_failure_reason_metrics();
    let log_path = temp_log_path("live-prune-missing");
    fs::write(&log_path, "").expect("test log should exist");
    let shared = test_runtime_broker_shared(log_path.clone());
    runtime_proxy_record_continuity_failure_reason(
        &shared,
        "stale_continuation",
        "previous_response_not_found",
    );
    assert!(
        runtime_proxy_continuity_failure_reason_metrics_snapshot(&log_path).is_some(),
        "live entry should exist before log removal"
    );

    fs::remove_file(&log_path).expect("test log should clean up");

    assert!(
        runtime_proxy_continuity_failure_reason_metrics_snapshot(&log_path).is_none(),
        "missing log path should evict the live entry"
    );
}

#[test]
fn runtime_proxy_continuity_failure_reason_metrics_store_evicts_old_log_paths() {
    let _guard = acquire_test_runtime_lock();
    clear_all_runtime_proxy_continuity_failure_reason_metrics();
    let mut log_paths = Vec::new();

    for index in 0..20 {
        let log_path = temp_log_path(&format!("live-store-{index}"));
        fs::write(&log_path, "").expect("test log should exist");
        let shared = test_runtime_broker_shared(log_path.clone());
        runtime_proxy_record_continuity_failure_reason(
            &shared,
            "stale_continuation",
            "previous_response_not_found",
        );
        log_paths.push(log_path);
    }

    assert_eq!(
        runtime_proxy_continuity_failure_reason_metrics_store_entry_count(),
        16
    );
    assert!(
        runtime_proxy_continuity_failure_reason_metrics_snapshot(&log_paths[0]).is_none(),
        "oldest path should be evicted once the store exceeds its cap"
    );
    assert!(
        runtime_proxy_continuity_failure_reason_metrics_snapshot(
            log_paths.last().expect("live store path"),
        )
        .is_some(),
        "most recent path should stay resident"
    );

    for log_path in log_paths {
        clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
        let _ = fs::remove_file(&log_path);
    }
    clear_all_runtime_proxy_continuity_failure_reason_metrics();
}
