use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{
    PrometheusTextOptions, RuntimeBrokerContinuationMetrics,
    RuntimeBrokerContinuationSignalMetrics, RuntimeBrokerContinuityFailureReasonMetrics,
    RuntimeBrokerLaneMetrics, RuntimeBrokerPreviousResponseContinuityMetrics,
    RuntimeBrokerRouteContinuityMetrics, RuntimeBrokerSnapshot, RuntimeBrokerStateLockWaitMetrics,
    RuntimeBrokerTrafficMetrics, RuntimeBrokerWaitDurationMetrics,
    format_runtime_broker_snapshot_summary, render_runtime_broker_prometheus,
    render_runtime_broker_prometheus_from_metrics, render_runtime_broker_prometheus_with_options,
    runtime_broker_prometheus_snapshot,
};
use prodex_runtime_broker as broker;

fn sample_snapshot() -> RuntimeBrokerSnapshot {
    let mut profile_inflight = BTreeMap::new();
    profile_inflight.insert("main".to_string(), 3);
    profile_inflight.insert("second".to_string(), 1);

    RuntimeBrokerSnapshot {
        broker_key: "broker-123".to_string(),
        listen_addr: "127.0.0.1:8080".to_string(),
        pid: 4242,
        started_at_unix_seconds: 1_715_000_000,
        current_profile: "main".to_string(),
        include_code_review: false,
        persistence_role: "owner".to_string(),
        prodex_version: Some("0.29.0".to_string()),
        executable_path: Some("/tmp/prodex".to_string()),
        executable_sha256: Some("abcd1234".to_string()),
        active_requests: 5,
        active_request_limit: 12,
        local_overload_backoff_remaining_seconds: 0,
        runtime_state_lock_wait: RuntimeBrokerStateLockWaitMetrics {
            wait_total_ns: 25_000_000,
            wait_count: 3,
            wait_max_ns: 10_000_000,
        },
        admission_wait: RuntimeBrokerWaitDurationMetrics {
            wait_total_ns: 15_000_000,
            wait_count: 2,
            wait_max_ns: 9_000_000,
        },
        long_lived_queue_wait: RuntimeBrokerWaitDurationMetrics {
            wait_total_ns: 8_000_000,
            wait_count: 1,
            wait_max_ns: 8_000_000,
        },
        traffic: RuntimeBrokerTrafficMetrics {
            responses: RuntimeBrokerLaneMetrics {
                active: 3,
                limit: 9,
                admissions_total: 42,
                releases_total: 40,
                global_limit_rejections_total: 2,
                lane_limit_rejections_total: 5,
                release_underflows_total: 1,
            },
            compact: RuntimeBrokerLaneMetrics {
                active: 1,
                limit: 3,
                admissions_total: 12,
                releases_total: 11,
                global_limit_rejections_total: 1,
                lane_limit_rejections_total: 4,
                release_underflows_total: 0,
            },
            websocket: RuntimeBrokerLaneMetrics {
                active: 0,
                limit: 4,
                admissions_total: 7,
                releases_total: 7,
                global_limit_rejections_total: 0,
                lane_limit_rejections_total: 1,
                release_underflows_total: 0,
            },
            standard: RuntimeBrokerLaneMetrics {
                active: 1,
                limit: 2,
                admissions_total: 9,
                releases_total: 8,
                global_limit_rejections_total: 3,
                lane_limit_rejections_total: 2,
                release_underflows_total: 0,
            },
        },
        profile_inflight,
        active_request_release_underflows_total: 1,
        profile_inflight_admissions_total: 17,
        profile_inflight_releases_total: 16,
        profile_inflight_release_underflows_total: 1,
        retry_backoffs: 2,
        transport_backoffs: 1,
        route_circuits: 4,
        degraded_profiles: 1,
        degraded_routes: 2,
        continuations: RuntimeBrokerContinuationMetrics {
            response_bindings: 7,
            turn_state_bindings: 2,
            session_id_bindings: 1,
            warm: 1,
            verified: 3,
            suspect: 2,
            dead: 1,
            failure_counts: RuntimeBrokerContinuationSignalMetrics {
                response: 5,
                turn_state: 2,
                session_id: 1,
            },
            not_found_streaks: RuntimeBrokerContinuationSignalMetrics {
                response: 3,
                turn_state: 1,
                session_id: 0,
            },
            stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics {
                response: 1,
                turn_state: 0,
                session_id: 1,
            },
        },
        previous_response_continuity: RuntimeBrokerPreviousResponseContinuityMetrics {
            negative_cache_entries: RuntimeBrokerRouteContinuityMetrics {
                responses: 2,
                compact: 1,
                websocket: 1,
                standard: 0,
            },
            negative_cache_failures: RuntimeBrokerRouteContinuityMetrics {
                responses: 4,
                compact: 2,
                websocket: 1,
                standard: 0,
            },
        },
        continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: BTreeMap::from([(
                "previous_response_not_found_locked_affinity".to_string(),
                2,
            )]),
            chain_dead_upstream_confirmed: BTreeMap::from([(
                "previous_response_not_found_locked_affinity".to_string(),
                1,
            )]),
            stale_continuation: BTreeMap::from([
                ("previous_response_not_found".to_string(), 3),
                ("tenant-secret-token".to_string(), 2),
                ("websocket_reuse_watchdog_locked_affinity".to_string(), 1),
            ]),
        },
    }
}

fn sample_broker_metadata() -> broker::RuntimeBrokerMetadata {
    broker::RuntimeBrokerMetadata {
        broker_key: "broker-123".to_string(),
        listen_addr: "127.0.0.1:8080".to_string(),
        started_at: 1_715_000_000,
        current_profile: "main".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        instance_id: "instance".to_string(),
        admin_token: Arc::new(broker::RuntimeBrokerSecret::new("admin").unwrap()),
        prodex_version: Some("0.29.0".to_string()),
        executable_path: Some("/tmp/prodex".to_string()),
        executable_sha256: Some("abcd1234".to_string()),
    }
}

fn sample_broker_lane(active: usize, limit: usize) -> broker::RuntimeBrokerLaneMetrics {
    broker::RuntimeBrokerLaneMetrics {
        active,
        limit,
        admissions_total: 42,
        releases_total: 40,
        global_limit_rejections_total: 2,
        lane_limit_rejections_total: 5,
        release_underflows_total: 1,
    }
}

fn sample_broker_metrics() -> broker::RuntimeBrokerMetrics {
    broker::RuntimeBrokerMetrics {
        health: broker::RuntimeBrokerHealth {
            pid: 4242,
            started_at: 1_715_000_000,
            current_profile: "main".to_string(),
            include_code_review: false,
            active_requests: 5,
            instance_id: "instance".to_string(),
            persistence_role: "owner".to_string(),
            prodex_version: Some("0.29.0".to_string()),
            executable_path: Some("/tmp/prodex".to_string()),
            executable_sha256: Some("abcd1234".to_string()),
        },
        active_request_limit: 12,
        local_overload_backoff_remaining_seconds: 0,
        runtime_state_lock_wait: Default::default(),
        admission_wait: broker::RuntimeWaitDurationMetrics {
            wait_total_ns: 30,
            wait_count: 2,
            wait_max_ns: 20,
        },
        long_lived_queue_wait: broker::RuntimeWaitDurationMetrics {
            wait_total_ns: 40,
            wait_count: 3,
            wait_max_ns: 25,
        },
        traffic: broker::RuntimeBrokerTrafficMetrics {
            responses: sample_broker_lane(3, 9),
            compact: sample_broker_lane(1, 3),
            websocket: sample_broker_lane(0, 4),
            standard: sample_broker_lane(1, 2),
        },
        profile_inflight: BTreeMap::from([("main".to_string(), 3), ("second".to_string(), 1)]),
        active_request_release_underflows_total: 1,
        profile_inflight_admissions_total: 17,
        profile_inflight_releases_total: 16,
        profile_inflight_release_underflows_total: 1,
        retry_backoffs: 2,
        transport_backoffs: 1,
        route_circuits: 4,
        degraded_profiles: 1,
        degraded_routes: 2,
        continuations: broker::RuntimeBrokerContinuationMetrics {
            response_bindings: 7,
            turn_state_bindings: 2,
            session_id_bindings: 1,
            warm: 1,
            verified: 3,
            suspect: 2,
            dead: 1,
            failure_counts: broker::RuntimeBrokerContinuationSignalMetrics {
                response: 5,
                turn_state: 2,
                session_id: 1,
            },
            not_found_streaks: broker::RuntimeBrokerContinuationSignalMetrics {
                response: 3,
                turn_state: 1,
                session_id: 0,
            },
            stale_verified_bindings: broker::RuntimeBrokerContinuationSignalMetrics {
                response: 1,
                turn_state: 0,
                session_id: 1,
            },
        },
        previous_response_continuity: broker::RuntimeBrokerPreviousResponseContinuityMetrics {
            negative_cache_entries: broker::RuntimeBrokerRouteContinuityMetrics {
                responses: 2,
                compact: 1,
                websocket: 1,
                standard: 0,
            },
            negative_cache_failures: broker::RuntimeBrokerRouteContinuityMetrics {
                responses: 4,
                compact: 2,
                websocket: 1,
                standard: 0,
            },
        },
        continuity_failure_reasons: broker::RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: BTreeMap::from([(
                "previous_response_not_found_locked_affinity".to_string(),
                2,
            )]),
            chain_dead_upstream_confirmed: BTreeMap::from([(
                "previous_response_not_found_locked_affinity".to_string(),
                1,
            )]),
            stale_continuation: BTreeMap::from([
                ("previous_response_not_found".to_string(), 3),
                ("websocket_reuse_watchdog_locked_affinity".to_string(), 1),
            ]),
        },
    }
}

#[test]
fn builds_prometheus_snapshot_from_broker_metrics() {
    let snapshot =
        runtime_broker_prometheus_snapshot(&sample_broker_metadata(), &sample_broker_metrics());

    assert_eq!(snapshot.broker_key, "broker-123");
    assert_eq!(snapshot.pid, 4242);
    assert_eq!(snapshot.traffic.responses.active, 3);
    assert_eq!(snapshot.traffic.responses.limit, 9);
    assert_eq!(snapshot.profile_inflight.get("main"), Some(&3));
    assert_eq!(snapshot.admission_wait.wait_total_ns, 30);
    assert_eq!(snapshot.long_lived_queue_wait.wait_count, 3);
    assert_eq!(snapshot.continuations.response_bindings, 7);
    assert_eq!(
        snapshot
            .continuity_failure_reasons
            .stale_continuation
            .get("previous_response_not_found"),
        Some(&3)
    );

    let rendered = render_runtime_broker_prometheus_from_metrics(
        &sample_broker_metadata(),
        &sample_broker_metrics(),
    );
    assert!(rendered.contains("prodex_runtime_broker_active_requests"));
    assert!(rendered.contains("broker_key=\"broker-123\""));
}

#[test]
fn renders_prometheus_text_with_help_and_labels() {
    let rendered = render_runtime_broker_prometheus(&sample_snapshot());
    assert!(rendered.contains("# HELP prodex_runtime_broker_info"));
    assert!(rendered.contains("# TYPE prodex_runtime_broker_info gauge"));
    assert!(rendered.contains("prodex_runtime_broker_active_requests"));
    assert!(rendered.contains("prodex_runtime_broker_runtime_state_lock_wait_total_seconds"));
    assert!(rendered.contains(
            "prodex_runtime_broker_runtime_state_lock_acquisitions_total{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 3"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_runtime_state_lock_wait_max_seconds{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 0.01"
        ));
    assert!(rendered.contains(
        "prodex_runtime_broker_admission_wait_total_seconds{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 0.015"
    ));
    assert!(rendered.contains(
        "prodex_runtime_broker_admission_waits_total{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 2"
    ));
    assert!(rendered.contains(
        "prodex_runtime_broker_admission_wait_max_seconds{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 0.009"
    ));
    assert!(rendered.contains(
        "prodex_runtime_broker_long_lived_queue_wait_total_seconds{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 0.008"
    ));
    assert!(rendered.contains(
        "prodex_runtime_broker_long_lived_queue_waits_total{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 1"
    ));
    assert!(rendered.contains(
        "prodex_runtime_broker_long_lived_queue_wait_max_seconds{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 0.008"
    ));
    assert!(rendered.contains("prodex_runtime_broker_lane_admissions_total"));
    assert!(rendered.contains("prodex_runtime_broker_lane_releases_total"));
    assert!(rendered.contains("prodex_runtime_broker_lane_global_limit_rejections_total"));
    assert!(rendered.contains("prodex_runtime_broker_lane_lane_limit_rejections_total"));
    assert!(rendered.contains("prodex_runtime_broker_lane_release_underflows_total"));
    assert!(rendered.contains("prodex_runtime_broker_active_request_release_underflows_total"));
    assert!(rendered.contains("prodex_runtime_broker_profile_inflight_release_underflows_total"));
    assert!(rendered.contains("# HELP prodex_runtime_broker_continuation_binding_counts"));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_binding_counts{binding_kind=\"response\",broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 7"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_binding_counts{binding_kind=\"turn_state\",broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 2"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_binding_counts{binding_kind=\"session_id\",broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 1"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_bindings{broker_key=\"broker-123\",lifecycle=\"warm\",listen_addr=\"127.0.0.1:8080\"} 1"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_bindings{broker_key=\"broker-123\",lifecycle=\"verified\",listen_addr=\"127.0.0.1:8080\"} 3"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_failure_counts{binding_kind=\"response\",broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 5"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_not_found_streaks{binding_kind=\"turn_state\",broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 1"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuation_stale_verified_bindings{binding_kind=\"session_id\",broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 1"
        ));
    assert!(rendered.contains("# HELP prodex_runtime_broker_continuity_failures_total"));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuity_failures_total{broker_key=\"broker-123\",event=\"chain_retried_owner\",listen_addr=\"127.0.0.1:8080\",reason=\"previous_response_not_found_locked_affinity\"} 2"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuity_failures_total{broker_key=\"broker-123\",event=\"stale_continuation\",listen_addr=\"127.0.0.1:8080\",reason=\"websocket_reuse_watchdog_locked_affinity\"} 1"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_continuity_failures_total{broker_key=\"broker-123\",event=\"stale_continuation\",listen_addr=\"127.0.0.1:8080\",reason=\"other\"} 2"
        ));
    assert!(!rendered.contains("tenant-secret-token"));
    assert!(rendered.contains(
            "prodex_runtime_broker_previous_response_negative_cache_entries{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\",route=\"responses\"} 2"
        ));
    assert!(rendered.contains(
            "prodex_runtime_broker_previous_response_negative_cache_failures{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\",route=\"compact\"} 2"
        ));
    assert!(rendered.contains("broker_key=\"broker-123\""));
    assert!(rendered.contains("listen_addr=\"127.0.0.1:8080\""));
    assert!(!rendered.contains("current_profile="));
    assert!(rendered.contains("prodex_version=\"0.29.0\""));
    assert!(!rendered.contains("executable_path="));
    assert!(!rendered.contains("/tmp/prodex"));
    assert!(rendered.contains("executable_sha256=\"abcd1234\""));
    assert!(rendered.contains("lane=\"responses\""));
    assert!(rendered.contains(
        "prodex_runtime_broker_profile_inflight{broker_key=\"broker-123\",listen_addr=\"127.0.0.1:8080\"} 4"
    ));
    assert!(!rendered.contains("profile=\"main\""));
    assert!(!rendered.contains("profile=\"second\""));
}

#[test]
fn escapes_label_values_and_keeps_metric_order_stable() {
    let mut snapshot = sample_snapshot();
    snapshot.broker_key = "broker\\\"1\n".to_string();
    snapshot.listen_addr = "127.0.0.1:8080".to_string();
    let rendered = render_runtime_broker_prometheus_with_options(
        &snapshot,
        PrometheusTextOptions {
            include_help: false,
        },
    );

    assert!(rendered.contains("broker_key=\"broker\\\\\\\"1\\n\""));
    let first = rendered
        .lines()
        .find(|line| line.starts_with("prodex_runtime_broker_info"))
        .unwrap();
    let second = rendered
        .lines()
        .find(|line| line.starts_with("prodex_runtime_broker_active_requests"))
        .unwrap();
    assert!(first.starts_with("prodex_runtime_broker_info"));
    assert!(second.starts_with("prodex_runtime_broker_active_requests"));
}

#[test]
fn summary_is_concise_and_machine_readable() {
    let summary = format_runtime_broker_snapshot_summary(&sample_snapshot());
    assert!(summary.contains("broker_key=broker-123"));
    assert!(summary.contains("listen_addr=127.0.0.1:8080"));
    assert!(summary.contains("limits=9/3/4/2"));
    assert!(summary.contains("degraded_profiles=1"));
    assert!(summary.contains("degraded_routes=2"));
}
