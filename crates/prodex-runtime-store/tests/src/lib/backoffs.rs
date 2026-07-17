use super::*;
use prodex_runtime_state::{RuntimeProfileUsageSnapshot, RuntimeQuotaWindowStatus};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn usage_snapshot_compaction_prunes_missing_and_expired_profiles() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let snapshots = BTreeMap::from([
        (
            "alpha".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: 200,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: 0,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: 0,
            },
        ),
        (
            "missing".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: 200,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: 0,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: 0,
            },
        ),
    ]);

    let compacted = compact_runtime_usage_snapshots(snapshots, &profiles, 250);

    assert_eq!(compacted.len(), 1);
    assert!(compacted.contains_key("alpha"));
}

#[test]
fn usage_snapshot_timestamp_ties_merge_commutatively() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let snapshot = |status, remaining| RuntimeProfileUsageSnapshot {
        checked_at: 200,
        five_hour_status: status,
        five_hour_remaining_percent: remaining,
        five_hour_reset_at: 300,
        weekly_status: status,
        weekly_remaining_percent: remaining,
        weekly_reset_at: 400,
    };
    let left = BTreeMap::from([(
        "alpha".to_string(),
        snapshot(RuntimeQuotaWindowStatus::Ready, 90),
    )]);
    let right = BTreeMap::from([(
        "alpha".to_string(),
        snapshot(RuntimeQuotaWindowStatus::Critical, 5),
    )]);

    assert_eq!(
        merge_runtime_usage_snapshots(&left, &right, &profiles, 200),
        merge_runtime_usage_snapshots(&right, &left, &profiles, 200)
    );
}

#[test]
fn profile_score_compaction_supports_route_scoped_keys() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let scores = BTreeMap::from([
        (
            "__route_health__:responses:alpha".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 200,
            },
        ),
        (
            "__route_health__:responses:missing".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 200,
            },
        ),
    ]);

    let compacted = compact_runtime_profile_scores(scores, &profiles, 250);

    assert_eq!(
        compacted.keys().cloned().collect::<Vec<_>>(),
        vec!["__route_health__:responses:alpha".to_string()]
    );
}

#[test]
fn profile_score_timestamp_ties_merge_commutatively() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let left = BTreeMap::from([(
        "alpha".to_string(),
        RuntimeProfileHealth {
            score: 1,
            updated_at: 200,
        },
    )]);
    let right = BTreeMap::from([(
        "alpha".to_string(),
        RuntimeProfileHealth {
            score: 2,
            updated_at: 200,
        },
    )]);

    assert_eq!(
        merge_runtime_profile_scores(&left, &right, &profiles, 200),
        merge_runtime_profile_scores(&right, &left, &profiles, 200)
    );
}

#[test]
fn profile_score_clear_tombstone_wins_timestamp_ties() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let active = BTreeMap::from([(
        "alpha".to_string(),
        RuntimeProfileHealth {
            score: 2,
            updated_at: 200,
        },
    )]);
    let cleared = BTreeMap::from([(
        "alpha".to_string(),
        RuntimeProfileHealth {
            score: 0,
            updated_at: 200,
        },
    )]);

    for merged in [
        merge_runtime_profile_scores(&active, &cleared, &profiles, 200),
        merge_runtime_profile_scores(&cleared, &active, &profiles, 200),
    ] {
        assert_eq!(merged["alpha"].score, 0);
    }
}

#[test]
fn cleared_backoff_is_not_resurrected_by_stale_snapshot() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let update_key = runtime_profile_retry_backoff_update_key("alpha");
    let stale = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([("alpha".to_string(), 300)]),
        transport_backoff_until: BTreeMap::new(),
        route_circuit_open_until: BTreeMap::new(),
        updated_at: BTreeMap::from([(update_key.clone(), 1_000)]),
    };
    let cleared = RuntimeProfileBackoffs {
        updated_at: BTreeMap::from([(update_key, 2_000)]),
        ..RuntimeProfileBackoffs::default()
    };

    let merged = merge_runtime_profile_backoffs(&stale, &cleared, &profiles, 100);
    assert!(!merged.retry_backoff_until.contains_key("alpha"));
    let merged_again = merge_runtime_profile_backoffs(&merged, &stale, &profiles, 100);
    assert!(!merged_again.retry_backoff_until.contains_key("alpha"));
}

#[test]
fn profile_health_sort_key_includes_route_coupling_and_performance() {
    let scores = BTreeMap::from([
        (
            "alpha".to_string(),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_health_key("alpha", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 8,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 100,
            },
        ),
    ]);

    assert_eq!(
        runtime_profile_health_sort_key("alpha", &scores, 100, RuntimeRouteKind::Responses),
        19
    );
}

#[test]
fn previous_response_negative_cache_helpers_decay_and_clear_route_keys() {
    let mut scores = BTreeMap::from([
        (
            runtime_previous_response_negative_cache_key(
                "resp",
                "alpha",
                RuntimeRouteKind::Responses,
            ),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 100,
            },
        ),
        (
            runtime_previous_response_negative_cache_key(
                "resp",
                "alpha",
                RuntimeRouteKind::Websocket,
            ),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 100,
            },
        ),
    ]);

    assert_eq!(
        runtime_previous_response_negative_cache_failures(
            &scores,
            "resp",
            "alpha",
            RuntimeRouteKind::Responses,
            102,
            2,
        ),
        2
    );
    assert!(runtime_previous_response_negative_cache_active(
        &scores,
        "resp",
        "alpha",
        RuntimeRouteKind::Websocket,
        100,
        2,
    ));
    assert!(clear_runtime_previous_response_negative_cache(
        &mut scores,
        "resp",
        "alpha",
        103,
    ));
    assert!(
        scores
            .values()
            .all(|entry| entry.score == 0 && entry.updated_at == 103)
    );
}

#[test]
fn backoff_compaction_keeps_valid_route_keys_and_future_backoffs() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let backoffs = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([
            ("alpha".to_string(), 300),
            ("expired".to_string(), 100),
        ]),
        transport_backoff_until: BTreeMap::from([
            (
                "__route_transport_backoff__:responses:alpha".to_string(),
                300,
            ),
            ("__route_transport_backoff__:unknown:alpha".to_string(), 300),
            ("missing".to_string(), 300),
        ]),
        route_circuit_open_until: BTreeMap::from([
            ("__route_circuit__:responses:alpha".to_string(), 300),
            ("__route_circuit__:responses:missing".to_string(), 300),
        ]),
        updated_at: BTreeMap::new(),
    };

    let compacted = compact_runtime_profile_backoffs(backoffs, &profiles, 200);

    assert_eq!(
        compacted
            .retry_backoff_until
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["alpha".to_string()]
    );
    assert_eq!(
        compacted
            .transport_backoff_until
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["__route_transport_backoff__:responses:alpha".to_string()]
    );
    assert_eq!(
        compacted
            .route_circuit_open_until
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["__route_circuit__:responses:alpha".to_string()]
    );
}

#[test]
fn transport_backoff_helpers_prefer_route_scoped_future_values() {
    let profile_names = BTreeSet::from(["alpha".to_string()]);
    let route_key = runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Responses);
    let backoffs = BTreeMap::from([
        ("alpha".to_string(), 250),
        (route_key.clone(), 300),
        (
            runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Compact),
            100,
        ),
    ]);

    assert!(runtime_profile_transport_backoff_key_valid(
        &route_key,
        &profile_names
    ));
    assert_eq!(
        runtime_profile_transport_backoff_until_from_map(
            &backoffs,
            "alpha",
            RuntimeRouteKind::Responses,
            200,
        ),
        Some(300)
    );
    assert_eq!(
        runtime_profile_transport_backoff_max_until(&backoffs, "alpha", 200),
        Some(300)
    );
}

#[test]
fn selection_backoff_helpers_include_retry_transport_and_circuit() {
    let now = 100;
    let retry_backoff_until = BTreeMap::from([("alpha".to_string(), 110)]);
    let transport_backoff_until = BTreeMap::from([(
        runtime_profile_transport_backoff_key("beta", RuntimeRouteKind::Responses),
        120,
    )]);
    let route_circuit_open_until = BTreeMap::from([(
        runtime_profile_route_circuit_key("gamma", RuntimeRouteKind::Responses),
        130,
    )]);

    assert!(runtime_profile_name_in_selection_backoff(
        "alpha",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));
    assert!(runtime_profile_name_in_selection_backoff(
        "beta",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));
    assert!(runtime_profile_name_in_selection_backoff(
        "gamma",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));
    assert!(!runtime_profile_name_in_selection_backoff(
        "delta",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));

    assert_eq!(
        runtime_profile_backoff_sort_key(
            "gamma",
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            RuntimeRouteKind::Responses,
            now,
        ),
        (1, 130, 0, 0)
    );
    assert_eq!(
        runtime_profile_backoff_sort_key(
            "beta",
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            RuntimeRouteKind::Responses,
            now,
        ),
        (2, 120, 0, 0)
    );
    assert_eq!(
        runtime_profile_backoff_sort_key(
            "alpha",
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            RuntimeRouteKind::Responses,
            now,
        ),
        (3, 110, 0, 0)
    );
}

#[test]
fn startup_backoff_softening_prunes_expired_and_caps_future_route_state() {
    let now = 100;
    let transport_key = runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Responses);
    let circuit_key = runtime_profile_route_circuit_key("alpha", RuntimeRouteKind::Responses);
    let mut backoffs = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([("alpha".to_string(), 500)]),
        transport_backoff_until: BTreeMap::from([
            ("expired".to_string(), 90),
            (transport_key.clone(), 500),
        ]),
        route_circuit_open_until: BTreeMap::from([
            ("expired".to_string(), 90),
            (circuit_key.clone(), 500),
        ]),
        updated_at: BTreeMap::new(),
    };
    let profile_scores = BTreeMap::from([(
        runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
        RuntimeProfileHealth {
            score: RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
            updated_at: now,
        },
    )]);

    assert!(runtime_soften_persisted_backoffs_for_startup(
        &mut backoffs,
        &profile_scores,
        now,
    ));

    assert_eq!(backoffs.retry_backoff_until["alpha"], 500);
    assert_eq!(
        backoffs.transport_backoff_until,
        BTreeMap::from([(
            transport_key,
            now + RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
        )])
    );
    assert_eq!(
        backoffs.route_circuit_open_until,
        BTreeMap::from([(
            circuit_key,
            now + runtime_profile_circuit_half_open_probe_seconds(
                RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
            ),
        )])
    );
}
