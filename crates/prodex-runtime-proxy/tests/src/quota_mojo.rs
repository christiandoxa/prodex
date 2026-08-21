use super::*;

#[cfg(feature = "mojo")]
#[test]
fn mojo_feature_requires_real_compiled_core() {
    if prodex_mojo_core::MOJO_REQUIRED && !prodex_mojo_core::MOJO_ACTIVE {
        panic!("strict Mojo mode did not activate the compiled Mojo core");
    }
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

#[test]
fn quota_window_summary_uses_the_compiled_status_kernel() {
    let cases = [
        (0, RuntimeSelectionQuotaWindowStatus::Exhausted),
        (5, RuntimeSelectionQuotaWindowStatus::Critical),
        (6, RuntimeSelectionQuotaWindowStatus::Thin),
        (15, RuntimeSelectionQuotaWindowStatus::Thin),
        (16, RuntimeSelectionQuotaWindowStatus::Ready),
    ];
    for (remaining_percent, expected) in cases {
        assert_eq!(
            runtime_proxy_quota_window_summary(Some(window(remaining_percent))).status,
            expected,
            "remaining_percent={remaining_percent}",
        );
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

#[test]
fn smart_context_byte_estimate_matches_rust_oracle_at_boundaries() {
    for body_bytes in [0, 1, 3, 4, 5, 80_001, usize::MAX] {
        let expected = u64::try_from(body_bytes)
            .unwrap_or(u64::MAX)
            .saturating_add(3)
            / 4;
        assert_eq!(
            crate::smart_context_estimate_tokens_from_body_bytes(body_bytes),
            expected
        );
    }
}
