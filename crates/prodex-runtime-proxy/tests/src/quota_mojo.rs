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
fn route_pressure_band_matches_expected_boundaries() {
    use RuntimeRouteKind as Route;
    use RuntimeSelectionQuotaPressureBand as Band;

    let cases = [
        (Route::Standard, None, None, Band::Unknown),
        (Route::Responses, None, Some(100), Band::Healthy),
        (Route::Responses, Some(0), Some(5), Band::Exhausted),
        (Route::Responses, Some(5), None, Band::Critical),
        (Route::Responses, Some(6), None, Band::Thin),
        (Route::Responses, Some(10), None, Band::Thin),
        (Route::Responses, Some(11), None, Band::Healthy),
        (Route::Responses, None, Some(10), Band::Critical),
        (Route::Responses, None, Some(11), Band::Thin),
        (Route::Responses, None, Some(20), Band::Thin),
        (Route::Responses, None, Some(21), Band::Healthy),
        (Route::Websocket, Some(10), None, Band::Thin),
        (Route::Websocket, None, Some(10), Band::Critical),
        (Route::Compact, Some(3), None, Band::Critical),
        (Route::Compact, Some(4), None, Band::Thin),
        (Route::Compact, Some(5), None, Band::Thin),
        (Route::Compact, Some(6), None, Band::Healthy),
        (Route::Compact, None, Some(5), Band::Critical),
        (Route::Compact, None, Some(6), Band::Thin),
        (Route::Compact, None, Some(10), Band::Thin),
        (Route::Compact, None, Some(11), Band::Healthy),
        (Route::Standard, Some(-1), None, Band::Critical),
        (Route::Standard, None, Some(5), Band::Critical),
    ];

    for (route, five_hour, weekly, expected) in cases {
        assert_eq!(
            runtime_proxy_quota_pressure_band_for_route(
                five_hour.map(window),
                weekly.map(window),
                route,
            ),
            expected,
            "route={route:?} five_hour={five_hour:?} weekly={weekly:?}",
        );
    }
}

#[test]
fn quota_score_matches_expected_route_weight_and_reserve_bias() {
    let healthy_five_hour = window(60);
    let critical_five_hour = window(5);
    let weekly = window(80);
    let expected_healthy = RuntimeProxyQuotaScore {
        pressure_band: RuntimeSelectionQuotaPressureBand::Healthy,
        total_pressure: 510_000,
        weekly_pressure: 45_000,
        five_hour_pressure: 60_000,
        reserve_floor: 60,
        weekly_remaining: 80,
        five_hour_remaining: 60,
        weekly_reset_at: 3_600,
        five_hour_reset_at: 3_600,
    };
    let expected_critical_responses = RuntimeProxyQuotaScore {
        pressure_band: RuntimeSelectionQuotaPressureBand::Critical,
        total_pressure: 2_170_000,
        weekly_pressure: 45_000,
        five_hour_pressure: 720_000,
        reserve_floor: 5,
        weekly_remaining: 80,
        five_hour_remaining: 5,
        weekly_reset_at: 3_600,
        five_hour_reset_at: 3_600,
    };
    let expected_thin_compact = RuntimeProxyQuotaScore {
        pressure_band: RuntimeSelectionQuotaPressureBand::Thin,
        total_pressure: 1_330_000,
        ..expected_critical_responses
    };
    let expected_unknown = RuntimeProxyQuotaScore {
        pressure_band: RuntimeSelectionQuotaPressureBand::Unknown,
        total_pressure: i64::MAX,
        weekly_pressure: i64::MAX,
        five_hour_pressure: i64::MAX,
        reserve_floor: 0,
        weekly_remaining: 0,
        five_hour_remaining: 0,
        weekly_reset_at: i64::MAX,
        five_hour_reset_at: i64::MAX,
    };

    assert_eq!(
        runtime_proxy_quota_scores_for_route_batch(
            &[
                (Some(healthy_five_hour), Some(weekly)),
                (Some(critical_five_hour), Some(weekly)),
                (None, None),
            ],
            RuntimeRouteKind::Responses,
        ),
        vec![
            expected_healthy,
            expected_critical_responses,
            expected_unknown
        ],
    );
    assert_eq!(
        runtime_proxy_quota_score_for_route(
            Some(critical_five_hour),
            Some(weekly),
            RuntimeRouteKind::Compact,
        ),
        expected_thin_compact,
    );
    assert!(
        runtime_proxy_quota_scores_for_route_batch(&[], RuntimeRouteKind::Responses).is_empty()
    );
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
