use super::*;

#[test]
fn guarded_release_reports_underflow_without_wrapping() {
    let active = AtomicUsize::new(0);
    let lane = AtomicUsize::new(2);
    let lane_releases = AtomicU64::new(0);
    let active_underflows = AtomicU64::new(0);
    let lane_underflows = AtomicU64::new(0);
    let wait = (Mutex::new(()), Condvar::new());

    let snapshot = release_runtime_proxy_active_request_guard(
        &active,
        &lane,
        &lane_releases,
        &active_underflows,
        &lane_underflows,
        &wait,
    );

    assert_eq!(
        snapshot,
        RuntimeProxyGuardReleaseSnapshot {
            active_remaining: 0,
            lane_remaining: 1,
            active_underflow: true,
            lane_underflow: false,
        }
    );
    assert_eq!(active_underflows.load(Ordering::Relaxed), 1);
    assert_eq!(lane_underflows.load(Ordering::Relaxed), 0);
    assert_eq!(lane_releases.load(Ordering::Relaxed), 1);
}

#[test]
fn pressure_mode_only_applies_background_pressure_to_side_lanes() {
    assert!(!runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Responses,
        false,
        true,
    ));
    assert!(runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Compact,
        false,
        true,
    ));
    assert!(runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Websocket,
        true,
        false,
    ));
}
