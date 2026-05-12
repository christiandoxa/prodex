use crate::RuntimeRouteKind;

use super::RUNTIME_PROFILE_LATENCY_PENALTY_MAX;

pub fn runtime_profile_latency_penalty(
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

pub fn runtime_profile_latency_observation_next_score(
    current_score: u32,
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let observed = runtime_profile_latency_penalty(elapsed_ms, route_kind, stage);
    if observed == 0 {
        current_score.saturating_sub(2)
    } else {
        (((current_score as u64) * 2) + (observed as u64)).div_ceil(3) as u32
    }
}

pub fn runtime_profile_latency_failure_next_score(current_score: u32) -> u32 {
    current_score
        .saturating_add(crate::RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY)
        .min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX)
}
