use crate::{
    RuntimeProxyQuotaObservationPair, RuntimeProxyQuotaScore, RuntimeProxyQuotaWindowObservation,
    RuntimeRouteKind, RuntimeSelectionQuotaPressureBand,
};

pub(super) fn route_kind_tag(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    }
}

pub(super) fn quota_band_from_tag(
    band: i64,
) -> Result<RuntimeSelectionQuotaPressureBand, prodex_mojo_core::MojoError> {
    match band {
        0 => Ok(RuntimeSelectionQuotaPressureBand::Healthy),
        1 => Ok(RuntimeSelectionQuotaPressureBand::Thin),
        2 => Ok(RuntimeSelectionQuotaPressureBand::Critical),
        3 => Ok(RuntimeSelectionQuotaPressureBand::Exhausted),
        4 => Ok(RuntimeSelectionQuotaPressureBand::Unknown),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

pub(crate) fn pressure_band_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> Result<RuntimeSelectionQuotaPressureBand, prodex_mojo_core::MojoError> {
    let five_hour = five_hour.map(|window| (window.remaining_percent, 1));
    let weekly = weekly.map(|window| (window.remaining_percent, 1));
    quota_band_from_tag(prodex_mojo_core::runtime::pressure_band_for_route(
        five_hour,
        weekly,
        route_kind_tag(route_kind),
    )?)
}

pub(crate) fn quota_score_batch(
    observations: &[RuntimeProxyQuotaObservationPair],
    route_kind: RuntimeRouteKind,
) -> Result<Vec<RuntimeProxyQuotaScore>, prodex_mojo_core::MojoError> {
    let inputs = observations
        .iter()
        .map(|(five_hour, weekly)| {
            let five_hour = five_hour.as_ref();
            let weekly = weekly.as_ref();
            prodex_mojo_core::runtime::QuotaScoreInput {
                weekly_pressure: weekly.map_or(i64::MAX, |window| window.pressure_score),
                five_hour_pressure: five_hour.map_or(i64::MAX, |window| window.pressure_score),
                weekly_remaining: weekly.map_or(0, |window| window.remaining_percent),
                five_hour_remaining: five_hour.map_or(0, |window| window.remaining_percent),
                weekly_has_value: weekly.is_some(),
                five_hour_has_value: five_hour.is_some(),
                weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
                five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
            }
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::runtime::quota_score_batch(&inputs, route_kind_tag(route_kind))?
        .into_iter()
        .map(|score| {
            Ok(RuntimeProxyQuotaScore {
                pressure_band: quota_band_from_tag(score.pressure_band)?,
                total_pressure: score.total_pressure,
                weekly_pressure: score.weekly_pressure,
                five_hour_pressure: score.five_hour_pressure,
                reserve_floor: score.reserve_floor,
                weekly_remaining: score.weekly_remaining,
                five_hour_remaining: score.five_hour_remaining,
                weekly_reset_at: score.weekly_reset_at,
                five_hour_reset_at: score.five_hour_reset_at,
            })
        })
        .collect()
}
