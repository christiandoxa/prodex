//! Checked caller-owned quota capacity ABI adapter.

use super::{
    QUOTA_CAPACITY_BATCH_MAX_COUNT, QUOTA_CAPACITY_FIELD_COUNT, QUOTA_CAPACITY_LANE_MAIN,
    QUOTA_CAPACITY_LANE_MODEL_SPECIFIC, QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL,
    QuotaAdmissionValue, QuotaCapacityInput, QuotaCapacityOutput, prodex_quota_capacity_batch_v2,
};

pub fn quota_capacity_batch(
    inputs: &[QuotaCapacityInput],
    route_kind: i64,
) -> Result<Vec<QuotaCapacityOutput>, crate::MojoError> {
    if inputs.len() > QUOTA_CAPACITY_BATCH_MAX_COUNT
        || !(0..=3).contains(&route_kind)
        || inputs.iter().any(|input| {
            !(QUOTA_CAPACITY_LANE_MAIN..=QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL)
                .contains(&input.lane)
                || input.scale_bps < 0
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }

    let mut fields = Vec::with_capacity(inputs.len() * QUOTA_CAPACITY_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            input.lane,
            allowed_tag(input.pair_allowed),
            allowed_tag(input.outer_allowed),
            limit_reached_tag(input.pair_limit_reached),
            limit_reached_tag(input.outer_limit_reached),
            admission_value_tag(input.rate_limit_reached_type),
            admission_value_tag(input.camel_rate_limit_reached_type),
            admission_value_tag(input.spend_control_reached),
            admission_value_tag(input.camel_spend_control_reached),
            admission_value_tag(input.ordinary_usage_allowed),
            input.five_hour_used_percent,
            i64::from(input.five_hour_has_value),
            input.five_hour_reset_at,
            input.weekly_used_percent,
            i64::from(input.weekly_has_value),
            input.weekly_reset_at,
            input.primary_used_percent,
            i64::from(input.primary_has_value),
            input.secondary_used_percent,
            i64::from(input.secondary_has_value),
            input.scale_bps,
            input.now,
        ]);
    }

    let mut lane = vec![0_i64; inputs.len()];
    let mut five_hour_remaining = vec![0_i64; inputs.len()];
    let mut weekly_remaining = vec![0_i64; inputs.len()];
    let mut five_hour_status = vec![0_i64; inputs.len()];
    let mut weekly_status = vec![0_i64; inputs.len()];
    let mut pressure_band = vec![0_i64; inputs.len()];
    let mut admission_allowed = vec![0_i64; inputs.len()];
    let mut pair_ready = vec![0_i64; inputs.len()];
    let mut any_window_exhausted = vec![0_i64; inputs.len()];
    let mut usable = vec![0_i64; inputs.len()];
    let mut routing_eligible = vec![0_i64; inputs.len()];
    let mut reserve_floor = vec![0_i64; inputs.len()];
    let mut five_hour_pressure = vec![0_i64; inputs.len()];
    let mut weekly_pressure = vec![0_i64; inputs.len()];
    let mut total_pressure = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_quota_capacity_batch_v2(
            fields.as_ptr() as u64,
            lane.as_mut_ptr() as u64,
            five_hour_remaining.as_mut_ptr() as u64,
            weekly_remaining.as_mut_ptr() as u64,
            five_hour_status.as_mut_ptr() as u64,
            weekly_status.as_mut_ptr() as u64,
            pressure_band.as_mut_ptr() as u64,
            admission_allowed.as_mut_ptr() as u64,
            pair_ready.as_mut_ptr() as u64,
            any_window_exhausted.as_mut_ptr() as u64,
            usable.as_mut_ptr() as u64,
            routing_eligible.as_mut_ptr() as u64,
            reserve_floor.as_mut_ptr() as u64,
            five_hour_pressure.as_mut_ptr() as u64,
            weekly_pressure.as_mut_ptr() as u64,
            total_pressure.as_mut_ptr() as u64,
            route_kind,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(if status == 1 {
            crate::MojoError::InvalidInput
        } else {
            crate::MojoError::InvalidOutput
        });
    }

    let mut outputs = Vec::with_capacity(inputs.len());
    for index in 0..inputs.len() {
        if lane[index] != inputs[index].lane
            || !(0..=100).contains(&five_hour_remaining[index])
            || !(0..=100).contains(&weekly_remaining[index])
            || !(0..=4).contains(&five_hour_status[index])
            || !(0..=4).contains(&weekly_status[index])
            || !(0..=4).contains(&pressure_band[index])
            || !matches!(
                (
                    admission_allowed[index],
                    pair_ready[index],
                    any_window_exhausted[index],
                    usable[index],
                    routing_eligible[index]
                ),
                (0 | 1, 0 | 1, 0 | 1, 0 | 1, 0 | 1)
            )
            || reserve_floor[index] < 0
            || reserve_floor[index] > 100
            || five_hour_pressure[index] < 0
            || weekly_pressure[index] < 0
            || total_pressure[index] < 0
            || usable[index] != admission_allowed[index] * pair_ready[index]
            || routing_eligible[index]
                != i64::from(
                    usable[index] == 1
                        && matches!(
                            inputs[index].lane,
                            QUOTA_CAPACITY_LANE_MAIN | QUOTA_CAPACITY_LANE_MODEL_SPECIFIC
                        ),
                )
        {
            return Err(crate::MojoError::InvalidOutput);
        }
        outputs.push(QuotaCapacityOutput {
            lane: lane[index],
            five_hour_remaining: five_hour_remaining[index],
            weekly_remaining: weekly_remaining[index],
            five_hour_status: five_hour_status[index],
            weekly_status: weekly_status[index],
            pressure_band: pressure_band[index],
            admission_allowed: admission_allowed[index] == 1,
            pair_ready: pair_ready[index] == 1,
            any_window_exhausted: any_window_exhausted[index] == 1,
            usable: usable[index] == 1,
            routing_eligible: routing_eligible[index] == 1,
            reserve_floor: reserve_floor[index],
            five_hour_pressure: five_hour_pressure[index],
            weekly_pressure: weekly_pressure[index],
            total_pressure: total_pressure[index],
        });
    }
    Ok(outputs)
}

fn allowed_tag(value: Option<bool>) -> i64 {
    match value {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    }
}

fn limit_reached_tag(value: Option<bool>) -> i64 {
    match value {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    }
}

fn admission_value_tag(value: QuotaAdmissionValue) -> i64 {
    match value {
        QuotaAdmissionValue::Missing => 0,
        QuotaAdmissionValue::Null => 1,
        QuotaAdmissionValue::True => 2,
        QuotaAdmissionValue::False => 3,
        QuotaAdmissionValue::Other => 4,
    }
}
