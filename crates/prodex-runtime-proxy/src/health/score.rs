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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
        .saturating_add(runtime_profile_route_coupling_score_by_key_rust(
            health_entry,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score_by_key_rust(
            health_entry,
            profile_name,
            now,
            route_kind,
        ))
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
mod mojo_parity_tests {
    use super::*;

    #[test]
    fn health_sort_key_map_adapter_matches_by_key_path_for_all_routes() {
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
            let health_entry = |key: &str| health.get(key).copied();
            assert_eq!(
                runtime_profile_health_sort_key("alpha", &health, now, route_kind),
                runtime_profile_health_sort_key_by_key("alpha", health_entry, now, route_kind),
                "map/by-key route: {route_kind:?}"
            );
            assert_eq!(
                runtime_profile_health_sort_key_by_key("alpha", health_entry, now, route_kind),
                runtime_profile_health_sort_key_by_key_rust("alpha", health_entry, now, route_kind),
                "Mojo/Rust-oracle route: {route_kind:?}"
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
            let lookup = |key: &str| health.get(key).copied();
            assert_eq!(
                runtime_profile_route_coupling_score_from_map(&health, "p", now, route),
                runtime_profile_route_coupling_score_by_key(lookup, "p", now, route),
            );
            assert_eq!(
                runtime_profile_route_performance_score(&health, "p", now, route),
                runtime_profile_route_performance_score_by_key(lookup, "p", now, route),
            );
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

    fn next_observation(seed: &mut u64) -> RuntimeProfileHealthSnapshot {
        *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let score = *seed as u32;
        *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        RuntimeProfileHealthSnapshot {
            score,
            updated_at: *seed as i64,
        }
    }

    fn assert_effective_score_boundaries(time_boundaries: [i64; 5]) {
        for score in [0_u32, u32::MAX] {
            for updated_at in time_boundaries {
                for now in time_boundaries {
                    for decay_seconds in time_boundaries {
                        let entry = RuntimeProfileHealthSnapshot { score, updated_at };
                        assert_eq!(
                            runtime_profile_effective_score(&entry, now, decay_seconds),
                            runtime_profile_effective_score_rust(&entry, now, decay_seconds),
                            "boundary score={score} updated={updated_at} now={now} decay={decay_seconds}"
                        );
                    }
                }
            }
        }
    }

    fn assert_random_effective_scores(seed: &mut u64) {
        for _ in 0..10_000 {
            let entry = next_observation(seed);
            *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let now = *seed as i64;
            *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let decay_seconds = *seed as i64;
            assert_eq!(
                runtime_profile_effective_score(&entry, now, decay_seconds),
                runtime_profile_effective_score_rust(&entry, now, decay_seconds),
                "score={} updated={} now={now} decay={decay_seconds}",
                entry.score,
                entry.updated_at
            );
        }
    }

    fn assert_route_parity(
        health: &BTreeMap<String, RuntimeProfileHealthSnapshot>,
        profile: &str,
        now: i64,
    ) {
        let lookup = |key: &str| health.get(key).copied();
        for route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Standard,
        ] {
            assert_eq!(
                runtime_profile_route_coupling_score_by_key(lookup, profile, now, route),
                runtime_profile_route_coupling_score_by_key_rust(lookup, profile, now, route),
                "coupling profile={profile} route={route:?}"
            );
            assert_eq!(
                runtime_profile_route_performance_score_by_key(lookup, profile, now, route),
                runtime_profile_route_performance_score_by_key_rust(lookup, profile, now, route),
                "performance profile={profile} route={route:?}"
            );
            assert_eq!(
                runtime_profile_health_sort_key_by_key(profile, lookup, now, route),
                runtime_profile_health_sort_key_by_key_rust(profile, lookup, now, route),
                "sort profile={profile} route={route:?}"
            );
        }
    }

    fn build_health_parity_fixture(
        seed: &mut u64,
    ) -> BTreeMap<String, RuntimeProfileHealthSnapshot> {
        let mut health = BTreeMap::new();
        for index in 0..128 {
            let profile = format!("profile-{index}");
            health.insert(profile.clone(), next_observation(seed));
            for route in [
                RuntimeRouteKind::Responses,
                RuntimeRouteKind::Compact,
                RuntimeRouteKind::Websocket,
                RuntimeRouteKind::Standard,
            ] {
                health.insert(
                    runtime_profile_route_health_key(&profile, route),
                    next_observation(seed),
                );
                health.insert(
                    runtime_profile_route_bad_pairing_key(&profile, route),
                    next_observation(seed),
                );
                health.insert(
                    runtime_profile_route_performance_key(&profile, route),
                    next_observation(seed),
                );
            }
            *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            assert_route_parity(&health, &profile, *seed as i64);
        }
        health
    }

    #[test]
    fn health_scores_match_rust_oracles_over_full_width_inputs() {
        let mut seed = 0x6a09_e667_f3bc_c909_u64;
        let time_boundaries = [i64::MIN, -1, 0, 1, i64::MAX];
        assert_effective_score_boundaries(time_boundaries);
        assert_random_effective_scores(&mut seed);
        let health = build_health_parity_fixture(&mut seed);
        for now in time_boundaries {
            assert_route_parity(&health, "profile-0", now);
        }
    }
}
