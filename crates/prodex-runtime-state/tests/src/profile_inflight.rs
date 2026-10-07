use super::*;

#[test]
fn runtime_proxy_profile_inflight_hard_limit_is_atomic_and_hard_affinity_bypasses_it() {
    let admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: 1,
        compact: 1,
        websocket: 1,
        standard: 1,
    });

    assert_eq!(
        admission.try_acquire_profile_inflight("main", 1, Some(2)),
        Some(1)
    );
    assert_eq!(
        admission.try_acquire_profile_inflight("main", 1, Some(2)),
        Some(2)
    );
    assert_eq!(
        admission.try_acquire_profile_inflight("main", 1, Some(2)),
        None
    );
    assert_eq!(admission.profile_inflight_count("main"), 2);
    assert_eq!(
        admission.try_acquire_profile_inflight("main", 1, None),
        Some(3)
    );
}
