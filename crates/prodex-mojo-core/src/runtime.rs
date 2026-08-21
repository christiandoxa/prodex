#[cfg(feature = "mojo-runtime")]
pub use crate::runtime_decisions::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileScoreInput {
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub scale_bps: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub reserve_bias: i64,
    pub weekly_weight: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileScore {
    pub total_pressure: i64,
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub reserve_floor: i64,
}

pub const RUNTIME_PROFILE_ORDER_FIELD_COUNT: usize = 15;
pub const RUNTIME_PROFILE_ORDER_MAX_COUNT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextPressureSnapshot {
    pub effective_usable_context_tokens: Option<u64>,
    pub effective_used_tokens: u64,
    pub pressure_basis_points: Option<u32>,
    pub pressure_band: i64,
    pub absolute_safety_floor_tokens: u64,
    pub estimator_confidence: i64,
}

pub const RUNTIME_CANDIDATE_PLAN_FIELD_COUNT: usize = 22;
pub const RUNTIME_CANDIDATE_PLAN_MAX_COUNT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCandidatePlan {
    pub ready_indices: Vec<usize>,
    pub fallback_indices: Vec<usize>,
}

pub fn candidate_plan_self_test() -> bool {
    let mut fields = vec![0_i64; RUNTIME_CANDIDATE_PLAN_FIELD_COUNT * 2];
    fields[1] = 1;
    fields[RUNTIME_CANDIDATE_PLAN_FIELD_COUNT + 16] = 1;
    runtime_candidate_plan_batch(&fields, 0)
        .is_ok_and(|plan| plan.ready_indices == [1, 0] && plan.fallback_indices == [1, 0])
}

pub fn smart_context_pressure_snapshot_self_test() -> bool {
    smart_context_pressure_snapshot(Some(100), 20, 72, 0, false, false, false).is_ok_and(
        |snapshot| {
            snapshot.effective_usable_context_tokens == Some(80)
                && snapshot.pressure_basis_points == Some(9_000)
                && snapshot.pressure_band == 4
                && snapshot.absolute_safety_floor_tokens == 1_000
                && snapshot.estimator_confidence == 0
        },
    )
}

unsafe extern "C" {
    fn prodex_runtime_quota_pressure_band_for_route(
        five_hour_remaining_percent: i64,
        five_hour_has_value: i64,
        weekly_remaining_percent: i64,
        weekly_has_value: i64,
        route_kind: i64,
    ) -> i64;
    fn prodex_runtime_quota_profile_score_batch(
        weekly_pressure: *const i64,
        five_hour_pressure: *const i64,
        scale_bps: *const i64,
        weekly_remaining: *const i64,
        five_hour_remaining: *const i64,
        reserve_bias: *const i64,
        weekly_weight: *const i64,
        total_pressure: *mut i64,
        scaled_weekly_pressure: *mut i64,
        scaled_five_hour_pressure: *mut i64,
        reserve_floor: *mut i64,
        count: i64,
    ) -> i64;
    fn prodex_runtime_quota_profile_order_batch(
        fields: *const i64,
        ordered_indices: *mut i64,
        ordered_count: *mut i64,
        count: i64,
    ) -> i64;
    fn prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64;
    fn prodex_smart_context_pressure_snapshot(
        model_context_window_tokens: u64,
        model_context_window_has_value: i64,
        reserved_output_tokens: u64,
        effective_input_tokens: u64,
        effective_input_source: i64,
        unknown_token_window: i64,
        zero_context_window: i64,
        reserved_output_consumes_window: i64,
        effective_usable_context_tokens: *mut u64,
        effective_usable_has_value: *mut i64,
        pressure_basis_points: *mut u64,
        pressure_has_value: *mut i64,
        pressure_band: *mut i64,
        absolute_safety_floor_tokens: *mut u64,
        estimator_confidence: *mut i64,
    ) -> i64;
    fn prodex_runtime_candidate_plan_batch(
        fields: *const i64,
        ready_indices: *mut i64,
        ready_count: *mut i64,
        fallback_indices: *mut i64,
        fallback_count: *mut i64,
        count: i64,
        route_kind: i64,
    ) -> i64;
}
pub fn pressure_band_for_route(
    five_hour: Option<(i64, i64)>,
    weekly: Option<(i64, i64)>,
    route_kind: i64,
) -> Result<i64, crate::MojoError> {
    if five_hour.is_some_and(|(_, has_value)| !matches!(has_value, 0 | 1))
        || weekly.is_some_and(|(_, has_value)| !matches!(has_value, 0 | 1))
        || !(0..=3).contains(&route_kind)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let (five_hour_remaining_percent, five_hour_has_value) = five_hour.unwrap_or((0, 0));
    let (weekly_remaining_percent, weekly_has_value) = weekly.unwrap_or((0, 0));
    let band = unsafe {
        prodex_runtime_quota_pressure_band_for_route(
            five_hour_remaining_percent,
            five_hour_has_value,
            weekly_remaining_percent,
            weekly_has_value,
            route_kind,
        )
    };
    (0..=4)
        .contains(&band)
        .then_some(band)
        .ok_or(crate::MojoError::InvalidOutput)
}

pub fn profile_scores_batch(
    inputs: &[ProfileScoreInput],
) -> Result<Vec<ProfileScore>, crate::MojoError> {
    if inputs.len() > 64
        || inputs.iter().any(|input| {
            input.weekly_pressure < 0
                || input.five_hour_pressure < 0
                || input.scale_bps < 0
                || input.weekly_remaining < 0
                || input.weekly_remaining > 100
                || input.five_hour_remaining < 0
                || input.five_hour_remaining > 100
                || input.reserve_bias < 0
                || input.weekly_weight < 0
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let weekly_pressure = inputs
        .iter()
        .map(|input| input.weekly_pressure)
        .collect::<Vec<_>>();
    let five_hour_pressure = inputs
        .iter()
        .map(|input| input.five_hour_pressure)
        .collect::<Vec<_>>();
    let scale_bps = inputs
        .iter()
        .map(|input| input.scale_bps)
        .collect::<Vec<_>>();
    let weekly_remaining = inputs
        .iter()
        .map(|input| input.weekly_remaining)
        .collect::<Vec<_>>();
    let five_hour_remaining = inputs
        .iter()
        .map(|input| input.five_hour_remaining)
        .collect::<Vec<_>>();
    let reserve_bias = inputs
        .iter()
        .map(|input| input.reserve_bias)
        .collect::<Vec<_>>();
    let weekly_weight = inputs
        .iter()
        .map(|input| input.weekly_weight)
        .collect::<Vec<_>>();
    let mut total_pressure = vec![0_i64; inputs.len()];
    let mut scaled_weekly_pressure = vec![0_i64; inputs.len()];
    let mut scaled_five_hour_pressure = vec![0_i64; inputs.len()];
    let mut reserve_floor = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_runtime_quota_profile_score_batch(
            weekly_pressure.as_ptr(),
            five_hour_pressure.as_ptr(),
            scale_bps.as_ptr(),
            weekly_remaining.as_ptr(),
            five_hour_remaining.as_ptr(),
            reserve_bias.as_ptr(),
            weekly_weight.as_ptr(),
            total_pressure.as_mut_ptr(),
            scaled_weekly_pressure.as_mut_ptr(),
            scaled_five_hour_pressure.as_mut_ptr(),
            reserve_floor.as_mut_ptr(),
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0
        || total_pressure.iter().any(|value| *value < 0)
        || scaled_weekly_pressure.iter().any(|value| *value < 0)
        || scaled_five_hour_pressure.iter().any(|value| *value < 0)
        || reserve_floor.iter().any(|value| !(0..=100).contains(value))
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(inputs
        .iter()
        .enumerate()
        .map(|(index, _)| ProfileScore {
            total_pressure: total_pressure[index],
            weekly_pressure: scaled_weekly_pressure[index],
            five_hour_pressure: scaled_five_hour_pressure[index],
            reserve_floor: reserve_floor[index],
        })
        .collect())
}

pub fn profile_order_self_test() -> bool {
    let mut fields = vec![0_i64; RUNTIME_PROFILE_ORDER_FIELD_COUNT * 2];
    fields[RUNTIME_PROFILE_ORDER_FIELD_COUNT] = 1;
    profile_order_batch(&fields).is_ok_and(|order| order == [0, 1])
}

pub fn profile_order_batch(fields: &[i64]) -> Result<Vec<usize>, crate::MojoError> {
    if !fields
        .len()
        .is_multiple_of(RUNTIME_PROFILE_ORDER_FIELD_COUNT)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let count = fields.len() / RUNTIME_PROFILE_ORDER_FIELD_COUNT;
    if count > RUNTIME_PROFILE_ORDER_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut ordered_indices = vec![0_i64; count];
    let mut ordered_count = 0_i64;
    let status = unsafe {
        prodex_runtime_quota_profile_order_batch(
            fields.as_ptr(),
            ordered_indices.as_mut_ptr(),
            &mut ordered_count,
            i64::try_from(count).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || ordered_count < 0 || ordered_count as usize != count {
        return Err(crate::MojoError::InvalidOutput);
    }

    let mut seen = vec![false; count];
    ordered_indices
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index)
                .ok()
                .filter(|index| *index < count)
                .ok_or(crate::MojoError::InvalidOutput)?;
            if seen[index] {
                return Err(crate::MojoError::InvalidOutput);
            }
            seen[index] = true;
            Ok(index)
        })
        .collect()
}

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64 {
    unsafe { prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes) }
}

pub fn smart_context_pressure_snapshot(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_tokens: u64,
    effective_input_source: i64,
    unknown_token_window: bool,
    zero_context_window: bool,
    reserved_output_consumes_window: bool,
) -> Result<SmartContextPressureSnapshot, crate::MojoError> {
    if !(0..=3).contains(&effective_input_source) {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut effective_usable_context_tokens = 0;
    let mut effective_usable_has_value = 0;
    let mut pressure_basis_points = 0;
    let mut pressure_has_value = 0;
    let mut pressure_band = 0;
    let mut absolute_safety_floor_tokens = 0;
    let mut estimator_confidence = 0;
    let status = unsafe {
        prodex_smart_context_pressure_snapshot(
            model_context_window_tokens.unwrap_or(0),
            i64::from(model_context_window_tokens.is_some()),
            reserved_output_tokens,
            effective_input_tokens,
            effective_input_source,
            i64::from(unknown_token_window),
            i64::from(zero_context_window),
            i64::from(reserved_output_consumes_window),
            &mut effective_usable_context_tokens,
            &mut effective_usable_has_value,
            &mut pressure_basis_points,
            &mut pressure_has_value,
            &mut pressure_band,
            &mut absolute_safety_floor_tokens,
            &mut estimator_confidence,
        )
    };
    if status != 0
        || !matches!(effective_usable_has_value, 0 | 1)
        || !matches!(pressure_has_value, 0 | 1)
        || !matches!(pressure_band, 0..=5)
        || !matches!(estimator_confidence, 0..=2)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextPressureSnapshot {
        effective_usable_context_tokens: (effective_usable_has_value == 1)
            .then_some(effective_usable_context_tokens),
        effective_used_tokens: effective_input_tokens,
        pressure_basis_points: (pressure_has_value == 1)
            .then_some(pressure_basis_points.min(u64::from(u32::MAX)) as u32),
        pressure_band,
        absolute_safety_floor_tokens,
        estimator_confidence,
    })
}

pub fn runtime_candidate_plan_batch(
    fields: &[i64],
    route_kind: i64,
) -> Result<RuntimeCandidatePlan, crate::MojoError> {
    if !fields
        .len()
        .is_multiple_of(RUNTIME_CANDIDATE_PLAN_FIELD_COUNT)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let count = fields.len() / RUNTIME_CANDIDATE_PLAN_FIELD_COUNT;
    if count > RUNTIME_CANDIDATE_PLAN_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    if !(0..=3).contains(&route_kind) {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut ready_indices = vec![0_i64; count];
    let mut fallback_indices = vec![0_i64; count];
    let mut ready_count = 0_i64;
    let mut fallback_count = 0_i64;
    let status = unsafe {
        prodex_runtime_candidate_plan_batch(
            fields.as_ptr(),
            ready_indices.as_mut_ptr(),
            &mut ready_count,
            fallback_indices.as_mut_ptr(),
            &mut fallback_count,
            i64::try_from(count).map_err(|_| crate::MojoError::InvalidInput)?,
            route_kind,
        )
    };
    if status != 0
        || ready_count < 0
        || fallback_count < 0
        || ready_count as usize > count
        || fallback_count as usize > count
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    let ready_indices = runtime_candidate_plan_indices(&ready_indices, ready_count, count)
        .ok_or(crate::MojoError::InvalidOutput)?;
    let fallback_indices = runtime_candidate_plan_indices(&fallback_indices, fallback_count, count)
        .ok_or(crate::MojoError::InvalidOutput)?;
    if fallback_indices.len() != count {
        return Err(crate::MojoError::InvalidOutput);
    }
    let mut seen_ready = vec![false; count];
    for index in &ready_indices {
        if seen_ready[*index] {
            return Err(crate::MojoError::InvalidOutput);
        }
        seen_ready[*index] = true;
    }
    let mut seen_fallback = vec![false; count];
    for index in &fallback_indices {
        if seen_fallback[*index] {
            return Err(crate::MojoError::InvalidOutput);
        }
        seen_fallback[*index] = true;
    }
    if seen_fallback.iter().any(|value| !value) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(RuntimeCandidatePlan {
        ready_indices,
        fallback_indices,
    })
}

fn runtime_candidate_plan_indices(
    values: &[i64],
    count: i64,
    candidate_count: usize,
) -> Option<Vec<usize>> {
    values
        .get(..usize::try_from(count).ok()?)?
        .iter()
        .map(|value| {
            usize::try_from(*value)
                .ok()
                .filter(|index| *index < candidate_count)
        })
        .collect()
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    use std::cmp::Ordering;

    fn field(fields: &[i64], index: usize, offset: usize) -> i64 {
        fields[index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT + offset]
    }

    fn ready_cmp(fields: &[i64], left: usize, right: usize, route_kind: i64) -> Ordering {
        let left_source = if route_kind == 0 || route_kind == 2 {
            field(fields, left, 11)
        } else {
            0
        };
        let right_source = if route_kind == 0 || route_kind == 2 {
            field(fields, right, 11)
        } else {
            0
        };
        let ascending = [
            (field(fields, left, 1), field(fields, right, 1)),
            (field(fields, left, 2), field(fields, right, 2)),
            (field(fields, left, 3), field(fields, right, 3)),
            (field(fields, left, 4), field(fields, right, 4)),
            (field(fields, left, 8), field(fields, right, 8)),
            (field(fields, left, 9), field(fields, right, 9)),
            (left_source, right_source),
            (field(fields, left, 12), field(fields, right, 12)),
            (field(fields, left, 13), field(fields, right, 13)),
            (field(fields, left, 14), field(fields, right, 14)),
            (field(fields, left, 15), field(fields, right, 15)),
            (field(fields, left, 16), field(fields, right, 16)),
            (field(fields, left, 17), field(fields, right, 17)),
        ];
        for (left_value, right_value) in ascending {
            let ordering = left_value.cmp(&right_value);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        for offset in [5, 6, 7] {
            let ordering = field(fields, right, offset).cmp(&field(fields, left, offset));
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        left.cmp(&right)
    }

    fn fallback_cmp(fields: &[i64], left: usize, right: usize, route_kind: i64) -> Ordering {
        for offset in 18..=21 {
            let ordering = field(fields, left, offset).cmp(&field(fields, right, offset));
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        ready_cmp(fields, left, right, route_kind)
    }

    fn next_random(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    }

    #[derive(Clone, Copy)]
    struct PressureCase {
        model_context_window_tokens: Option<u64>,
        reserved_output_tokens: u64,
        effective_input_tokens: u64,
        source: i64,
        unknown_window: bool,
        zero_context_window: bool,
        reserved_output_consumes_window: bool,
    }

    fn generated_pressure_case(state: &mut u64, case: usize) -> PressureCase {
        PressureCase {
            model_context_window_tokens: if case.is_multiple_of(5) {
                None
            } else {
                Some(next_random(state) % 200_000)
            },
            reserved_output_tokens: if case.is_multiple_of(7) {
                u64::MAX
            } else {
                next_random(state) % 200_000
            },
            effective_input_tokens: if case.is_multiple_of(11) {
                u64::MAX
            } else {
                next_random(state) % 400_000
            },
            source: (case % 4) as i64,
            unknown_window: case.is_multiple_of(6),
            zero_context_window: case.is_multiple_of(9),
            reserved_output_consumes_window: case.is_multiple_of(8),
        }
    }

    fn expected_pressure_snapshot(input: PressureCase) -> SmartContextPressureSnapshot {
        let usable = input
            .model_context_window_tokens
            .and_then(|window| window.checked_sub(input.reserved_output_tokens));
        let pressure_basis_points = usable.and_then(|usable| {
            (usable > 0).then(|| {
                input
                    .effective_input_tokens
                    .saturating_mul(10_000)
                    .checked_div(usable)
                    .unwrap_or(u64::MAX)
                    .min(u64::from(u32::MAX)) as u32
            })
        });
        let pressure_band = match pressure_basis_points {
            None => 0,
            Some(value) if value >= 10_000 => 5,
            Some(value) if value >= 9_000 => 4,
            Some(value) if value >= 7_500 => 3,
            Some(value) if value >= 5_000 => 2,
            Some(_) => 1,
        };
        let estimator_confidence = if input.unknown_window
            || input.zero_context_window
            || input.reserved_output_consumes_window
        {
            2
        } else {
            match input.source {
                0 | 2 => 0,
                1 => 1,
                _ => 2,
            }
        };
        let usable_for_floor = input
            .model_context_window_tokens
            .map(|window| window.saturating_sub(input.reserved_output_tokens));
        SmartContextPressureSnapshot {
            effective_usable_context_tokens: usable,
            effective_used_tokens: input.effective_input_tokens,
            pressure_basis_points,
            pressure_band,
            absolute_safety_floor_tokens: usable_for_floor
                .map(|value| (value / 20).clamp(1_000, 8_000))
                .unwrap_or(2_000),
            estimator_confidence,
        }
    }

    #[test]
    fn candidate_plan_matches_rust_oracle_for_generated_batches() {
        let mut state = 0x6d6f6a6f5f706c61_u64;
        for case in 0..300 {
            let count = (next_random(&mut state) % 25) as usize;
            let route_kind = (case % 4) as i64;
            let mut fields = vec![0_i64; count * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT];
            for index in 0..count {
                let base = index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT;
                fields[base] = (next_random(&mut state) % 2) as i64;
                fields[base + 1] = (next_random(&mut state) % 8) as i64;
                fields[base + 2] = (next_random(&mut state) % 5) as i64;
                for offset in 3..=10 {
                    fields[base + offset] = (next_random(&mut state) % 401) as i64 - 200;
                }
                fields[base + 11] = (next_random(&mut state) % 2) as i64;
                fields[base + 12] = (next_random(&mut state) % 32) as i64;
                fields[base + 13] = (next_random(&mut state) % 32) as i64;
                fields[base + 14] = (next_random(&mut state) % 2) as i64;
                fields[base + 15] = next_random(&mut state) as i64;
                fields[base + 16] = index as i64;
                fields[base + 17] = next_random(&mut state) as i64;
                fields[base + 18] = (next_random(&mut state) % 8) as i64;
                for offset in 19..=21 {
                    fields[base + offset] = (next_random(&mut state) % 401) as i64 - 200;
                }
            }

            let mut expected_ready = (0..count)
                .filter(|index| field(&fields, *index, 0) == 0)
                .collect::<Vec<_>>();
            expected_ready.sort_by(|left, right| ready_cmp(&fields, *left, *right, route_kind));
            let mut expected_fallback = (0..count).collect::<Vec<_>>();
            expected_fallback
                .sort_by(|left, right| fallback_cmp(&fields, *left, *right, route_kind));

            let actual = runtime_candidate_plan_batch(&fields, route_kind)
                .expect("strict Mojo candidate plan should accept generated input");
            assert_eq!(
                actual.ready_indices, expected_ready,
                "candidate case {case}"
            );
            assert_eq!(
                actual.fallback_indices, expected_fallback,
                "candidate case {case}"
            );
        }
    }

    #[test]
    fn pressure_snapshot_matches_rust_oracle_for_generated_inputs() {
        let mut state = 0x7072657373757265_u64;
        for case in 0..300 {
            let input = generated_pressure_case(&mut state, case);
            let expected = expected_pressure_snapshot(input);
            let actual = smart_context_pressure_snapshot(
                input.model_context_window_tokens,
                input.reserved_output_tokens,
                input.effective_input_tokens,
                input.source,
                input.unknown_window,
                input.zero_context_window,
                input.reserved_output_consumes_window,
            )
            .expect("strict Mojo pressure snapshot should accept generated input");
            assert_eq!(actual, expected, "pressure case {case}");
        }
    }

    fn profile_field(fields: &[i64], index: usize, offset: usize) -> i64 {
        fields[index * RUNTIME_PROFILE_ORDER_FIELD_COUNT + offset]
    }

    fn profile_cmp(fields: &[i64], left: usize, right: usize) -> Ordering {
        for offset in 0..=6 {
            let ordering =
                profile_field(fields, left, offset).cmp(&profile_field(fields, right, offset));
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        for offset in 7..=9 {
            let ordering =
                profile_field(fields, right, offset).cmp(&profile_field(fields, left, offset));
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        for offset in 10..RUNTIME_PROFILE_ORDER_FIELD_COUNT {
            let ordering =
                profile_field(fields, left, offset).cmp(&profile_field(fields, right, offset));
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        left.cmp(&right)
    }

    #[test]
    fn profile_order_matches_rust_oracle_for_generated_batches() {
        let mut state = 0x70726f66696c65_u64;
        for case in 0..400 {
            let count = (next_random(&mut state) % 32) as usize;
            let mut fields = vec![0_i64; count * RUNTIME_PROFILE_ORDER_FIELD_COUNT];
            for index in 0..count {
                let base = index * RUNTIME_PROFILE_ORDER_FIELD_COUNT;
                fields[base] = (next_random(&mut state) % 8) as i64;
                fields[base + 1] = (next_random(&mut state) % 2) as i64;
                fields[base + 2] = (next_random(&mut state) % 2) as i64;
                fields[base + 3] = next_random(&mut state) as i64;
                for offset in 4..=11 {
                    fields[base + offset] = (next_random(&mut state) % 10_000) as i64;
                }
                fields[base + 12] = (next_random(&mut state) % 2) as i64;
                fields[base + 13] = (next_random(&mut state) % 2) as i64;
                fields[base + 14] = index as i64;
            }
            let mut expected = (0..count).collect::<Vec<_>>();
            expected.sort_by(|left, right| profile_cmp(&fields, *left, *right));
            let actual = profile_order_batch(&fields)
                .expect("strict Mojo profile order should accept generated input");
            assert_eq!(actual, expected, "profile order case {case}");
        }
    }
}
