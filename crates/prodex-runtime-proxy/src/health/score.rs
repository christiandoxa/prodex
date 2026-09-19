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
    #[cfg(feature = "mojo")]
    {
        return prodex_mojo_core::runtime::profile_health_effective_score(
            entry.runtime_profile_health_score(),
            entry.runtime_profile_health_updated_at(),
            now,
            decay_seconds,
        )
        .unwrap_or_else(|error| panic!("Mojo profile effective health score failed: {error:?}"));
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_effective_score_rust(entry, now, decay_seconds)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_effective_score_rust<T: RuntimeProfileHealthEntry>(
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

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_effective_health_score_from_map_rust<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| {
            runtime_profile_effective_score_rust(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
        })
        .unwrap_or(0)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_effective_score_from_map_rust<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score_rust(entry, now, decay_seconds))
        .unwrap_or(0)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_effective_health_score_by_key_rust<F>(
    health_entry: F,
    key: &str,
    now: i64,
) -> u32
where
    F: FnOnce(&str) -> Option<RuntimeProfileHealthSnapshot>,
{
    health_entry(key)
        .map(|entry| {
            runtime_profile_effective_score_rust(&entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
        })
        .unwrap_or(0)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_effective_score_by_key_rust<F>(
    health_entry: F,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32
where
    F: FnOnce(&str) -> Option<RuntimeProfileHealthSnapshot>,
{
    health_entry(key)
        .map(|entry| runtime_profile_effective_score_rust(&entry, now, decay_seconds))
        .unwrap_or(0)
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
    #[cfg(feature = "mojo")]
    {
        return runtime_route_coupled_kinds(route_kind)
            .iter()
            .copied()
            .map(|coupled_kind| {
                let route = profile_health
                    .get(&runtime_profile_route_health_key(
                        profile_name,
                        coupled_kind,
                    ))
                    .map(|entry| {
                        (
                            entry.runtime_profile_health_score(),
                            entry.runtime_profile_health_updated_at(),
                        )
                    })
                    .unwrap_or_default();
                let bad = profile_health
                    .get(&runtime_profile_route_bad_pairing_key(
                        profile_name,
                        coupled_kind,
                    ))
                    .map(|entry| {
                        (
                            entry.runtime_profile_health_score(),
                            entry.runtime_profile_health_updated_at(),
                        )
                    })
                    .unwrap_or_default();
                prodex_mojo_core::runtime::profile_health_coupling_score(
                    route.0,
                    route.1,
                    bad.0,
                    bad.1,
                    now,
                    RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
                    RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
                )
                .unwrap_or_else(|error| panic!("Mojo profile coupling score failed: {error:?}"))
            })
            .fold(0, u32::saturating_add);
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_route_coupling_score_from_map_rust(
        profile_health,
        profile_name,
        now,
        route_kind,
    )
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_route_coupling_score_from_map_rust<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_from_map_rust(
                profile_health,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_from_map_rust(
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
    #[cfg(feature = "mojo")]
    {
        return runtime_route_coupled_kinds(route_kind)
            .iter()
            .copied()
            .map(|coupled_kind| {
                let route = health_entry(&runtime_profile_route_health_key(
                    profile_name,
                    coupled_kind,
                ))
                .map(|entry| (entry.score, entry.updated_at))
                .unwrap_or_default();
                let bad = health_entry(&runtime_profile_route_bad_pairing_key(
                    profile_name,
                    coupled_kind,
                ))
                .map(|entry| (entry.score, entry.updated_at))
                .unwrap_or_default();
                prodex_mojo_core::runtime::profile_health_coupling_score(
                    route.0,
                    route.1,
                    bad.0,
                    bad.1,
                    now,
                    RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
                    RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
                )
                .unwrap_or_else(|error| panic!("Mojo profile coupling score failed: {error:?}"))
            })
            .fold(0, u32::saturating_add);
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_route_coupling_score_by_key_rust(health_entry, profile_name, now, route_kind)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_route_coupling_score_by_key_rust<F>(
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
            let route_score = runtime_profile_effective_health_score_by_key_rust(
                health_entry,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_by_key_rust(
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
    #[cfg(feature = "mojo")]
    {
        let route = profile_health
            .get(&runtime_profile_route_performance_key(
                profile_name,
                route_kind,
            ))
            .map(|entry| {
                (
                    entry.runtime_profile_health_score(),
                    entry.runtime_profile_health_updated_at(),
                )
            })
            .unwrap_or_default();
        let coupled_kind = runtime_route_coupled_kinds(route_kind)
            .first()
            .copied()
            .expect("runtime routes always have one coupled route");
        let coupled = profile_health
            .get(&runtime_profile_route_performance_key(
                profile_name,
                coupled_kind,
            ))
            .map(|entry| {
                (
                    entry.runtime_profile_health_score(),
                    entry.runtime_profile_health_updated_at(),
                )
            })
            .unwrap_or_default();
        return prodex_mojo_core::runtime::profile_health_performance_score(
            route.0,
            route.1,
            coupled.0,
            coupled.1,
            now,
            RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
        )
        .unwrap_or_else(|error| panic!("Mojo profile performance score failed: {error:?}"));
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_route_performance_score_rust(profile_health, profile_name, now, route_kind)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_route_performance_score_rust<T: RuntimeProfileHealthEntry>(
    profile_health: &BTreeMap<String, T>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    let route_score = runtime_profile_effective_score_from_map_rust(
        profile_health,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_from_map_rust(
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
    #[cfg(feature = "mojo")]
    {
        let route = health_entry(&runtime_profile_route_performance_key(
            profile_name,
            route_kind,
        ))
        .map(|entry| (entry.score, entry.updated_at))
        .unwrap_or_default();
        let coupled_kind = runtime_route_coupled_kinds(route_kind)
            .first()
            .copied()
            .expect("runtime routes always have one coupled route");
        let coupled = health_entry(&runtime_profile_route_performance_key(
            profile_name,
            coupled_kind,
        ))
        .map(|entry| (entry.score, entry.updated_at))
        .unwrap_or_default();
        return prodex_mojo_core::runtime::profile_health_performance_score(
            route.0,
            route.1,
            coupled.0,
            coupled.1,
            now,
            RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
        )
        .unwrap_or_else(|error| panic!("Mojo profile performance score failed: {error:?}"));
    }
    #[cfg(not(feature = "mojo"))]
    runtime_profile_route_performance_score_by_key_rust(health_entry, profile_name, now, route_kind)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_route_performance_score_by_key_rust<F>(
    health_entry: F,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32
where
    F: Fn(&str) -> Option<RuntimeProfileHealthSnapshot> + Copy,
{
    let route_score = runtime_profile_effective_score_by_key_rust(
        health_entry,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_by_key_rust(
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
        runtime_profile_health_sort_key_mojo(
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
        )
    }

    #[cfg(not(feature = "mojo"))]
    {
        runtime_profile_health_sort_key_rust(profile_name, profile_health, now, route_kind)
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_profile_health_sort_key_rust<T: RuntimeProfileHealthEntry>(
    profile_name: &str,
    profile_health: &BTreeMap<String, T>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map_rust(profile_health, profile_name, now)
        .saturating_add(runtime_profile_effective_health_score_from_map_rust(
            profile_health,
            &runtime_profile_route_health_key(profile_name, route_kind),
            now,
        ))
        .saturating_add(runtime_profile_effective_score_from_map_rust(
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
        runtime_profile_health_sort_key_mojo(
            profile_health_score_input(
                |key| health_entry(key).map(|entry| (entry.score, entry.updated_at)),
                profile_name,
                route_kind,
            ),
            now,
        )
    }

    #[cfg(not(feature = "mojo"))]
    {
        runtime_profile_health_sort_key_by_key_rust(profile_name, health_entry, now, route_kind)
    }
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
    runtime_profile_effective_health_score_by_key_rust(health_entry, profile_name, now)
        .saturating_add(runtime_profile_effective_health_score_by_key_rust(
            health_entry,
            &runtime_profile_route_health_key(profile_name, route_kind),
            now,
        ))
        .saturating_add(runtime_profile_effective_score_by_key_rust(
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
    .unwrap_or(u32::MAX)
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_parity_tests {
    use super::*;

    #[test]
    fn health_sort_key_feature_path_matches_rust_oracle_for_all_routes() {
        let now = 100;
        let mut health = BTreeMap::from([(
            "alpha".to_string(),
            RuntimeProfileHealthSnapshot {
                score: 5,
                updated_at: now,
            },
        )]);
        let routes = [
            (RuntimeRouteKind::Responses, 3, 2, 8),
            (RuntimeRouteKind::Compact, 6, 2, 3),
            (RuntimeRouteKind::Websocket, 4, 1, 5),
            (RuntimeRouteKind::Standard, 7, 3, 9),
        ];
        for (route_kind, health_score, bad_pairing_score, performance_score) in routes {
            health.insert(
                runtime_profile_route_health_key("alpha", route_kind),
                RuntimeProfileHealthSnapshot {
                    score: health_score,
                    updated_at: now,
                },
            );
            health.insert(
                runtime_profile_route_bad_pairing_key("alpha", route_kind),
                RuntimeProfileHealthSnapshot {
                    score: bad_pairing_score,
                    updated_at: now,
                },
            );
            health.insert(
                runtime_profile_route_performance_key("alpha", route_kind),
                RuntimeProfileHealthSnapshot {
                    score: performance_score,
                    updated_at: now,
                },
            );
        }

        for (route_kind, _, _, _) in routes {
            assert_eq!(
                runtime_profile_health_sort_key("alpha", &health, now, route_kind),
                runtime_profile_health_sort_key_rust("alpha", &health, now, route_kind),
                "map route: {route_kind:?}"
            );
            let health_entry = |key: &str| health.get(key).copied();
            assert_eq!(
                runtime_profile_health_sort_key_by_key("alpha", health_entry, now, route_kind,),
                runtime_profile_health_sort_key_by_key_rust("alpha", health_entry, now, route_kind,),
                "callback route: {route_kind:?}"
            );
        }
    }

    #[test]
    fn scalar_health_paths_match_rust_oracles() {
        for score in [0_u32, 1, 5, 100, u32::MAX] {
            for updated_at in [-100_i64, 0, 50, 100, 200] {
                for now in [0_i64, 100, i64::MAX / 4] {
                    for decay in [0_i64, 1, 7, 60, 3_600] {
                        let entry = RuntimeProfileHealthSnapshot { score, updated_at };
                        assert_eq!(
                            runtime_profile_effective_score(&entry, now, decay),
                            runtime_profile_effective_score_rust(&entry, now, decay),
                            "score={score} updated={updated_at} now={now} decay={decay}"
                        );
                    }
                }
            }
        }

        let now = 1_000_i64;
        let mut health = BTreeMap::new();
        for (route, score, bad, perf) in [
            (RuntimeRouteKind::Responses, 9_u32, 3_u32, 12_u32),
            (RuntimeRouteKind::Websocket, 7, 5, 8),
            (RuntimeRouteKind::Compact, 4, 2, 11),
            (RuntimeRouteKind::Standard, 6, 1, 13),
        ] {
            health.insert(
                runtime_profile_route_health_key("p", route),
                RuntimeProfileHealthSnapshot {
                    score,
                    updated_at: 990,
                },
            );
            health.insert(
                runtime_profile_route_bad_pairing_key("p", route),
                RuntimeProfileHealthSnapshot {
                    score: bad,
                    updated_at: 980,
                },
            );
            health.insert(
                runtime_profile_route_performance_key("p", route),
                RuntimeProfileHealthSnapshot {
                    score: perf,
                    updated_at: 970,
                },
            );
        }
        for route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Standard,
        ] {
            assert_eq!(
                runtime_profile_route_coupling_score_from_map(&health, "p", now, route),
                runtime_profile_route_coupling_score_from_map_rust(&health, "p", now, route),
            );
            assert_eq!(
                runtime_profile_route_performance_score(&health, "p", now, route),
                runtime_profile_route_performance_score_rust(&health, "p", now, route),
            );
            let lookup = |key: &str| health.get(key).copied();
            assert_eq!(
                runtime_profile_route_coupling_score_by_key(lookup, "p", now, route),
                runtime_profile_route_coupling_score_by_key_rust(lookup, "p", now, route),
            );
            assert_eq!(
                runtime_profile_route_performance_score_by_key(lookup, "p", now, route),
                runtime_profile_route_performance_score_by_key_rust(lookup, "p", now, route),
            );
        }
    }
}
