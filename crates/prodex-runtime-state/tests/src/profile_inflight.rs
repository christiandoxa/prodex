use super::*;
use crate::{
    runtime_profile_route_bad_pairing_key, runtime_profile_route_circuit_health_key,
    runtime_profile_route_circuit_key, runtime_profile_route_circuit_profile_name,
    runtime_profile_route_circuit_reopen_key, runtime_profile_route_health_key,
    runtime_profile_route_key_parts, runtime_profile_route_performance_key,
    runtime_profile_route_success_streak_key, runtime_profile_transport_backoff_key,
    runtime_route_coupled_kinds, runtime_route_kind_from_label, runtime_route_kind_label,
};

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

#[test]
fn route_policy_preserves_labels_coupling_and_key_shapes() {
    assert_eq!(
        [
            runtime_route_kind_label(RuntimeRouteKind::Responses),
            runtime_route_kind_label(RuntimeRouteKind::Compact),
            runtime_route_kind_label(RuntimeRouteKind::Websocket),
            runtime_route_kind_label(RuntimeRouteKind::Standard),
        ],
        ["responses", "compact", "websocket", "standard"]
    );
    assert_eq!(
        runtime_route_kind_from_label("websocket"),
        Some(RuntimeRouteKind::Websocket)
    );
    assert_eq!(runtime_route_kind_from_label("not-a-route"), None);
    assert_eq!(
        runtime_route_coupled_kinds(RuntimeRouteKind::Responses),
        &[RuntimeRouteKind::Websocket]
    );
    assert_eq!(
        runtime_route_coupled_kinds(RuntimeRouteKind::Standard),
        &[RuntimeRouteKind::Compact]
    );

    assert_eq!(
        runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
        "__route_health__:responses:alpha"
    );
    assert_eq!(
        runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Compact),
        "__route_bad_pairing__:compact:alpha"
    );
    assert_eq!(
        runtime_profile_route_success_streak_key("alpha", RuntimeRouteKind::Websocket),
        "__route_success__:websocket:alpha"
    );
    assert_eq!(
        runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Standard),
        "__route_performance__:standard:alpha"
    );
    assert_eq!(
        runtime_profile_route_circuit_key("alpha", RuntimeRouteKind::Responses),
        "__route_circuit__:responses:alpha"
    );
    assert_eq!(
        runtime_profile_route_circuit_reopen_key("alpha", RuntimeRouteKind::Responses),
        "__route_circuit_reopen__:responses:alpha"
    );
    assert_eq!(
        runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Compact),
        "__route_transport_backoff__:compact:alpha"
    );
}

#[test]
fn route_policy_preserves_parsing_and_malformed_behavior() {
    let key = "__route_transport_backoff__:responses:profile:with:colons";
    assert_eq!(
        runtime_profile_route_key_parts(key, "__route_transport_backoff__:"),
        Some(("responses", "profile:with:colons"))
    );
    assert_eq!(
        runtime_profile_route_key_parts(
            "__route_transport_backoff__",
            "__route_transport_backoff__:",
        ),
        None
    );
    assert_eq!(
        runtime_profile_route_key_parts("other:responses:alpha", "__route_transport_backoff__:"),
        None
    );
    assert_eq!(runtime_profile_route_circuit_profile_name(key), "colons");
    assert_eq!(
        runtime_profile_route_circuit_health_key("__route_circuit__:responses:alpha"),
        "__route_health__:responses:alpha"
    );
    assert_eq!(
        runtime_profile_route_circuit_health_key("other:key"),
        "other:key"
    );
    assert_eq!(
        runtime_profile_route_circuit_health_key("prefix__route_circuit__middle__route_circuit__",),
        "prefix__route_health__middle__route_circuit__"
    );
}
