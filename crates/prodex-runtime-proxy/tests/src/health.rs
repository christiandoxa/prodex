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

    assert_eq!(
        first,
        runtime_profile_selection_jitter(42, "alpha", RuntimeRouteKind::Responses)
    );
    assert_ne!(
        first,
        runtime_profile_selection_jitter(43, "alpha", RuntimeRouteKind::Responses)
    );
    assert_ne!(
        first,
        runtime_profile_selection_jitter(42, "beta", RuntimeRouteKind::Responses)
    );
    assert_ne!(
        first,
        runtime_profile_selection_jitter(42, "alpha", RuntimeRouteKind::Websocket)
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
