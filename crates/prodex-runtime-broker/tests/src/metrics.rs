use std::collections::BTreeMap;
use std::sync::Arc;

use super::*;

#[test]
fn metrics_snapshot_input_builds_broker_metrics_dto() {
    let metadata = RuntimeBrokerMetadata {
        broker_key: "key".to_string(),
        listen_addr: "127.0.0.1:4567".to_string(),
        started_at: 100,
        current_profile: "main".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        instance_id: "instance".to_string(),
        admin_token: Arc::new(RuntimeBrokerSecret::new("admin").unwrap()),
        prodex_version: Some("0.7.0".to_string()),
        executable_path: Some("/tmp/prodex".to_string()),
        executable_sha256: Some("abc123".to_string()),
    };
    let lane = RuntimeBrokerLaneMetrics {
        active: 1,
        limit: 4,
        admissions_total: 10,
        releases_total: 9,
        global_limit_rejections_total: 1,
        lane_limit_rejections_total: 2,
        release_underflows_total: 0,
    };
    let traffic = RuntimeBrokerTrafficMetrics {
        responses: lane.clone(),
        compact: lane.clone(),
        websocket: lane.clone(),
        standard: lane,
    };
    let profile_inflight = BTreeMap::from([("main".to_string(), 2)]);
    let profile_health = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 99,
            },
        ),
        (
            "__route_health__:responses:main".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 99,
            },
        ),
        (
            "__previous_response_not_found__:responses:resp-1".to_string(),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 99,
            },
        ),
    ]);
    let continuation_statuses = RuntimeContinuationStatuses {
        response: BTreeMap::from([(
            "resp-1".to_string(),
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Verified,
                last_verified_at: Some(80),
                failure_count: 2,
                ..RuntimeContinuationBindingStatus::default()
            },
        )]),
        ..RuntimeContinuationStatuses::default()
    };

    let metrics = runtime_broker_metrics_from_snapshot_input(RuntimeBrokerMetricsSnapshotInput {
        metadata: &metadata,
        pid: 42,
        active_requests: 3,
        persistence_owner: true,
        active_request_limit: 8,
        local_overload_backoff_remaining_seconds: 5,
        runtime_state_lock_wait: RuntimeStateLockWaitMetrics {
            wait_total_ns: 10,
            wait_count: 1,
            wait_max_ns: 10,
        },
        admission_wait: RuntimeWaitDurationMetrics {
            wait_total_ns: 20,
            wait_count: 2,
            wait_max_ns: 15,
        },
        long_lived_queue_wait: RuntimeWaitDurationMetrics {
            wait_total_ns: 30,
            wait_count: 3,
            wait_max_ns: 18,
        },
        traffic,
        profile_inflight: &profile_inflight,
        profile_retry_backoff_until: &BTreeMap::from([("main".to_string(), 101)]),
        profile_transport_backoff_until: &BTreeMap::from([("main".to_string(), 101)]),
        profile_route_circuit_open_until: &BTreeMap::from([("main".to_string(), 99)]),
        profile_health: &profile_health,
        continuation_statuses: &continuation_statuses,
        continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics {
            stale_continuation: BTreeMap::from([("watchdog".to_string(), 1)]),
            ..RuntimeBrokerContinuityFailureReasonMetrics::default()
        },
        now: 100,
        health_decay_seconds: 10,
        stale_verified_seconds: 10,
        previous_response_negative_cache_seconds: 10,
    })
    .with_guard_counters(RuntimeBrokerMetricsGuardCounters {
        active_request_release_underflows_total: 1,
        profile_inflight_admissions_total: 2,
        profile_inflight_releases_total: 3,
        profile_inflight_release_underflows_total: 4,
    });

    assert_eq!(metrics.health.pid, 42);
    assert_eq!(metrics.health.persistence_role, "owner");
    assert_eq!(metrics.health.active_requests, 3);
    assert_eq!(metrics.admission_wait.wait_total_ns, 20);
    assert_eq!(metrics.long_lived_queue_wait.wait_count, 3);
    assert_eq!(metrics.profile_inflight.get("main"), Some(&2));
    assert_eq!(metrics.retry_backoffs, 1);
    assert_eq!(metrics.transport_backoffs, 1);
    assert_eq!(metrics.route_circuits, 0);
    assert_eq!(metrics.degraded_profiles, 1);
    assert_eq!(metrics.degraded_routes, 1);
    assert_eq!(metrics.continuations.verified, 1);
    assert_eq!(metrics.continuations.stale_verified_bindings.response, 1);
    assert_eq!(
        metrics
            .previous_response_continuity
            .negative_cache_entries
            .responses,
        1
    );
    assert_eq!(metrics.active_request_release_underflows_total, 1);
    assert_eq!(
        metrics
            .continuity_failure_reasons
            .stale_continuation
            .get("watchdog"),
        Some(&1)
    );
}
