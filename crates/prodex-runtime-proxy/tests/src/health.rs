use super::*;
use crate::RuntimeRouteKind;

#[test]
fn route_health_keys_match_runtime_labels() {
    assert_eq!(
        runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
        "__route_health__:responses:alpha"
    );
    assert_eq!(
        runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Websocket),
        "__route_transport_backoff__:websocket:alpha"
    );
    assert_eq!(
        runtime_profile_route_key_parts("__route_health__:compact:beta", "__route_health__:"),
        Some(("compact", "beta"))
    );
}

#[test]
fn weighted_inflight_hard_limit_always_admits_one_request() {
    assert_eq!(
        runtime_profile_inflight_effective_hard_limit("responses_http", 1),
        2
    );
    assert_eq!(
        runtime_profile_inflight_effective_hard_limit("standard_http", 1),
        1
    );
}

#[test]
fn effective_scores_decay_saturating() {
    let entry = RuntimeProfileHealth {
        score: 4,
        updated_at: 10,
    };

    assert_eq!(runtime_profile_effective_score(&entry, 10, 2), 4);
    assert_eq!(runtime_profile_effective_score(&entry, 14, 2), 2);
    assert_eq!(runtime_profile_effective_score(&entry, 100, 2), 0);
}

#[test]
fn route_coupling_affects_sort_key() {
    let now = 100;
    let mut health = BTreeMap::new();
    health.insert(
        runtime_profile_route_health_key("alpha", RuntimeRouteKind::Websocket),
        RuntimeProfileHealth {
            score: 4,
            updated_at: now,
        },
    );
    health.insert(
        runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Responses),
        RuntimeProfileHealth {
            score: 3,
            updated_at: now,
        },
    );

    assert_eq!(
        runtime_profile_health_sort_key("alpha", &health, now, RuntimeRouteKind::Responses),
        5
    );
}

#[test]
fn backoff_sort_key_orders_combined_backoffs() {
    let now = 10;
    let mut retry = BTreeMap::new();
    let mut transport = BTreeMap::new();
    let mut circuit = BTreeMap::new();
    retry.insert("alpha".to_string(), 30);
    transport.insert(
        runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Responses),
        20,
    );
    circuit.insert(
        runtime_profile_route_circuit_key("alpha", RuntimeRouteKind::Responses),
        40,
    );

    assert_eq!(
        runtime_profile_backoff_sort_key(
            "alpha",
            &retry,
            &transport,
            &circuit,
            RuntimeRouteKind::Responses,
            now,
        ),
        (7, 20, 40, 30)
    );
}

#[test]
fn latency_penalty_uses_route_stage_thresholds() {
    assert_eq!(
        runtime_profile_latency_penalty(120, RuntimeRouteKind::Responses, "ttfb"),
        0
    );
    assert_eq!(
        runtime_profile_latency_penalty(181, RuntimeRouteKind::Compact, "connect"),
        4
    );
    assert_eq!(
        runtime_profile_latency_failure_next_score(10),
        RUNTIME_PROFILE_LATENCY_PENALTY_MAX
    );
}

#[test]
fn selection_jitter_uses_sequence_profile_and_route() {
    let first = runtime_profile_selection_jitter(42, "alpha", RuntimeRouteKind::Responses);

    assert_eq!(first, 13_720_159_197_250_930_926);
    assert_eq!(
        first,
        runtime_profile_selection_jitter(42, "alpha", RuntimeRouteKind::Responses)
    );
    assert_ne!(
        first,
        runtime_profile_selection_jitter(43, "alpha", RuntimeRouteKind::Responses)
    );
    assert_eq!(
        runtime_profile_selection_jitter(43, "alpha", RuntimeRouteKind::Responses),
        2_052_999_783_678_899_254
    );
    assert_ne!(
        first,
        runtime_profile_selection_jitter(42, "beta", RuntimeRouteKind::Responses)
    );
    assert_eq!(
        runtime_profile_selection_jitter(42, "beta", RuntimeRouteKind::Responses),
        5_134_239_319_966_320_861
    );
    assert_ne!(
        first,
        runtime_profile_selection_jitter(42, "alpha", RuntimeRouteKind::Websocket)
    );
    assert_eq!(
        runtime_profile_selection_jitter(42, "alpha", RuntimeRouteKind::Websocket),
        19_596_772_748_915_481
    );
}

#[test]
fn health_bump_opens_circuit_and_tracks_reopen_stage() {
    assert_eq!(
        runtime_profile_health_bump_decision(1, 2, false, 3),
        RuntimeProfileHealthBumpDecision {
            next_score: 3,
            circuit_reopen_stage: None,
            circuit_open_seconds: None,
        }
    );
    assert_eq!(
        runtime_profile_health_bump_decision(3, 1, false, 3),
        RuntimeProfileHealthBumpDecision {
            next_score: 4,
            circuit_reopen_stage: Some(0),
            circuit_open_seconds: Some(RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS),
        }
    );
    assert_eq!(
        runtime_profile_health_bump_decision(4, 1, true, 1),
        RuntimeProfileHealthBumpDecision {
            next_score: 5,
            circuit_reopen_stage: Some(2),
            circuit_open_seconds: Some(runtime_profile_circuit_open_seconds(5, 2)),
        }
    );
}

#[test]
fn success_recovery_accelerates_after_first_streak() {
    assert_eq!(
        runtime_profile_health_recovery_decision(None, 2),
        RuntimeProfileHealthRecoveryDecision {
            next_score: None,
            next_success_streak: None,
        }
    );
    assert_eq!(
        runtime_profile_health_recovery_decision(Some(5), 0),
        RuntimeProfileHealthRecoveryDecision {
            next_score: Some(3),
            next_success_streak: Some(1),
        }
    );
    assert_eq!(
        runtime_profile_health_recovery_decision(Some(3), 1),
        RuntimeProfileHealthRecoveryDecision {
            next_score: None,
            next_success_streak: None,
        }
    );
}

#[test]
fn retry_or_transport_backoff_ignores_expired_entries() {
    let now = 10;
    let mut retry = BTreeMap::new();
    let mut transport = BTreeMap::new();
    retry.insert("alpha".to_string(), now - 1);
    transport.insert(
        runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Responses),
        now + 1,
    );

    assert!(!runtime_profile_name_in_retry_backoff("alpha", &retry, now));
    assert!(runtime_profile_name_in_transport_backoff(
        "alpha",
        &transport,
        RuntimeRouteKind::Responses,
        now,
    ));
    assert!(runtime_profile_name_in_retry_or_transport_backoff(
        "alpha",
        &retry,
        &transport,
        RuntimeRouteKind::Responses,
        now,
    ));
}

#[test]
fn mojo_active_backoff_reduction_respects_route_scope_and_strict_deadline() {
    let now = 100;
    let route = RuntimeRouteKind::Responses;
    let mut transport = BTreeMap::from([
        ("alpha".to_string(), now + 1),
        ("beta".to_string(), now + 900),
    ]);
    transport.insert(
        runtime_profile_transport_backoff_key("alpha", route),
        now + 11,
    );
    transport.insert(
        runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Websocket),
        now + 14,
    );
    assert_eq!(
        runtime_profile_transport_backoff_until_from_map(&transport, "alpha", route, now),
        Some(now + 11),
    );
    assert_eq!(
        runtime_profile_transport_backoff_max_until(&transport, "alpha", now),
        Some(now + 14),
    );
    assert_eq!(
        runtime_profile_transport_backoff_until_from_map(&transport, "alpha", route, now + 11),
        None,
    );
    assert_eq!(
        runtime_profile_transport_backoff_max_until(&transport, "alpha", now + 14),
        None,
    );
    assert_eq!(
        runtime_profile_transport_backoff_max_until(&transport, "missing", now),
        None,
    );

    let retry = BTreeMap::from([("alpha".to_string(), now + 9)]);
    let circuit = BTreeMap::from([(runtime_profile_route_circuit_key("alpha", route), now + 15)]);
    assert!(runtime_profile_name_in_retry_or_transport_backoff(
        "alpha", &retry, &transport, route, now,
    ));
    assert!(runtime_profile_name_in_selection_backoff(
        "alpha", &retry, &transport, &circuit, route, now,
    ));
    assert!(!runtime_profile_name_in_selection_backoff(
        "alpha",
        &retry,
        &transport,
        &circuit,
        route,
        now + 15,
    ));
    assert!(!runtime_profile_name_in_selection_backoff(
        "missing", &retry, &transport, &circuit, route, now,
    ));
}

#[test]
fn mojo_route_circuit_startup_softening_expires_clamps_and_is_idempotent() {
    let now = 1_000;
    let response_key = runtime_profile_route_circuit_key("alpha", RuntimeRouteKind::Responses);
    let websocket_key = runtime_profile_route_circuit_key("alpha", RuntimeRouteKind::Websocket);
    let mut circuits = BTreeMap::from([
        (response_key.clone(), now + 1_000),
        (websocket_key.clone(), now),
        ("unrecognized-circuit".to_owned(), now + 1_000),
    ]);
    let scores = BTreeMap::<String, RuntimeProfileHealth>::new();
    assert!(runtime_soften_persisted_route_circuits_for_startup(
        &mut circuits,
        &scores,
        now,
    ));
    let expected = now + RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    assert_eq!(circuits.get(&response_key), Some(&expected));
    assert_eq!(circuits.get(&websocket_key), None);
    assert_eq!(circuits.get("unrecognized-circuit"), Some(&expected));
    assert!(!runtime_soften_persisted_route_circuits_for_startup(
        &mut circuits,
        &scores,
        now,
    ));
    assert!(runtime_soften_persisted_route_circuits_for_startup(
        &mut circuits,
        &scores,
        expected,
    ));
    assert!(circuits.is_empty());
}

#[test]
fn startup_softening_clamps_future_backoffs() {
    let now = 100;
    let mut backoffs = RuntimeProfileBackoffs::default();
    backoffs
        .transport_backoff_until
        .insert("alpha".to_string(), now + 100);
    backoffs
        .transport_backoff_until
        .insert("beta".to_string(), now - 1);
    let profile_scores = BTreeMap::<String, RuntimeProfileHealth>::new();
    let changed =
        runtime_soften_persisted_backoffs_for_startup(&mut backoffs, &profile_scores, now);

    assert!(changed);
    assert_eq!(
        backoffs.transport_backoff_until.get("alpha").copied(),
        Some(now + RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS)
    );
    assert!(!backoffs.transport_backoff_until.contains_key("beta"));
}
