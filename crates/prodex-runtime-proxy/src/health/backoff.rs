use std::collections::BTreeMap;

use crate::{RuntimeRouteKind, runtime_route_kind_from_label};

use super::{
    RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS,
    RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS, RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS,
    RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS, RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD,
    RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE, RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    RuntimeProfileBackoffs, RuntimeProfileHealthEntry,
    runtime_profile_effective_health_score_from_map, runtime_profile_route_circuit_key,
    runtime_profile_route_health_key, runtime_profile_route_key_parts,
    runtime_profile_transport_backoff_key, runtime_profile_transport_backoff_profile_name,
};

pub fn runtime_profile_transport_backoff_until_from_map(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    [
        transport_backoff_until.get(&route_key).copied(),
        transport_backoff_until.get(profile_name).copied(),
    ]
    .into_iter()
    .flatten()
    .filter(|until| *until > now)
    .max()
}

pub fn runtime_profile_transport_backoff_max_until(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    now: i64,
) -> Option<i64> {
    transport_backoff_until
        .iter()
        .filter(|(key, until)| {
            runtime_profile_transport_backoff_profile_name(key) == profile_name && **until > now
        })
        .map(|(_, until)| *until)
        .max()
}

pub fn runtime_profile_name_in_retry_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    now: i64,
) -> bool {
    retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
}

pub fn runtime_profile_name_in_transport_backoff(
    profile_name: &str,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_profile_transport_backoff_until_from_map(
        transport_backoff_until,
        profile_name,
        route_kind,
        now,
    )
    .is_some()
}

pub fn runtime_profile_name_in_retry_or_transport_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_profile_name_in_retry_backoff(profile_name, retry_backoff_until, now)
        || runtime_profile_name_in_transport_backoff(
            profile_name,
            transport_backoff_until,
            route_kind,
            now,
        )
}

pub fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    if retry_backoff_until.is_empty()
        && transport_backoff_until.is_empty()
        && route_circuit_open_until.is_empty()
    {
        return false;
    }
    runtime_profile_name_in_retry_or_transport_backoff(
        profile_name,
        retry_backoff_until,
        transport_backoff_until,
        route_kind,
        now,
    ) || route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .is_some_and(|until| until > now)
}

pub fn runtime_profile_backoff_sort_key(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (usize, i64, i64, i64) {
    let retry_until = retry_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let transport_until = runtime_profile_transport_backoff_until_from_map(
        transport_backoff_until,
        profile_name,
        route_kind,
        now,
    );
    let circuit_until = route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now);

    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::runtime::profile_backoff_sort_key(
            circuit_until,
            transport_until,
            retry_until,
            now,
        )
        .unwrap_or_else(|error| panic!("Mojo profile backoff sort key failed: {error:?}"))
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_backoff_sort_key_rust(circuit_until, transport_until, retry_until)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_backoff_sort_key_rust(
    circuit_until: Option<i64>,
    transport_until: Option<i64>,
    retry_until: Option<i64>,
) -> (usize, i64, i64, i64) {
    match (circuit_until, transport_until, retry_until) {
        (None, None, None) => (0, 0, 0, 0),
        (Some(circuit_until), None, None) => (1, circuit_until, 0, 0),
        (None, Some(transport_until), None) => (2, transport_until, 0, 0),
        (None, None, Some(retry_until)) => (3, retry_until, 0, 0),
        (Some(circuit_until), Some(transport_until), None) => (
            4,
            circuit_until.min(transport_until),
            circuit_until.max(transport_until),
            0,
        ),
        (Some(circuit_until), None, Some(retry_until)) => (
            5,
            circuit_until.min(retry_until),
            circuit_until.max(retry_until),
            0,
        ),
        (None, Some(transport_until), Some(retry_until)) => (
            6,
            transport_until.min(retry_until),
            transport_until.max(retry_until),
            0,
        ),
        (Some(circuit_until), Some(transport_until), Some(retry_until)) => (
            7,
            circuit_until.min(transport_until.min(retry_until)),
            circuit_until.max(transport_until.max(retry_until)),
            retry_until,
        ),
    }
}

pub fn runtime_soften_persisted_backoff_map_for_startup(
    backoffs: &mut BTreeMap<String, i64>,
    now: i64,
    max_future_seconds: i64,
) -> bool {
    #[cfg(feature = "mojo")]
    {
        let mut changed = false;
        backoffs.retain(|_, until| {
            let softened = prodex_mojo_core::runtime::profile_soften_backoff_until(
                *until,
                now,
                max_future_seconds,
            )
            .unwrap_or_else(|error| panic!("Mojo profile backoff softening failed: {error:?}"));
            changed |= softened.changed;
            *until = softened.until;
            softened.keep
        });
        changed
    }
    #[cfg(not(feature = "mojo"))]
    runtime_soften_persisted_backoff_map_for_startup_rust(backoffs, now, max_future_seconds)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_soften_persisted_backoff_map_for_startup_rust(
    backoffs: &mut BTreeMap<String, i64>,
    now: i64,
    max_future_seconds: i64,
) -> bool {
    let max_until = now.saturating_add(max_future_seconds.max(0));
    let mut changed = false;
    backoffs.retain(|_, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::runtime::profile_circuit_half_open_seconds(
            score,
            RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD,
            RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS,
            RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS,
        )
        .unwrap_or_else(|error| panic!("Mojo half-open circuit timing failed: {error:?}"))
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_circuit_half_open_probe_seconds_rust(score)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_circuit_half_open_probe_seconds_rust(score: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS)
}

pub fn runtime_profile_circuit_open_seconds(score: u32, reopen_stage: u32) -> i64 {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::runtime::profile_circuit_open_seconds(
            score,
            reopen_stage,
            RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD,
            RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE,
            RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS,
            RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS,
        )
        .unwrap_or_else(|error| panic!("Mojo circuit-open timing failed: {error:?}"))
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_circuit_open_seconds_rust(score, reopen_stage)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_circuit_open_seconds_rust(score: u32, reopen_stage: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3)
                .saturating_add(reopen_stage.min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS)
}

pub fn runtime_profile_route_circuit_probe_seconds<T: RuntimeProfileHealthEntry>(
    profile_scores: &BTreeMap<String, T>,
    route_profile_key: &str,
    now: i64,
) -> i64 {
    let Some((route_label, profile_name)) =
        runtime_profile_route_key_parts(route_profile_key, "__route_circuit__:")
    else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let Some(route_kind) = runtime_route_kind_from_label(route_label) else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let score = runtime_profile_effective_health_score_from_map(
        profile_scores,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    );
    runtime_profile_circuit_half_open_probe_seconds(score)
}

pub fn runtime_soften_persisted_route_circuits_for_startup<T: RuntimeProfileHealthEntry>(
    route_circuit_open_until: &mut BTreeMap<String, i64>,
    profile_scores: &BTreeMap<String, T>,
    now: i64,
) -> bool {
    let mut changed = false;
    route_circuit_open_until.retain(|route_profile_key, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let max_until = now.saturating_add(runtime_profile_route_circuit_probe_seconds(
            profile_scores,
            route_profile_key,
            now,
        ));
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub fn runtime_soften_persisted_backoffs_for_startup<T: RuntimeProfileHealthEntry>(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, T>,
    now: i64,
) -> bool {
    let mut changed = runtime_soften_persisted_backoff_map_for_startup(
        &mut backoffs.transport_backoff_until,
        now,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_parity_tests {
    use super::*;

    #[test]
    fn backoff_sort_key_matches_rust_oracle() {
        let now = 100_i64;
        for circuit in [None, Some(90), Some(110), Some(150)] {
            for transport in [None, Some(80), Some(120), Some(160)] {
                for retry in [None, Some(70), Some(130), Some(170)] {
                    let active_circuit = circuit.filter(|until| *until > now);
                    let active_transport = transport.filter(|until| *until > now);
                    let active_retry = retry.filter(|until| *until > now);
                    assert_eq!(
                        prodex_mojo_core::runtime::profile_backoff_sort_key(
                            circuit, transport, retry, now,
                        )
                        .unwrap(),
                        runtime_profile_backoff_sort_key_rust(
                            active_circuit,
                            active_transport,
                            active_retry,
                        )
                    );
                }
            }
        }
    }

    #[test]
    fn circuit_timings_match_rust_oracle() {
        for score in [0_u32, RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD, 10, u32::MAX] {
            assert_eq!(
                runtime_profile_circuit_half_open_probe_seconds(score),
                runtime_profile_circuit_half_open_probe_seconds_rust(score),
            );
            for stage in [0_u32, 1, RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE, u32::MAX] {
                assert_eq!(
                    runtime_profile_circuit_open_seconds(score, stage),
                    runtime_profile_circuit_open_seconds_rust(score, stage),
                );
            }
        }
    }

    #[test]
    fn persisted_softening_matches_rust_oracle() {
        for now in [-10_i64, 0, 100, i64::MAX - 20] {
            for max_future in [-5_i64, 0, 10, 60] {
                let source = BTreeMap::from([
                    ("past".to_string(), now.saturating_sub(1)),
                    ("near".to_string(), now.saturating_add(1)),
                    ("far".to_string(), now.saturating_add(10_000)),
                ]);
                let mut mojo = source.clone();
                let mut rust = source;
                assert_eq!(
                    runtime_soften_persisted_backoff_map_for_startup(&mut mojo, now, max_future,),
                    runtime_soften_persisted_backoff_map_for_startup_rust(
                        &mut rust, now, max_future,
                    )
                );
                assert_eq!(mojo, rust);
            }
        }
    }
}
