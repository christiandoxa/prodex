use std::collections::BTreeMap;

use crate::RuntimeRouteKind;

use super::{
    RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
    RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS, RuntimeProfileHealthEntry,
    RuntimeProfileHealthSnapshot, runtime_profile_route_bad_pairing_key,
    runtime_profile_route_health_key, runtime_profile_route_performance_key,
    runtime_route_coupled_kinds,
};

pub fn runtime_profile_effective_health_score<T: RuntimeProfileHealthEntry>(
    entry: &T,
    now: i64,
) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

pub fn runtime_profile_effective_score<T: RuntimeProfileHealthEntry>(
    entry: &T,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    prodex_mojo_core::runtime::profile_health_effective_score(
        entry.runtime_profile_health_score(),
        entry.runtime_profile_health_updated_at(),
        now,
        decay_seconds,
    )
    .unwrap_or_else(|error| panic!("Mojo profile effective health score failed: {error:?}"))
}

pub fn runtime_profile_effective_health_score_from_map<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

pub fn runtime_profile_effective_score_from_map<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

pub fn runtime_profile_effective_health_score_by_key<F>(health_entry: F, key: &str, now: i64) -> u32
where
    F: FnOnce(&str) -> Option<RuntimeProfileHealthSnapshot>,
{
    health_entry(key)
        .map(|entry| runtime_profile_effective_health_score(&entry, now))
        .unwrap_or(0)
}

pub fn runtime_profile_effective_score_by_key<F>(
    health_entry: F,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32
where
    F: FnOnce(&str) -> Option<RuntimeProfileHealthSnapshot>,
{
    health_entry(key)
        .map(|entry| runtime_profile_effective_score(&entry, now, decay_seconds))
        .unwrap_or(0)
}

pub fn runtime_profile_route_coupling_score_from_map<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_route_coupling_score_by_key(
        |key| {
            profile_health
                .get(key)
                .map(|entry| RuntimeProfileHealthSnapshot {
                    score: entry.runtime_profile_health_score(),
                    updated_at: entry.runtime_profile_health_updated_at(),
                })
        },
        profile_name,
        now,
        route_kind,
    )
}
pub fn runtime_profile_route_coupling_score_by_key<F>(
    health_entry: F,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    profile_health_coupling_score_mojo(
        profile_health_score_input(
            |key| health_entry(key).map(|entry| (entry.score, entry.updated_at)),
            profile_name,
            route_kind,
        ),
        now,
    )
}

pub fn runtime_profile_route_performance_score<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_route_performance_score_by_key(
        |key| {
            profile_health
                .get(key)
                .map(|entry| RuntimeProfileHealthSnapshot {
                    score: entry.runtime_profile_health_score(),
                    updated_at: entry.runtime_profile_health_updated_at(),
                })
        },
        profile_name,
        now,
        route_kind,
    )
}
pub fn runtime_profile_route_performance_score_by_key<F>(
    health_entry: F,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    profile_health_performance_score_mojo(
        profile_health_score_input(
            |key| health_entry(key).map(|entry| (entry.score, entry.updated_at)),
            profile_name,
            route_kind,
        ),
        now,
    )
}

pub fn runtime_profile_health_sort_key<T: RuntimeProfileHealthEntry>(
    profile_name: &str,
    profile_health: &BTreeMap<String, T>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_health_sort_key_by_key(
        profile_name,
        |key| {
            profile_health
                .get(key)
                .map(|entry| RuntimeProfileHealthSnapshot {
                    score: entry.runtime_profile_health_score(),
                    updated_at: entry.runtime_profile_health_updated_at(),
                })
        },
        now,
        route_kind,
    )
}
pub fn runtime_profile_health_sort_key_by_key<F>(
    profile_name: &str,
    health_entry: F,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    runtime_profile_health_sort_key_mojo(
        profile_health_score_input(
            |key| health_entry(key).map(|entry| (entry.score, entry.updated_at)),
            profile_name,
            route_kind,
        ),
        now,
    )
}

fn profile_health_score_input<F>(
    health_entry: F,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> prodex_mojo_core::runtime::ProfileHealthScoreInput
where
    F: Fn(&str) -> Option<(u32, i64)> + Copy,
{
    let coupled_kind = runtime_route_coupled_kinds(route_kind).first().copied();
    let value = |key: String| health_entry(&key).unwrap_or((0, 0));
    let (global_score, global_updated_at) = value(profile_name.to_string());
    let (route_health_score, route_health_updated_at) =
        value(runtime_profile_route_health_key(profile_name, route_kind));
    let (route_bad_pairing_score, route_bad_pairing_updated_at) = value(
        runtime_profile_route_bad_pairing_key(profile_name, route_kind),
    );
    let (coupled_health_score, coupled_health_updated_at) = coupled_kind
        .map(|coupled_kind| value(runtime_profile_route_health_key(profile_name, coupled_kind)))
        .unwrap_or_default();
    let (coupled_bad_pairing_score, coupled_bad_pairing_updated_at) = coupled_kind
        .map(|coupled_kind| {
            value(runtime_profile_route_bad_pairing_key(
                profile_name,
                coupled_kind,
            ))
        })
        .unwrap_or_default();
    let (route_performance_score, route_performance_updated_at) = value(
        runtime_profile_route_performance_key(profile_name, route_kind),
    );
    let (coupled_performance_score, coupled_performance_updated_at) = coupled_kind
        .map(|coupled_kind| {
            value(runtime_profile_route_performance_key(
                profile_name,
                coupled_kind,
            ))
        })
        .unwrap_or_default();
    prodex_mojo_core::runtime::ProfileHealthScoreInput {
        global_score,
        global_updated_at,
        route_health_score,
        route_health_updated_at,
        route_bad_pairing_score,
        route_bad_pairing_updated_at,
        coupled_health_score,
        coupled_health_updated_at,
        coupled_bad_pairing_score,
        coupled_bad_pairing_updated_at,
        route_performance_score,
        route_performance_updated_at,
        coupled_performance_score,
        coupled_performance_updated_at,
    }
}

fn profile_health_coupling_score_mojo(
    input: prodex_mojo_core::runtime::ProfileHealthScoreInput,
    now: i64,
) -> u32 {
    prodex_mojo_core::runtime::profile_health_coupling_score(
        input.coupled_health_score,
        input.coupled_health_updated_at,
        input.coupled_bad_pairing_score,
        input.coupled_bad_pairing_updated_at,
        now,
        RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    )
    .unwrap_or_else(|error| panic!("Mojo profile coupling score failed: {error:?}"))
}

fn profile_health_performance_score_mojo(
    input: prodex_mojo_core::runtime::ProfileHealthScoreInput,
    now: i64,
) -> u32 {
    prodex_mojo_core::runtime::profile_health_performance_score(
        input.route_performance_score,
        input.route_performance_updated_at,
        input.coupled_performance_score,
        input.coupled_performance_updated_at,
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    )
    .unwrap_or_else(|error| panic!("Mojo profile performance score failed: {error:?}"))
}

fn runtime_profile_health_sort_key_mojo(
    input: prodex_mojo_core::runtime::ProfileHealthScoreInput,
    now: i64,
) -> u32 {
    prodex_mojo_core::runtime::profile_health_sort_key_batch(
        &[input],
        now,
        RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    )
    .unwrap_or_else(|error| panic!("Mojo profile health score failed: {error:?}"))
    .into_iter()
    .next()
    .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_score_handles_decay_boundaries() {
        for (score, updated_at, now, decay_seconds, expected) in [
            (5, 10, 10, 0, 5),
            (5, 10, 9, 2, 5),
            (5, 10, 13, 2, 4),
            (5, 10, 100, 1, 0),
            (u32::MAX, i64::MIN, i64::MAX, i64::MIN, 0),
        ] {
            assert_eq!(
                runtime_profile_effective_score(
                    &RuntimeProfileHealthSnapshot { score, updated_at },
                    now,
                    decay_seconds,
                ),
                expected,
                "score={score} updated_at={updated_at} now={now} decay={decay_seconds}"
            );
        }
    }

    #[test]
    fn route_scores_and_sort_key_have_expected_values_and_saturate() {
        let now = 100;
        let at_now = |score| RuntimeProfileHealthSnapshot {
            score,
            updated_at: now,
        };
        let aged = |score, updated_at| RuntimeProfileHealthSnapshot { score, updated_at };
        let health = BTreeMap::from([
            ("alpha".to_string(), at_now(2)),
            (
                runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
                at_now(3),
            ),
            (
                runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Responses),
                at_now(1),
            ),
            (
                runtime_profile_route_health_key("alpha", RuntimeRouteKind::Websocket),
                aged(4, 96),
            ),
            (
                runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Websocket),
                aged(2, 96),
            ),
            (
                runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Responses),
                aged(8, 84),
            ),
            (
                runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Websocket),
                aged(4, 84),
            ),
        ]);

        assert_eq!(
            runtime_profile_route_coupling_score_from_map(
                &health,
                "alpha",
                now,
                RuntimeRouteKind::Responses,
            ),
            1,
        );
        assert_eq!(
            runtime_profile_route_performance_score(
                &health,
                "alpha",
                now,
                RuntimeRouteKind::Responses,
            ),
            7,
        );
        assert_eq!(
            runtime_profile_health_sort_key("alpha", &health, now, RuntimeRouteKind::Responses),
            14,
        );

        let maxed = BTreeMap::from([
            ("alpha".to_string(), at_now(u32::MAX)),
            (
                runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
                at_now(u32::MAX),
            ),
            (
                runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Responses),
                at_now(u32::MAX),
            ),
            (
                runtime_profile_route_health_key("alpha", RuntimeRouteKind::Websocket),
                at_now(u32::MAX),
            ),
            (
                runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Websocket),
                at_now(u32::MAX),
            ),
            (
                runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Responses),
                at_now(u32::MAX),
            ),
            (
                runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Websocket),
                at_now(u32::MAX),
            ),
        ]);
        assert_eq!(
            runtime_profile_health_sort_key("alpha", &maxed, now, RuntimeRouteKind::Responses),
            u32::MAX,
        );
    }
}
