use std::collections::{BTreeMap, BTreeSet};

use crate::{RuntimeRouteKind, runtime_route_kind_from_label, runtime_route_kind_label};

pub const RUNTIME_PROFILE_HEALTH_DECAY_SECONDS: i64 = if cfg!(test) { 2 } else { 60 };
pub const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
pub const RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
pub const RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 15 };
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS: i64 =
    if cfg!(test) { 20 } else { 60 };
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProfileBackoffs {
    pub retry_backoff_until: BTreeMap<String, i64>,
    pub transport_backoff_until: BTreeMap<String, i64>,
    pub route_circuit_open_until: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProfileHealth {
    pub score: u32,
    pub updated_at: i64,
}

pub fn runtime_route_coupled_kinds(route_kind: RuntimeRouteKind) -> &'static [RuntimeRouteKind] {
    match route_kind {
        RuntimeRouteKind::Responses => &[RuntimeRouteKind::Websocket],
        RuntimeRouteKind::Websocket => &[RuntimeRouteKind::Responses],
        RuntimeRouteKind::Compact => &[RuntimeRouteKind::Standard],
        RuntimeRouteKind::Standard => &[RuntimeRouteKind::Compact],
    }
}

pub fn runtime_profile_effective_health_score(entry: &RuntimeProfileHealth, now: i64) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

pub fn runtime_profile_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

pub fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

pub fn runtime_profile_effective_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

pub fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_success__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    key.replacen("__route_circuit__", "__route_health__", 1)
}

pub fn runtime_profile_route_circuit_reopen_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit_reopen__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_key_parts<'a>(
    key: &'a str,
    prefix: &str,
) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

pub fn runtime_profile_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_transport_backoff__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_profile_route_key_parts(key, "__route_transport_backoff__:")
}

pub fn runtime_profile_transport_backoff_profile_name(key: &str) -> &str {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(_, profile_name)| profile_name)
        .unwrap_or(key)
}

pub fn runtime_profile_transport_backoff_key_valid(
    key: &str,
    valid_profiles: &BTreeSet<String>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && valid_profiles.contains(profile_name)
        })
        .unwrap_or_else(|| valid_profiles.contains(key))
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

pub fn runtime_profile_route_coupling_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_from_map(
                profile_health,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_bad_pairing_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            route_score
                .saturating_add(bad_pairing_score)
                .saturating_div(2)
        })
        .fold(0, u32::saturating_add)
}

pub fn runtime_profile_route_performance_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    let route_score = runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_performance_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
            .saturating_div(2)
        })
        .fold(0, u32::saturating_add);
    route_score.saturating_add(coupled_score)
}

pub fn runtime_profile_health_sort_key(
    profile_name: &str,
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
        .saturating_add(runtime_profile_effective_health_score_from_map(
            profile_health,
            &runtime_profile_route_health_key(profile_name, route_kind),
            now,
        ))
        .saturating_add(runtime_profile_effective_score_from_map(
            profile_health,
            &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
            now,
            RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        ))
        .saturating_add(runtime_profile_route_coupling_score_from_map(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

pub fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    profile_inflight.get(profile_name).copied().unwrap_or(0)
}

pub fn runtime_profile_inflight_weight(context: &str) -> usize {
    match context {
        "websocket_session" | "responses_http" => 2,
        _ => 1,
    }
}

pub fn runtime_profile_inflight_soft_limit(
    route_kind: RuntimeRouteKind,
    pressure_mode: bool,
    base_limit: usize,
) -> usize {
    let base = base_limit.max(1);
    if !pressure_mode {
        return base;
    }
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => base.saturating_sub(1).max(1),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => base.saturating_sub(2).max(1),
    }
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

pub fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
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

pub fn runtime_profile_route_circuit_probe_seconds(
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
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
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

#[cfg(test)]
mod tests {
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
    fn startup_softening_clamps_future_backoffs() {
        let now = 100;
        let mut backoffs = RuntimeProfileBackoffs::default();
        backoffs
            .transport_backoff_until
            .insert("alpha".to_string(), now + 100);
        backoffs
            .transport_backoff_until
            .insert("beta".to_string(), now - 1);
        let changed =
            runtime_soften_persisted_backoffs_for_startup(&mut backoffs, &BTreeMap::new(), now);

        assert!(changed);
        assert_eq!(
            backoffs.transport_backoff_until.get("alpha").copied(),
            Some(now + RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS)
        );
        assert!(!backoffs.transport_backoff_until.contains_key("beta"));
    }
}
