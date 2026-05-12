use super::route::{
    runtime_profile_route_circuit_key, runtime_profile_route_circuit_profile_name,
    runtime_profile_route_health_key, runtime_profile_route_key_parts,
    runtime_profile_transport_backoff_key, runtime_profile_transport_backoff_key_parts,
    runtime_profile_transport_backoff_profile_name, runtime_route_kind_from_label,
};
use super::score::runtime_profile_effective_health_score_from_map;
use prodex_runtime_state::{RuntimeProfileBackoffs, RuntimeProfileHealth, RuntimeRouteKind};
use prodex_state::ProfileEntry;
use std::collections::BTreeMap;

pub fn compact_runtime_profile_backoffs(
    mut backoffs: RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    backoffs
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    backoffs.transport_backoff_until.retain(|key, until| {
        runtime_profile_transport_backoff_key_matches_profiles(key, profiles) && *until > now
    });
    backoffs
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    backoffs
}

pub fn merge_runtime_profile_backoffs(
    existing: &RuntimeProfileBackoffs,
    incoming: &RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    let mut merged = existing.clone();
    for (profile_name, until) in &incoming.retry_backoff_until {
        merged
            .retry_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (profile_name, until) in &incoming.transport_backoff_until {
        merged
            .transport_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (route_profile_key, until) in &incoming.route_circuit_open_until {
        merged
            .route_circuit_open_until
            .insert(route_profile_key.clone(), *until);
    }
    compact_runtime_profile_backoffs(merged, profiles, now)
}

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

pub fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
        || runtime_profile_transport_backoff_until_from_map(
            transport_backoff_until,
            profile_name,
            route_kind,
            now,
        )
        .is_some()
        || route_circuit_open_until
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

pub fn runtime_profile_circuit_open_seconds(score: u32, reopen_stage: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(crate::RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3)
                .saturating_add(reopen_stage.min(crate::RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)),
        )
        .unwrap_or(i64::MAX);
    crate::RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS
        .saturating_mul(multiplier)
        .min(crate::RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS)
}

pub fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(crate::RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3),
        )
        .unwrap_or(i64::MAX);
    crate::RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS
        .saturating_mul(multiplier)
        .min(crate::RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS)
}

pub fn runtime_profile_route_circuit_probe_seconds(
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    route_profile_key: &str,
    now: i64,
) -> i64 {
    let Some((route_label, profile_name)) =
        runtime_profile_route_key_parts(route_profile_key, "__route_circuit__:")
    else {
        return crate::RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let Some(route_kind) = runtime_route_kind_from_label(route_label) else {
        return crate::RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let score = runtime_profile_effective_health_score_from_map(
        profile_scores,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    );
    runtime_profile_circuit_half_open_probe_seconds(score)
}

pub fn runtime_soften_persisted_route_circuits_for_startup(
    route_circuit_open_until: &mut BTreeMap<String, i64>,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
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

pub fn runtime_soften_persisted_backoffs_for_startup(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = runtime_soften_persisted_backoff_map_for_startup(
        &mut backoffs.transport_backoff_until,
        now,
        crate::RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

fn runtime_profile_transport_backoff_key_matches_profiles(
    key: &str,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && profiles.contains_key(profile_name)
        })
        .unwrap_or_else(|| profiles.contains_key(key))
}
