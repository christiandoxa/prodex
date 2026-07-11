use super::*;

fn policy() -> RuntimeProxyContinuationPolicy {
    RuntimeProxyContinuationPolicy {
        touch_persist_interval_seconds: 1,
        suspect_grace_seconds: 5,
        suspect_not_found_streak_limit: 2,
        confidence_max: 8,
        verified_confidence_bonus: 2,
        touch_confidence_bonus: 1,
        suspect_confidence_penalty: 1,
    }
}

#[test]
fn suspect_status_recovers_after_grace_touch() {
    let mut status = RuntimeProxyContinuationBindingStatus {
        state: RuntimeProxyContinuationBindingLifecycle::Suspect,
        confidence: 1,
        last_not_found_at: Some(10),
        not_found_streak: 1,
        failure_count: 1,
        ..RuntimeProxyContinuationBindingStatus::default()
    };

    assert!(runtime_proxy_continuation_status_touches(
        &mut status,
        15,
        policy(),
    ));

    assert_eq!(status.state, RuntimeProxyContinuationBindingLifecycle::Warm);
    assert_eq!(status.not_found_streak, 0);
    assert_eq!(status.last_not_found_at, None);
    assert_eq!(status.confidence, 2);
}

#[test]
fn repeated_suspect_marks_dead_at_policy_limit() {
    let mut status = RuntimeProxyContinuationBindingStatus {
        confidence: 2,
        ..RuntimeProxyContinuationBindingStatus::default()
    };

    assert!(runtime_proxy_mark_continuation_status_suspect(
        &mut status,
        10,
        policy(),
    ));
    assert_eq!(
        status.state,
        RuntimeProxyContinuationBindingLifecycle::Suspect
    );
    assert!(runtime_proxy_mark_continuation_status_suspect(
        &mut status,
        11,
        policy(),
    ));
    assert_eq!(status.state, RuntimeProxyContinuationBindingLifecycle::Dead);
}

#[test]
fn verified_refresh_checks_route_and_touch_interval() {
    let status = RuntimeProxyContinuationBindingStatus {
        state: RuntimeProxyContinuationBindingLifecycle::Verified,
        last_verified_at: Some(10),
        last_verified_route: Some("responses".to_string()),
        ..RuntimeProxyContinuationBindingStatus::default()
    };

    assert!(!runtime_proxy_continuation_status_should_refresh_verified(
        Some(&status),
        11,
        Some("responses"),
        policy(),
    ));
    assert!(runtime_proxy_continuation_status_should_refresh_verified(
        Some(&status),
        12,
        Some("responses"),
        policy(),
    ));
    assert!(runtime_proxy_continuation_status_should_refresh_verified(
        Some(&status),
        11,
        Some("compact"),
        policy(),
    ));
}

#[test]
fn dead_status_shadowed_by_newer_binding() {
    let status = RuntimeProxyContinuationBindingStatus {
        state: RuntimeProxyContinuationBindingLifecycle::Dead,
        last_not_found_at: Some(10),
        ..RuntimeProxyContinuationBindingStatus::default()
    };

    assert!(runtime_proxy_continuation_dead_status_shadowed_by_binding_bound_at(11, &status));
    assert!(!runtime_proxy_continuation_dead_status_shadowed_by_binding_bound_at(10, &status));
}
