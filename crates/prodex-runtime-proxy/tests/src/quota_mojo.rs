use super::*;

#[cfg(all(feature = "mojo", prodex_mojo_required))]
#[test]
fn mojo_feature_requires_real_compiled_core() {
    #[cfg(not(prodex_mojo_active))]
    panic!("Mojo feature unexpectedly built without a real Mojo archive");
    #[cfg(prodex_mojo_fallback)]
    panic!("Mojo feature unexpectedly built with the Rust fallback");
}

fn window(remaining_percent: i64) -> RuntimeProxyQuotaWindowObservation {
    RuntimeProxyQuotaWindowObservation {
        remaining_percent,
        reset_at: 3_600,
        pressure_score: 3_600_000 / remaining_percent.max(1),
    }
}

#[test]
fn route_pressure_band_matches_rust_oracle_for_all_routes_and_boundaries() {
    let routes = [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard,
    ];
    let values = [
        None,
        Some(-1),
        Some(0),
        Some(3),
        Some(5),
        Some(10),
        Some(20),
        Some(100),
    ];

    for route in routes {
        for five_hour in values {
            for weekly in values {
                assert_eq!(
                    runtime_proxy_quota_pressure_band_for_route(
                        five_hour.map(window),
                        weekly.map(window),
                        route,
                    ),
                    rust_route_pressure_band(five_hour, weekly, route),
                    "route={route:?} five_hour={five_hour:?} weekly={weekly:?}",
                );
            }
        }
    }
}

fn rust_route_pressure_band(
    five_hour: Option<i64>,
    weekly: Option<i64>,
    route: RuntimeRouteKind,
) -> RuntimeSelectionQuotaPressureBand {
    if five_hour.is_none() && weekly.is_none() {
        return RuntimeSelectionQuotaPressureBand::Unknown;
    }
    if five_hour == Some(0) || weekly == Some(0) {
        return RuntimeSelectionQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };
    if weekly.is_some_and(|value| value <= critical_weekly)
        || five_hour.is_some_and(|value| value <= critical_five_hour)
    {
        RuntimeSelectionQuotaPressureBand::Critical
    } else if weekly.is_some_and(|value| value <= thin_weekly)
        || five_hour.is_some_and(|value| value <= thin_five_hour)
    {
        RuntimeSelectionQuotaPressureBand::Thin
    } else {
        RuntimeSelectionQuotaPressureBand::Healthy
    }
}
