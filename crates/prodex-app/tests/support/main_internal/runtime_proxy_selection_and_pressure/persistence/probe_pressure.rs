use super::*;

#[test]
fn cold_start_candidate_probe_is_queued_without_blocking_selection() {
    let temp_dir = TestDir::isolated();
    let (listener, base_url) = unresponsive_loopback_backend_listener();
    let shared = runtime_shared_for_cold_start_probe_selection(&temp_dir, base_url);

    let started_at = Instant::now();
    let candidate = next_runtime_response_candidate(&shared, &BTreeSet::new())
        .expect("candidate lookup should succeed");
    assert_eq!(candidate, Some("second".to_string()));
    assert!(
        started_at.elapsed() < ci_timing_upper_bound_ms(80, 250),
        "candidate selection must not wait on the quota network probe"
    );

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(!runtime.profile_probe_cache.contains_key("second"));
    assert!(!runtime.profile_usage_snapshots.contains_key("second"));
    drop(runtime);

    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(
        log.contains("profile_probe_refresh_queued profile=second reason=queued"),
        "cold-start probing should move to the background queue: {log}"
    );

    drop(listener);
    wait_for_runtime_background_queues_idle();
}

#[test]
fn cold_start_candidate_is_allowed_without_cached_auth() {
    let temp_dir = TestDir::isolated();
    let (listener, base_url) = unresponsive_loopback_backend_listener();
    let shared = runtime_shared_for_cold_start_probe_selection(&temp_dir, base_url);
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_probe_cache
        .clear();

    let candidate = next_runtime_response_candidate(&shared, &BTreeSet::new())
        .expect("candidate lookup should succeed");
    assert_eq!(candidate, Some("second".to_string()));

    drop(listener);
    wait_for_runtime_background_queues_idle();
}

#[test]
fn sync_probe_pressure_mode_is_route_aware_for_background_queue_pressure() {
    assert!(!runtime_proxy_sync_probe_pressure_mode_for_route(
        RuntimeRouteKind::Responses,
        false,
        true,
    ));
    assert!(!runtime_proxy_sync_probe_pressure_mode_for_route(
        RuntimeRouteKind::Websocket,
        false,
        true,
    ));
    assert!(runtime_proxy_sync_probe_pressure_mode_for_route(
        RuntimeRouteKind::Compact,
        false,
        true,
    ));
    assert!(runtime_proxy_sync_probe_pressure_mode_for_route(
        RuntimeRouteKind::Standard,
        false,
        true,
    ));
    assert!(runtime_proxy_sync_probe_pressure_mode_for_route(
        RuntimeRouteKind::Responses,
        true,
        false,
    ));
}
