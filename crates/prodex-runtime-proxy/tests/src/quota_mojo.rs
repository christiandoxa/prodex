use super::*;

#[cfg(feature = "mojo")]
#[test]
fn mojo_feature_requires_real_compiled_core() {
    if prodex_mojo_core::MOJO_REQUIRED && !prodex_mojo_core::MOJO_ACTIVE {
        panic!("strict Mojo mode unexpectedly activated the Rust fallback");
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
fn profile_score_batch_matches_the_rust_arithmetic_oracle() {
    let inputs = [
        RuntimeProxyQuotaProfileScoreInput {
            weekly_pressure: 10_000,
            five_hour_pressure: 20_000,
            scale_bps: 2_000,
            weekly_remaining: 80,
            five_hour_remaining: 40,
            reserve_bias: 0,
            weekly_weight: 10,
        },
        RuntimeProxyQuotaProfileScoreInput {
            weekly_pressure: i64::MAX,
            five_hour_pressure: 7,
            scale_bps: 5_000,
            weekly_remaining: 0,
            five_hour_remaining: 5,
            reserve_bias: 1_000_000,
            weekly_weight: 8,
        },
        RuntimeProxyQuotaProfileScoreInput {
            weekly_pressure: 1,
            five_hour_pressure: 2,
            scale_bps: 12_000,
            weekly_remaining: 100,
            five_hour_remaining: 99,
            reserve_bias: i64::MAX / 4,
            weekly_weight: 10,
        },
    ];

    let actual = runtime_proxy_quota_profile_scores_batch(&inputs);
    let expected = inputs.map(|input| {
        let scale = |pressure: i64| {
            if pressure == i64::MAX {
                return i64::MAX;
            }
            pressure
                .saturating_mul(input.scale_bps.max(0))
                .checked_div(10_000)
                .unwrap_or(i64::MAX)
        };
        let weekly_pressure = scale(input.weekly_pressure);
        let five_hour_pressure = scale(input.five_hour_pressure);
        RuntimeProxyQuotaProfileScore {
            total_pressure: input
                .reserve_bias
                .saturating_add(weekly_pressure.saturating_mul(input.weekly_weight))
                .saturating_add(five_hour_pressure),
            weekly_pressure,
            five_hour_pressure,
            reserve_floor: input.weekly_remaining.min(input.five_hour_remaining),
        }
    });
    assert_eq!(actual, expected);
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
