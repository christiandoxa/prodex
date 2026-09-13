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
    let decay = now
        .saturating_sub(entry.runtime_profile_health_updated_at())
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.runtime_profile_health_score().saturating_sub(decay)
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

pub fn runtime_profile_route_coupling_score_by_key<F>(
    health_entry: F,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_by_key(
                health_entry,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_by_key(
                health_entry,
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

pub fn runtime_profile_route_performance_score<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
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

pub fn runtime_profile_route_performance_score_by_key<F>(
    health_entry: F,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    let route_score = runtime_profile_effective_score_by_key(
        health_entry,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_by_key(
                health_entry,
                &runtime_profile_route_performance_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
            .saturating_div(2)
        })
        .fold(0, u32::saturating_add);
    route_score.saturating_add(coupled_score)
}

pub fn runtime_profile_health_sort_key<T: RuntimeProfileHealthEntry>(
    profile_name: &str,
    profile_health: &BTreeMap<String, T>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    #[cfg(feature = "mojo")]
    {
        return runtime_profile_health_sort_key_mojo(
            profile_health_score_input(
                |key| {
                    profile_health.get(key).map(|entry| {
                        (
                            entry.runtime_profile_health_score(),
                            entry.runtime_profile_health_updated_at(),
                        )
                    })
                },
                profile_name,
                route_kind,
            ),
            now,
        );
    }

    #[cfg(not(feature = "mojo"))]
    runtime_profile_health_sort_key_rust(profile_name, profile_health, now, route_kind)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_health_sort_key_rust<T: RuntimeProfileHealthEntry>(
    profile_name: &str,
    profile_health: &BTreeMap<String, T>,
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

pub fn runtime_profile_health_sort_key_by_key<F>(
    profile_name: &str,
    health_entry: F,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    #[cfg(feature = "mojo")]
    {
        return runtime_profile_health_sort_key_mojo(
            profile_health_score_input(
                |key| health_entry(key).map(|entry| (entry.score, entry.updated_at)),
                profile_name,
                route_kind,
            ),
            now,
        );
    }

    #[cfg(not(feature = "mojo"))]
    runtime_profile_health_sort_key_by_key_rust(profile_name, health_entry, now, route_kind)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_health_sort_key_by_key_rust<F>(
    profile_name: &str,
    health_entry: F,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    runtime_profile_effective_health_score_by_key(health_entry, profile_name, now)
        .saturating_add(runtime_profile_effective_health_score_by_key(
            health_entry,
            &runtime_profile_route_health_key(profile_name, route_kind),
            now,
        ))
        .saturating_add(runtime_profile_effective_score_by_key(
            health_entry,
            &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
            now,
            RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        ))
        .saturating_add(runtime_profile_route_coupling_score_by_key(
            health_entry,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score_by_key(
            health_entry,
            profile_name,
            now,
            route_kind,
        ))
}

#[cfg(feature = "mojo")]
fn profile_health_score_input<F>(
    health_entry: F,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> prodex_mojo_core::runtime::ProfileHealthScoreInput
where
    F: Fn(&str) -> Option<(u32, i64)> + Copy,
{
    let coupled_kind = runtime_route_coupled_kinds(route_kind)
        .first()
        .copied()
        .expect("every runtime route has one coupled route");
    let value = |key: String| health_entry(&key).unwrap_or((0, 0));
    let (global_score, global_updated_at) = value(profile_name.to_string());
    let (route_health_score, route_health_updated_at) =
        value(runtime_profile_route_health_key(profile_name, route_kind));
    let (route_bad_pairing_score, route_bad_pairing_updated_at) = value(
        runtime_profile_route_bad_pairing_key(profile_name, route_kind),
    );
    let (coupled_health_score, coupled_health_updated_at) =
        value(runtime_profile_route_health_key(profile_name, coupled_kind));
    let (coupled_bad_pairing_score, coupled_bad_pairing_updated_at) = value(
        runtime_profile_route_bad_pairing_key(profile_name, coupled_kind),
    );
    let (route_performance_score, route_performance_updated_at) = value(
        runtime_profile_route_performance_key(profile_name, route_kind),
    );
    let (coupled_performance_score, coupled_performance_updated_at) = value(
        runtime_profile_route_performance_key(profile_name, coupled_kind),
    );
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

#[cfg(feature = "mojo")]
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
    .expect("single profile health score must be returned")
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_parity_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn health_sort_key_feature_path_matches_rust_oracle() {
        let now = 100;
        let health = BTreeMap::from([
            (
                "alpha".to_string(),
                crate::RuntimeProfileHealth {
                    score: 5,
                    updated_at: 100,
                },
            ),
            (
                runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
                crate::RuntimeProfileHealth {
                    score: 3,
                    updated_at: 100,
                },
            ),
            (
                runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Responses),
                crate::RuntimeProfileHealth {
                    score: 2,
                    updated_at: 100,
                },
            ),
            (
                runtime_profile_route_health_key("alpha", RuntimeRouteKind::Websocket),
                crate::RuntimeProfileHealth {
                    score: 4,
                    updated_at: 100,
                },
            ),
            (
                runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Websocket),
                crate::RuntimeProfileHealth {
                    score: 2,
                    updated_at: 100,
                },
            ),
            (
                runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Responses),
                crate::RuntimeProfileHealth {
                    score: 8,
                    updated_at: 100,
                },
            ),
            (
                runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Websocket),
                crate::RuntimeProfileHealth {
                    score: 4,
                    updated_at: 100,
                },
            ),
        ]);

        let expected = runtime_profile_health_sort_key_rust(
            "alpha",
            &health,
            now,
            RuntimeRouteKind::Responses,
        );
        assert_eq!(
            runtime_profile_health_sort_key("alpha", &health, now, RuntimeRouteKind::Responses),
            expected
        );

        let expected = runtime_profile_health_sort_key_by_key_rust(
            "alpha",
            |key| {
                health.get(key).map(|entry| RuntimeProfileHealthSnapshot {
                    score: entry.score,
                    updated_at: entry.updated_at,
                })
            },
            now,
            RuntimeRouteKind::Responses,
        );
        assert_eq!(
            runtime_profile_health_sort_key_by_key(
                "alpha",
                |key| {
                    health.get(key).map(|entry| RuntimeProfileHealthSnapshot {
                        score: entry.score,
                        updated_at: entry.updated_at,
                    })
                },
                now,
                RuntimeRouteKind::Responses,
            ),
            expected
        );
    }
}
