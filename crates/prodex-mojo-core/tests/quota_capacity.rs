#![cfg(feature = "mojo-quota")]

use prodex_mojo_core::quota::{
    QUOTA_CAPACITY_LANE_MAIN, QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL, QuotaAdmissionValue,
    QuotaCapacityInput, QuotaCapacityOutput, quota_capacity_batch,
};

fn ready_capacity_input() -> QuotaCapacityInput {
    QuotaCapacityInput {
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
        five_hour_reset_at: 1_700_003_600,
        weekly_used_percent: 20,
        weekly_has_value: true,
        weekly_reset_at: 1_700_086_400,
        primary_used_percent: 10,
        primary_has_value: true,
        secondary_used_percent: 20,
        secondary_has_value: true,
        scale_bps: 10_000,
        now: 1_700_000_000,
    }
}

#[test]
fn quota_capacity_returns_expected_window_metrics() {
    assert_eq!(
        quota_capacity_batch(&[ready_capacity_input()], 0).expect("valid quota capacity"),
        [QuotaCapacityOutput {
            lane: QUOTA_CAPACITY_LANE_MAIN,
            five_hour_remaining: 90,
            weekly_remaining: 80,
            five_hour_status: 0,
            weekly_status: 0,
            pressure_band: 0,
            admission_allowed: true,
            pair_ready: true,
            any_window_exhausted: false,
            usable: true,
            routing_eligible: true,
            reserve_floor: 80,
            five_hour_pressure: 40_000,
            weekly_pressure: 1_080_000,
            total_pressure: 10_840_000,
        }]
    );
}

#[test]
fn quota_capacity_applies_admission_and_lane_rules() {
    let base = ready_capacity_input();
    let inputs = [
        base,
        QuotaCapacityInput {
            pair_allowed: Some(false),
            ..base
        },
        QuotaCapacityInput {
            outer_limit_reached: Some(true),
            ..base
        },
        QuotaCapacityInput {
            rate_limit_reached_type: QuotaAdmissionValue::Other,
            ..base
        },
        QuotaCapacityInput {
            spend_control_reached: QuotaAdmissionValue::True,
            ..base
        },
        QuotaCapacityInput {
            ordinary_usage_allowed: QuotaAdmissionValue::Null,
            ..base
        },
        QuotaCapacityInput {
            lane: QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL,
            ..base
        },
        QuotaCapacityInput {
            five_hour_has_value: false,
            weekly_has_value: false,
            ..base
        },
        QuotaCapacityInput {
            five_hour_used_percent: 100,
            primary_used_percent: 100,
            ..base
        },
    ];
    let expected = [
        (true, true, true, true),
        (false, true, false, false),
        (false, true, false, false),
        (false, true, false, false),
        (false, true, false, false),
        (false, true, false, false),
        (true, true, true, false),
        (true, false, false, false),
        (true, false, false, false),
    ];
    let outputs = quota_capacity_batch(&inputs, 0).expect("valid quota capacity rows");
    assert_eq!(
        outputs
            .iter()
            .map(|output| (
                output.admission_allowed,
                output.pair_ready,
                output.usable,
                output.routing_eligible,
            ))
            .collect::<Vec<_>>(),
        expected
    );
    assert!(outputs[8].any_window_exhausted);
}

#[test]
fn quota_capacity_preserves_sentinel_pressure_and_rejects_invalid_batches() {
    let input = QuotaCapacityInput {
        five_hour_reset_at: i64::MAX,
        weekly_reset_at: 0,
        now: 0,
        ..ready_capacity_input()
    };
    let output = quota_capacity_batch(&[input], 0).expect("valid sentinel reset")[0];
    assert_eq!(output.five_hour_pressure, 922_337_203_685_477);
    assert_eq!(output.weekly_pressure, 0);
    assert_eq!(output.total_pressure, 922_337_203_685_477);

    assert!(quota_capacity_batch(&[input; 257], 0).is_err());
    assert!(quota_capacity_batch(&[input], 4).is_err());
    assert!(quota_capacity_batch(&[QuotaCapacityInput { lane: 7, ..input }], 0).is_err());
    assert!(quota_capacity_batch(&[], 0).unwrap().is_empty());
}
