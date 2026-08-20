use super::RuntimeProxyQuotaWindowObservation;
use crate::{RuntimeRouteKind, RuntimeSelectionQuotaPressureBand};

unsafe extern "C" {
    fn prodex_runtime_quota_pressure_band_for_route(
        five_hour_remaining_percent: i64,
        five_hour_has_value: i64,
        weekly_remaining_percent: i64,
        weekly_has_value: i64,
        route_kind: i64,
    ) -> i64;
}

pub(super) fn pressure_band_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> RuntimeSelectionQuotaPressureBand {
    let (five_hour_remaining_percent, five_hour_has_value) =
        five_hour.map_or((0, 0), |window| (window.remaining_percent, 1));
    let (weekly_remaining_percent, weekly_has_value) =
        weekly.map_or((0, 0), |window| (window.remaining_percent, 1));
    let route_kind = match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    };

    // SAFETY: build.rs links the stateless scalar-only Mojo C-ABI object.
    let code = unsafe {
        prodex_runtime_quota_pressure_band_for_route(
            five_hour_remaining_percent,
            five_hour_has_value,
            weekly_remaining_percent,
            weekly_has_value,
            route_kind,
        )
    };
    match code {
        0 => RuntimeSelectionQuotaPressureBand::Healthy,
        1 => RuntimeSelectionQuotaPressureBand::Thin,
        2 => RuntimeSelectionQuotaPressureBand::Critical,
        3 => RuntimeSelectionQuotaPressureBand::Exhausted,
        _ => RuntimeSelectionQuotaPressureBand::Unknown,
    }
}
