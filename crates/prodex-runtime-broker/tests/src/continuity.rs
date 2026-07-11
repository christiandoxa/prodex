use std::collections::BTreeMap;

use super::*;

#[test]
fn continuation_metrics_aggregate_lifecycle_signals_and_staleness() {
    let statuses = RuntimeContinuationStatuses {
        response: BTreeMap::from([
            (
                "resp-warm".to_string(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Warm,
                    failure_count: 2,
                    not_found_streak: 1,
                    ..RuntimeContinuationBindingStatus::default()
                },
            ),
            (
                "resp-stale".to_string(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Verified,
                    last_verified_at: Some(80),
                    failure_count: 3,
                    ..RuntimeContinuationBindingStatus::default()
                },
            ),
        ]),
        turn_state: BTreeMap::from([(
            "turn-suspect".to_string(),
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Suspect,
                not_found_streak: 4,
                ..RuntimeContinuationBindingStatus::default()
            },
        )]),
        session_id: BTreeMap::from([(
            "session-dead".to_string(),
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Dead,
                failure_count: 1,
                ..RuntimeContinuationBindingStatus::default()
            },
        )]),
    };

    let metrics = runtime_broker_continuation_metrics(&statuses, 100, 10);

    assert_eq!(metrics.response_bindings, 2);
    assert_eq!(metrics.turn_state_bindings, 1);
    assert_eq!(metrics.session_id_bindings, 1);
    assert_eq!(metrics.warm, 1);
    assert_eq!(metrics.verified, 1);
    assert_eq!(metrics.suspect, 1);
    assert_eq!(metrics.dead, 1);
    assert_eq!(
        metrics.failure_counts,
        RuntimeBrokerContinuationSignalMetrics {
            response: 5,
            turn_state: 0,
            session_id: 1,
        }
    );
    assert_eq!(
        metrics.not_found_streaks,
        RuntimeBrokerContinuationSignalMetrics {
            response: 1,
            turn_state: 4,
            session_id: 0,
        }
    );
    assert_eq!(
        metrics.stale_verified_bindings,
        RuntimeBrokerContinuationSignalMetrics {
            response: 1,
            turn_state: 0,
            session_id: 0,
        }
    );
}

#[test]
fn previous_response_continuity_metrics_count_active_known_routes_only() {
    let profile_health = BTreeMap::from([
        (
            "__previous_response_not_found__:responses:resp-1".to_string(),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 90,
            },
        ),
        (
            "__previous_response_not_found__:compact:resp-2".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 99,
            },
        ),
        (
            "__previous_response_not_found__:websocket:resp-3".to_string(),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 0,
            },
        ),
        (
            "__previous_response_not_found__:unknown:resp-4".to_string(),
            RuntimeProfileHealth {
                score: 9,
                updated_at: 99,
            },
        ),
        (
            "main".to_string(),
            RuntimeProfileHealth {
                score: 9,
                updated_at: 99,
            },
        ),
    ]);

    let metrics = runtime_broker_previous_response_continuity_metrics(&profile_health, 100, 5);

    assert_eq!(
        metrics.negative_cache_entries,
        RuntimeBrokerRouteContinuityMetrics {
            responses: 1,
            compact: 1,
            websocket: 0,
            standard: 0,
        }
    );
    assert_eq!(
        metrics.negative_cache_failures,
        RuntimeBrokerRouteContinuityMetrics {
            responses: 2,
            compact: 2,
            websocket: 0,
            standard: 0,
        }
    );
}

#[test]
fn continuity_failure_reason_metrics_merge_and_subtract_saturating() {
    let mut metrics = RuntimeBrokerContinuityFailureReasonMetrics {
        chain_retried_owner: BTreeMap::from([
            ("previous_response_not_found".to_string(), 2),
            ("stale".to_string(), 1),
        ]),
        chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 3)]),
        stale_continuation: BTreeMap::from([("watchdog".to_string(), 1)]),
    };

    runtime_broker_merge_continuity_failure_reason_metrics(
        &mut metrics,
        RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: BTreeMap::from([
                ("previous_response_not_found".to_string(), 4),
                ("new".to_string(), 1),
            ]),
            chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 2)]),
            stale_continuation: BTreeMap::from([("watchdog".to_string(), 3)]),
        },
    );

    assert_eq!(
        metrics.chain_retried_owner,
        BTreeMap::from([
            ("new".to_string(), 1),
            ("previous_response_not_found".to_string(), 6),
            ("stale".to_string(), 1),
        ])
    );
    assert_eq!(
        runtime_broker_subtract_continuity_failure_reason_metrics(
            metrics,
            &RuntimeBrokerContinuityFailureReasonMetrics {
                chain_retried_owner: BTreeMap::from([
                    ("new".to_string(), 2),
                    ("previous_response_not_found".to_string(), 1),
                ]),
                chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 2)]),
                stale_continuation: BTreeMap::from([("watchdog".to_string(), 10)]),
            },
        ),
        RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: BTreeMap::from([
                ("previous_response_not_found".to_string(), 5),
                ("stale".to_string(), 1),
            ]),
            chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 3)]),
            stale_continuation: BTreeMap::new(),
        }
    );
}

#[test]
fn continuity_failure_reason_metrics_parse_text_and_json_logs() {
    let log = br#"[2026-04-22 10:00:00.000 +00:00] request=3 chain_retried_owner profile=second reason=previous_response_not_found_locked_affinity
{"timestamp":"2026-04-22 10:00:01.000 +00:00","event":"stale_continuation","message":"stale_continuation reason=websocket_reuse_watchdog_locked_affinity","fields":{"reason":"websocket_reuse_watchdog_locked_affinity"}}
[2026-04-22 10:00:01.500 +00:00] request=3 ignored_marker reason=ignored
{"timestamp":"2026-04-22 10:00:01.750 +00:00","message":"chain_dead_upstream_confirmed reason=\"json message reason\"","fields":{}}
[2026-04-22 10:00:02.000 +00:00] request=3 chain_dead_upstream_confirmed reason="quoted reason"
"#;

    let metrics = runtime_broker_continuity_failure_reason_metrics_from_log_bytes(log);

    assert_eq!(
        metrics.chain_retried_owner,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
    );
    assert_eq!(
        metrics.stale_continuation,
        BTreeMap::from([("websocket_reuse_watchdog_locked_affinity".to_string(), 1,)])
    );
    assert_eq!(
        metrics.chain_dead_upstream_confirmed,
        BTreeMap::from([
            ("json message reason".to_string(), 1),
            ("quoted reason".to_string(), 1),
        ])
    );
}

#[test]
fn continuity_failure_reason_metrics_merge_live_without_double_counting() {
    let parsed = RuntimeBrokerContinuityFailureReasonMetrics {
        stale_continuation: BTreeMap::from([("previous_response_not_found".to_string(), 2)]),
        ..RuntimeBrokerContinuityFailureReasonMetrics::default()
    };
    let baseline = RuntimeBrokerContinuityFailureReasonMetrics {
        stale_continuation: BTreeMap::from([("previous_response_not_found".to_string(), 1)]),
        ..RuntimeBrokerContinuityFailureReasonMetrics::default()
    };
    let live = RuntimeBrokerContinuityFailureReasonMetrics {
        stale_continuation: BTreeMap::from([
            ("previous_response_not_found".to_string(), 1),
            ("watchdog".to_string(), 2),
        ]),
        ..RuntimeBrokerContinuityFailureReasonMetrics::default()
    };

    let metrics =
        runtime_broker_continuity_failure_reason_metrics_with_live(parsed, &baseline, live);

    assert_eq!(
        metrics.stale_continuation,
        BTreeMap::from([
            ("previous_response_not_found".to_string(), 2),
            ("watchdog".to_string(), 2),
        ])
    );
}

#[test]
fn degraded_health_metrics_separates_profiles_and_routes() {
    let profile_health = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 98,
            },
        ),
        (
            "__route_health__:responses:main".to_string(),
            RuntimeProfileHealth {
                score: 5,
                updated_at: 99,
            },
        ),
        (
            "__auth_failure__:main".to_string(),
            RuntimeProfileHealth {
                score: 5,
                updated_at: 99,
            },
        ),
        (
            "stale".to_string(),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 90,
            },
        ),
    ]);

    assert_eq!(
        runtime_broker_degraded_health_metrics(&profile_health, 100, 2),
        RuntimeBrokerDegradedHealthMetrics {
            profiles: 1,
            routes: 1,
        }
    );
}
