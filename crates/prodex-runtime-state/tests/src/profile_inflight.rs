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

#[test]
fn released_capacity_never_exposes_an_old_release_revision() {
    let admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: 1,
        compact: 1,
        websocket: 1,
        standard: 1,
    });
    let observer = admission.clone();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let observed_barrier = barrier.clone();
    const ROUNDS: u64 = 10_000;
    let observer = std::thread::spawn(move || {
        let mut violations = Vec::new();
        for expected_revision in 1..=ROUNDS {
            observed_barrier.wait();
            while observer.profile_inflight_count("main") != 0 {
                std::thread::yield_now();
            }
            let revision = observer.inflight_release_revision();
            if revision != expected_revision {
                violations.push((expected_revision, revision));
            }
            observed_barrier.wait();
        }
        violations
    });
    for _ in 0..ROUNDS {
        admission.acquire_profile_inflight("main", 1);
        barrier.wait();
        admission.release_profile_inflight("main", 1);
        barrier.wait();
    }
    assert!(
        observer.join().unwrap().is_empty(),
        "capacity and its release generation must be published as one state transition"
    );
    assert_eq!(admission.inflight_release_revision(), ROUNDS);
}
