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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_ttfb_penalty_edges_and_score_updates_are_stable() {
        for (elapsed_ms, expected) in [
            (120, 0),
            (121, 2),
            (300, 2),
            (301, 4),
            (700, 4),
            (701, 7),
            (1_500, 7),
            (1_501, RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
            (u64::MAX, RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
        ] {
            assert_eq!(
                runtime_profile_latency_penalty(elapsed_ms, RuntimeRouteKind::Responses, "ttfb"),
                expected,
                "elapsed_ms={elapsed_ms}"
            );
        }

        assert_eq!(
            runtime_profile_latency_observation_next_score(
                5,
                120,
                RuntimeRouteKind::Responses,
                "ttfb"
            ),
            3,
        );
        assert_eq!(
            runtime_profile_latency_observation_next_score(
                1,
                701,
                RuntimeRouteKind::Responses,
                "ttfb"
            ),
            3,
        );
        assert_eq!(
            runtime_profile_latency_failure_next_score(u32::MAX),
            RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
        );
    }
}
