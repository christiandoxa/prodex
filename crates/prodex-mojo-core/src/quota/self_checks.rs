use super::*;

pub fn self_test() -> bool {
    let gemini = gemini_bucket_numeric_batch(&[GeminiBucketNumericInput {
        remaining_amount: GeminiRemainingAmount::Parsed(50),
        remaining_fraction: Some(0.5),
    }])
    .is_ok_and(|outputs| {
        outputs
            == [GeminiBucketNumericOutput {
                remaining: Some(50),
                total: Some(100),
                remaining_percent: Some(50),
                exhausted: false,
            }]
    });
    let capacity = quota_capacity_batch(
        &[QuotaCapacityInput {
            lane: QUOTA_CAPACITY_LANE_MAIN,
            pair_allowed: None,
            outer_allowed: None,
            pair_limit_reached: None,
            outer_limit_reached: None,
            rate_limit_reached_type: QuotaAdmissionValue::Missing,
            camel_rate_limit_reached_type: QuotaAdmissionValue::Missing,
            spend_control_reached: QuotaAdmissionValue::Missing,
            camel_spend_control_reached: QuotaAdmissionValue::Missing,
            ordinary_usage_allowed: QuotaAdmissionValue::Missing,
            five_hour_used_percent: 10,
            five_hour_has_value: true,
            five_hour_reset_at: 0,
            weekly_used_percent: 20,
            weekly_has_value: true,
            weekly_reset_at: 0,
            primary_used_percent: 10,
            primary_has_value: true,
            secondary_used_percent: 20,
            secondary_has_value: true,
            scale_bps: 10_000,
            now: 0,
        }],
        3,
    )
    .is_ok_and(|outputs| {
        outputs.len() == 1
            && outputs[0].five_hour_remaining == 90
            && outputs[0].weekly_remaining == 80
            && outputs[0].usable
            && outputs[0].routing_eligible
    });
    let window_pressure = quota_window_pressure(58, 1_700_003_600, 1_700_000_000)
        .is_ok_and(|pressure| pressure == 62_068);
    remaining_percent(Some(42)) == 58
        && window_status(5, true) == 2
        && pressure_band(1, 2) == 2
        && round_f64(1.5) == 2
        && round_f64(-0.5) == -1
        && capacity
        && window_pressure
        && gemini
}

pub fn main_quota_aggregation_self_test() -> bool {
    main_quota_aggregate_batch(&[
        MainQuotaAggregationInput {
            remaining_percent: Some(80),
            reset_at: Some(20),
        },
        MainQuotaAggregationInput {
            remaining_percent: Some(30),
            reset_at: Some(10),
        },
    ])
    .is_ok_and(|result| {
        result.profiles_with_data == 2
            && result.pool_remaining == 110
            && result.earliest_reset_at == Some(10)
    })
}
