use super::{
    QuotaRouteScoreInput, QuotaScore, RUNTIME_QUOTA_ROUTE_SCORE_FIELD_COUNT,
    RUNTIME_QUOTA_SCORE_MAX_COUNT,
};

/// Resolve route-aware profile scores from raw window observations in one
/// bounded Mojo call. Host code owns usage acquisition and plan selection;
/// Mojo owns scaling, window completeness, pressure-band, and score rules.
pub fn quota_route_score_batch(
    inputs: &[QuotaRouteScoreInput],
    route_kind: i64,
) -> Result<Vec<QuotaScore>, crate::MojoError> {
    if inputs.len() > RUNTIME_QUOTA_SCORE_MAX_COUNT
        || !(0..=3).contains(&route_kind)
        || inputs.iter().any(|input| {
            input.weekly_pressure < 0
                || input.five_hour_pressure < 0
                || input.scale_bps < 0
                || input.weekly_remaining < 0
                || input.weekly_remaining > 100
                || input.five_hour_remaining < 0
                || input.five_hour_remaining > 100
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }

    let mut fields = Vec::with_capacity(inputs.len() * RUNTIME_QUOTA_ROUTE_SCORE_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            input.weekly_pressure,
            input.five_hour_pressure,
            input.scale_bps,
            input.weekly_remaining,
            input.five_hour_remaining,
            i64::from(input.weekly_has_value),
            i64::from(input.five_hour_has_value),
            input.weekly_reset_at,
            input.five_hour_reset_at,
        ]);
    }

    let mut pressure_band = vec![0_i64; inputs.len()];
    let mut total_pressure = vec![0_i64; inputs.len()];
    let mut weekly_pressure = vec![0_i64; inputs.len()];
    let mut five_hour_pressure = vec![0_i64; inputs.len()];
    let mut reserve_floor = vec![0_i64; inputs.len()];
    let mut weekly_remaining = vec![0_i64; inputs.len()];
    let mut five_hour_remaining = vec![0_i64; inputs.len()];
    let mut weekly_reset_at = vec![0_i64; inputs.len()];
    let mut five_hour_reset_at = vec![0_i64; inputs.len()];
    let status = unsafe {
        super::prodex_runtime_quota_route_score_resolution_batch(
            fields.as_ptr() as u64,
            pressure_band.as_mut_ptr() as u64,
            total_pressure.as_mut_ptr() as u64,
            weekly_pressure.as_mut_ptr() as u64,
            five_hour_pressure.as_mut_ptr() as u64,
            reserve_floor.as_mut_ptr() as u64,
            weekly_remaining.as_mut_ptr() as u64,
            five_hour_remaining.as_mut_ptr() as u64,
            weekly_reset_at.as_mut_ptr() as u64,
            five_hour_reset_at.as_mut_ptr() as u64,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            route_kind,
        )
    };
    if status != 0
        || pressure_band.iter().any(|value| !(0..=4).contains(value))
        || total_pressure.iter().any(|value| *value < 0)
        || weekly_pressure.iter().any(|value| *value < 0)
        || five_hour_pressure.iter().any(|value| *value < 0)
        || reserve_floor.iter().any(|value| *value < 0)
        || weekly_remaining
            .iter()
            .any(|value| !(0..=100).contains(value))
        || five_hour_remaining
            .iter()
            .any(|value| !(0..=100).contains(value))
    {
        return Err(crate::MojoError::InvalidOutput);
    }

    Ok((0..inputs.len())
        .map(|index| QuotaScore {
            pressure_band: pressure_band[index],
            total_pressure: total_pressure[index],
            weekly_pressure: weekly_pressure[index],
            five_hour_pressure: five_hour_pressure[index],
            reserve_floor: reserve_floor[index],
            weekly_remaining: weekly_remaining[index],
            five_hour_remaining: five_hour_remaining[index],
            weekly_reset_at: weekly_reset_at[index],
            five_hour_reset_at: five_hour_reset_at[index],
        })
        .collect())
}
