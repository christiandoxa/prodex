#![cfg(feature = "mojo-quota")]

use prodex_mojo_core::quota::{
    QUOTA_CAPACITY_LANE_MAIN, QUOTA_CAPACITY_LANE_SPARK, QuotaCapacityInput, QuotaCapacityOutput,
    quota_capacity_batch,
};

fn remaining(used_percent: i64, has_value: bool) -> i64 {
    if !has_value {
        return 0;
    }
    if used_percent < 0 {
        100
    } else if used_percent > 100 {
        0
    } else {
        100 - used_percent
    }
}

fn window_status(remaining_percent: i64, has_value: bool) -> i64 {
    if !has_value {
        4
    } else if remaining_percent == 0 {
        3
    } else if remaining_percent <= 5 {
        2
    } else if remaining_percent <= 15 {
        1
    } else {
        0
    }
}

fn pressure_band(
    five_hour_remaining: i64,
    five_hour_has_value: bool,
    weekly_remaining: i64,
    weekly_has_value: bool,
    route_kind: i64,
) -> i64 {
    if !five_hour_has_value && !weekly_has_value {
        return 4;
    }
    if (five_hour_has_value && five_hour_remaining == 0)
        || (weekly_has_value && weekly_remaining == 0)
    {
        return 3;
    }
    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) =
        if route_kind == 0 || route_kind == 2 {
            (20, 10, 10, 5)
        } else {
            (10, 5, 5, 3)
        };
    let weekly_band = if !weekly_has_value {
        0
    } else if weekly_remaining <= critical_weekly {
        2
    } else if weekly_remaining <= thin_weekly {
        1
    } else {
        0
    };
    let five_hour_band = if !five_hour_has_value {
        0
    } else if five_hour_remaining <= critical_five_hour {
        2
    } else if five_hour_remaining <= thin_five_hour {
        1
    } else {
        0
    };
    weekly_band.max(five_hour_band)
}

fn pressure(seconds_until_reset: i64, remaining_percent: i64, has_value: bool) -> i64 {
    if !has_value {
        return i64::MAX;
    }
    seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX)
}

fn scale(value: i64, scale_bps: i64) -> i64 {
    if value == i64::MAX {
        return i64::MAX;
    }
    value
        .saturating_mul(scale_bps)
        .checked_div(10_000)
        .unwrap_or(i64::MAX)
}

fn oracle(input: QuotaCapacityInput, route_kind: i64) -> QuotaCapacityOutput {
    let five_hour_remaining = remaining(input.five_hour_used_percent, input.five_hour_has_value);
    let weekly_remaining = remaining(input.weekly_used_percent, input.weekly_has_value);
    let five_hour_status = window_status(five_hour_remaining, input.five_hour_has_value);
    let weekly_status = window_status(weekly_remaining, input.weekly_has_value);
    let five_hour_pressure = pressure(
        input.five_hour_seconds_until_reset,
        five_hour_remaining,
        input.five_hour_has_value,
    );
    let weekly_pressure = pressure(
        input.weekly_seconds_until_reset,
        weekly_remaining,
        input.weekly_has_value,
    );
    let pressure_band = pressure_band(
        five_hour_remaining,
        input.five_hour_has_value,
        weekly_remaining,
        input.weekly_has_value,
        route_kind,
    );
    let admission_allowed = input.allowed != 2 && input.limit_reached != 2;
    let pair_ready = (input.five_hour_has_value || input.weekly_has_value)
        && (!input.five_hour_has_value || five_hour_remaining > 0)
        && (!input.weekly_has_value || weekly_remaining > 0);
    let usable = admission_allowed && pair_ready;
    let routing_eligible = usable
        && matches!(
            input.lane,
            QUOTA_CAPACITY_LANE_MAIN | QUOTA_CAPACITY_LANE_SPARK
        );
    let reserve_bias = match pressure_band {
        0 => 0,
        1 => 250_000,
        2 => 1_000_000,
        3 | 4 => i64::MAX / 4,
        _ => unreachable!(),
    };
    let raw_total = reserve_bias
        .saturating_add(weekly_pressure.saturating_mul(input.weekly_weight))
        .saturating_add(five_hour_pressure);
    QuotaCapacityOutput {
        lane: input.lane,
        five_hour_remaining,
        weekly_remaining,
        five_hour_status,
        weekly_status,
        pressure_band,
        admission_allowed,
        pair_ready,
        usable,
        routing_eligible,
        reserve_floor: five_hour_remaining.min(weekly_remaining),
        five_hour_pressure: scale(five_hour_pressure, input.scale_bps),
        weekly_pressure: scale(weekly_pressure, input.scale_bps),
        total_pressure: scale(raw_total, input.scale_bps),
    }
}

fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

#[test]
fn quota_capacity_batch_matches_rust_oracle_for_seeded_rows() {
    let mut state = 0x71756174615f6361_u64;
    let used_values = [-1, 0, 1, 5, 6, 10, 15, 16, 50, 100, 101];
    let mut inputs = Vec::new();
    for index in 0..224 {
        let used = |state: &mut u64| used_values[(next(state) % used_values.len() as u64) as usize];
        inputs.push(QuotaCapacityInput {
            lane: (next(&mut state) % 3) as i64,
            allowed: (next(&mut state) % 3) as i64,
            limit_reached: (next(&mut state) % 3) as i64,
            five_hour_used_percent: used(&mut state),
            five_hour_has_value: next(&mut state) & 1 != 0,
            five_hour_seconds_until_reset: if index % 17 == 0 {
                i64::MAX
            } else {
                (next(&mut state) % 1_000_000) as i64
            },
            weekly_used_percent: used(&mut state),
            weekly_has_value: next(&mut state) & 1 != 0,
            weekly_seconds_until_reset: if index % 19 == 0 {
                i64::MAX
            } else {
                (next(&mut state) % 1_000_000) as i64
            },
            scale_bps: [0, 2_000, 5_000, 10_000, 20_000][(next(&mut state) % 5) as usize],
            weekly_weight: (next(&mut state) % 11) as i64,
        });
    }

    for route_kind in 0..4 {
        let expected = inputs
            .iter()
            .copied()
            .map(|input| oracle(input, route_kind))
            .collect::<Vec<_>>();
        assert_eq!(
            quota_capacity_batch(&inputs, route_kind).expect("valid capacity batch"),
            expected,
            "route_kind={route_kind}"
        );
    }
}

#[test]
fn quota_capacity_batch_rejects_oversized_and_invalid_rows() {
    let input = QuotaCapacityInput {
        lane: 0,
        allowed: 0,
        limit_reached: 0,
        five_hour_used_percent: 0,
        five_hour_has_value: true,
        five_hour_seconds_until_reset: 0,
        weekly_used_percent: 0,
        weekly_has_value: true,
        weekly_seconds_until_reset: 0,
        scale_bps: 10_000,
        weekly_weight: 10,
    };
    assert!(quota_capacity_batch(&[input; 257], 0).is_err());
    assert!(quota_capacity_batch(&[input], 4).is_err());
    assert!(quota_capacity_batch(&[QuotaCapacityInput { lane: 7, ..input }], 0).is_err());
    assert!(quota_capacity_batch(&[], 0).unwrap().is_empty());
}
