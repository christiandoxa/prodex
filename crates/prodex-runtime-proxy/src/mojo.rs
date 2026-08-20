use crate::{
    RuntimeProxyQuotaProfileScore, RuntimeProxyQuotaProfileScoreInput,
    RuntimeProxyQuotaWindowObservation, RuntimeRouteKind, RuntimeSelectionQuotaPressureBand,
};

pub(crate) fn pressure_band_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> RuntimeSelectionQuotaPressureBand {
    let five_hour = five_hour.map(|window| (window.remaining_percent, 1));
    let weekly = weekly.map(|window| (window.remaining_percent, 1));
    let route_kind = match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    };
    match prodex_mojo_core::runtime::pressure_band_for_route(five_hour, weekly, route_kind) {
        0 => RuntimeSelectionQuotaPressureBand::Healthy,
        1 => RuntimeSelectionQuotaPressureBand::Thin,
        2 => RuntimeSelectionQuotaPressureBand::Critical,
        3 => RuntimeSelectionQuotaPressureBand::Exhausted,
        _ => RuntimeSelectionQuotaPressureBand::Unknown,
    }
}

pub(crate) fn profile_scores_batch(
    inputs: &[RuntimeProxyQuotaProfileScoreInput],
) -> Vec<RuntimeProxyQuotaProfileScore> {
    let inputs = inputs
        .iter()
        .map(|input| prodex_mojo_core::runtime::ProfileScoreInput {
            weekly_pressure: input.weekly_pressure,
            five_hour_pressure: input.five_hour_pressure,
            scale_bps: input.scale_bps,
            weekly_remaining: input.weekly_remaining,
            five_hour_remaining: input.five_hour_remaining,
            reserve_bias: input.reserve_bias,
            weekly_weight: input.weekly_weight,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::runtime::profile_scores_batch(&inputs)
        .into_iter()
        .map(|score| RuntimeProxyQuotaProfileScore {
            total_pressure: score.total_pressure,
            weekly_pressure: score.weekly_pressure,
            five_hour_pressure: score.five_hour_pressure,
            reserve_floor: score.reserve_floor,
        })
        .collect()
}

pub(crate) fn smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64 {
    prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body_bytes(body_bytes)
}
