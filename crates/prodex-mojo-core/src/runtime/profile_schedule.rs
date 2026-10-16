use super::{
    ProfileScheduleInput, RUNTIME_PROFILE_SCHEDULE_FIELD_COUNT, RUNTIME_PROFILE_SCHEDULE_MAX_COUNT,
    prodex_runtime_quota_profile_schedule_batch,
};

pub fn profile_schedule_batch(
    inputs: &[ProfileScheduleInput],
) -> Result<Vec<usize>, crate::MojoError> {
    if inputs.len() > RUNTIME_PROFILE_SCHEDULE_MAX_COUNT
        || inputs.iter().any(|input| {
            let score = input.score;
            score.weekly_pressure < 0
                || score.five_hour_pressure < 0
                || score.scale_bps < 0
                || score.weekly_remaining < 0
                || score.weekly_remaining > 100
                || score.five_hour_remaining < 0
                || score.five_hour_remaining > 100
                || score.weekly_weight < 0
                || input.provider_priority < 0
                || !(0..=1).contains(&input.quota_source)
                || input.order_index < 0
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }

    let mut fields = Vec::with_capacity(inputs.len() * RUNTIME_PROFILE_SCHEDULE_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            input.score.weekly_pressure,
            input.score.five_hour_pressure,
            input.score.scale_bps,
            input.score.weekly_remaining,
            input.score.five_hour_remaining,
            i64::from(input.score.windows_complete),
            input.score.weekly_weight,
            input.provider_priority,
            i64::from(input.in_selection_cooldown),
            input.last_selected_at,
            input.weekly_reset_at,
            input.five_hour_reset_at,
            input.quota_source,
            i64::from(input.preferred),
            i64::from(input.affinity_preferred),
            input.order_index,
        ]);
    }

    let mut total_pressure = vec![0_i64; inputs.len()];
    let mut weekly_pressure = vec![0_i64; inputs.len()];
    let mut five_hour_pressure = vec![0_i64; inputs.len()];
    let mut reserve_floor = vec![0_i64; inputs.len()];
    let mut ordered_indices = vec![0_i64; inputs.len()];
    let mut ordered_count = 0_i64;
    let status = unsafe {
        prodex_runtime_quota_profile_schedule_batch(
            fields.as_ptr(),
            total_pressure.as_mut_ptr(),
            weekly_pressure.as_mut_ptr(),
            five_hour_pressure.as_mut_ptr(),
            reserve_floor.as_mut_ptr(),
            ordered_indices.as_mut_ptr(),
            &mut ordered_count,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || ordered_count < 0 || ordered_count as usize != inputs.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    if total_pressure
        .iter()
        .chain(&weekly_pressure)
        .chain(&five_hour_pressure)
        .any(|value| *value < 0)
        || reserve_floor.iter().any(|value| !(0..=100).contains(value))
    {
        return Err(crate::MojoError::InvalidOutput);
    }

    let mut seen = vec![false; inputs.len()];
    let ordered_indices = ordered_indices
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index)
                .ok()
                .filter(|index| *index < inputs.len())
                .ok_or(crate::MojoError::InvalidOutput)?;
            if seen[index] {
                return Err(crate::MojoError::InvalidOutput);
            }
            seen[index] = true;
            Ok(index)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if seen.iter().any(|value| !value) {
        return Err(crate::MojoError::InvalidOutput);
    }

    Ok(ordered_indices)
}
