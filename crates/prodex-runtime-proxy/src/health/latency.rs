use crate::RuntimeRouteKind;

use super::RUNTIME_PROFILE_LATENCY_PENALTY_MAX;

pub fn runtime_profile_latency_penalty(
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let route_kind = match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    };
    let stage_kind = match stage {
        "ttfb" => 1,
        "connect" => 2,
        _ => 0,
    };
    prodex_mojo_core::runtime::profile_latency_penalty(
        elapsed_ms,
        route_kind,
        stage_kind,
        RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
    )
    .unwrap_or_else(|error| panic!("Mojo latency penalty failed: {error:?}"))
}

pub fn runtime_profile_latency_observation_next_score(
    current_score: u32,
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let observed = runtime_profile_latency_penalty(elapsed_ms, route_kind, stage);
    prodex_mojo_core::runtime::profile_latency_next_score(current_score, observed)
        .unwrap_or_else(|error| panic!("Mojo latency next score failed: {error:?}"))
}

pub fn runtime_profile_latency_failure_next_score(current_score: u32) -> u32 {
    prodex_mojo_core::runtime::profile_latency_failure_score(
        current_score,
        crate::RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
        RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
    )
    .unwrap_or_else(|error| panic!("Mojo latency failure score failed: {error:?}"))
}

// Test-only pre-migration Rust oracle retained for direct Mojo parity checks.

#[cfg(test)]
fn runtime_profile_latency_penalty_rust(
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let (good_ms, warn_ms, poor_ms, severe_ms) = match (route_kind, stage) {
        (RuntimeRouteKind::Responses, "ttfb") | (RuntimeRouteKind::Websocket, "connect") => {
            (120, 300, 700, 1_500)
        }
        (RuntimeRouteKind::Compact, _) | (RuntimeRouteKind::Standard, _) => (80, 180, 400, 900),
        _ => (100, 250, 600, 1_200),
    };
    match elapsed_ms {
        elapsed if elapsed <= good_ms => 0,
        elapsed if elapsed <= warn_ms => 2,
        elapsed if elapsed <= poor_ms => 4,
        elapsed if elapsed <= severe_ms => 7,
        _ => RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
    }
}

#[cfg(test)]
fn runtime_profile_latency_observation_next_score_rust(current_score: u32, observed: u32) -> u32 {
    if observed == 0 {
        current_score.saturating_sub(2)
    } else {
        (((current_score as u64) * 2) + (observed as u64)).div_ceil(3) as u32
    }
}

#[cfg(test)]
fn runtime_profile_latency_failure_next_score_rust(current_score: u32) -> u32 {
    current_score
        .saturating_add(crate::RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY)
        .min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX)
}

#[cfg(test)]
mod mojo_parity_tests {
    use super::*;

    #[test]
    fn latency_policy_matches_rust_oracle() {
        for route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Standard,
        ] {
            for stage in ["ttfb", "connect", "other"] {
                for elapsed in [0_u64, 80, 120, 181, 300, 700, 1_500, 10_000] {
                    let observed = runtime_profile_latency_penalty_rust(elapsed, route, stage);
                    assert_eq!(
                        runtime_profile_latency_penalty(elapsed, route, stage),
                        observed,
                    );
                    for current in [0_u32, 1, 5, RUNTIME_PROFILE_LATENCY_PENALTY_MAX] {
                        assert_eq!(
                            runtime_profile_latency_observation_next_score(
                                current, elapsed, route, stage,
                            ),
                            runtime_profile_latency_observation_next_score_rust(current, observed),
                        );
                    }
                }
            }
        }
        for current in [0_u32, 1, 5, RUNTIME_PROFILE_LATENCY_PENALTY_MAX, u32::MAX] {
            assert_eq!(
                runtime_profile_latency_failure_next_score(current),
                runtime_profile_latency_failure_next_score_rust(current),
            );
        }
    }

    #[test]
    fn latency_matches_rust_oracle_for_full_u64_corpus_and_threshold_edges() {
        let mut seed = 0x3c6e_f372_fe94_f82b_u64;
        let mut elapsed_values = vec![
            0,
            79,
            80,
            81,
            119,
            120,
            121,
            179,
            180,
            181,
            249,
            250,
            251,
            299,
            300,
            301,
            399,
            400,
            401,
            599,
            600,
            601,
            699,
            700,
            701,
            899,
            900,
            901,
            1_199,
            1_200,
            1_201,
            1_499,
            1_500,
            1_501,
            u64::MAX,
        ];
        for _ in 0..10_000 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            elapsed_values.push(seed);
        }
        for route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Standard,
        ] {
            for stage in ["ttfb", "connect", "other"] {
                for elapsed in &elapsed_values {
                    let observed = runtime_profile_latency_penalty_rust(*elapsed, route, stage);
                    assert_eq!(
                        runtime_profile_latency_penalty(*elapsed, route, stage),
                        observed,
                        "elapsed={elapsed} route={route:?} stage={stage}"
                    );
                }
            }
        }
    }
}
