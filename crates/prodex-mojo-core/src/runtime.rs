use std::cmp::min;

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
    #[cfg(prodex_mojo_fallback)]
    {
        return true;
    }

    #[cfg(not(prodex_mojo_fallback))]
    {
        let mut fields = vec![0_i64; RUNTIME_CANDIDATE_PLAN_FIELD_COUNT * 2];
        fields[1] = 1;
        fields[RUNTIME_CANDIDATE_PLAN_FIELD_COUNT + 16] = 1;
        runtime_candidate_plan_batch(&fields, 0)
            .is_some_and(|plan| plan.ready_indices == [1, 0] && plan.fallback_indices == [1, 0])
    }
}

pub fn smart_context_pressure_snapshot_self_test() -> bool {
    smart_context_pressure_snapshot(Some(100), 20, 72, 0, false, false, false).is_some_and(
        |snapshot| {
            snapshot.effective_usable_context_tokens == Some(80)
                && snapshot.pressure_basis_points == Some(9_000)
                && snapshot.pressure_band == 4
                && snapshot.absolute_safety_floor_tokens == 1_000
                && snapshot.estimator_confidence == 0
        },
    )
}

#[cfg(not(prodex_mojo_fallback))]
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
) -> i64 {
    #[cfg(not(prodex_mojo_fallback))]
    {
        let (five_hour_remaining_percent, five_hour_has_value) = five_hour.unwrap_or((0, 0));
        let (weekly_remaining_percent, weekly_has_value) = weekly.unwrap_or((0, 0));
        unsafe {
            prodex_runtime_quota_pressure_band_for_route(
                five_hour_remaining_percent,
                five_hour_has_value,
                weekly_remaining_percent,
                weekly_has_value,
                route_kind,
            )
        }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        if five_hour.is_none() && weekly.is_none() {
            return 4;
        }
        if five_hour.is_some_and(|(remaining, _)| remaining == 0)
            || weekly.is_some_and(|(remaining, _)| remaining == 0)
        {
            return 3;
        }
        let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) =
            if route_kind == 0 || route_kind == 2 {
                (20, 10, 10, 5)
            } else {
                (10, 5, 5, 3)
            };
        let band = |value: Option<(i64, i64)>, thin: i64, critical: i64| match value {
            None => 0,
            Some((remaining, _)) if remaining <= critical => 2,
            Some((remaining, _)) if remaining <= thin => 1,
            Some(_) => 0,
        };
        band(weekly, thin_weekly, critical_weekly).max(band(
            five_hour,
            thin_five_hour,
            critical_five_hour,
        ))
    }
}

pub fn profile_scores_batch(inputs: &[ProfileScoreInput]) -> Vec<ProfileScore> {
    #[cfg(not(prodex_mojo_fallback))]
    {
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
                i64::try_from(inputs.len()).unwrap_or(i64::MAX),
            )
        };
        if status == 0 {
            return inputs
                .iter()
                .enumerate()
                .map(|(index, _)| ProfileScore {
                    total_pressure: total_pressure[index],
                    weekly_pressure: scaled_weekly_pressure[index],
                    five_hour_pressure: scaled_five_hour_pressure[index],
                    reserve_floor: reserve_floor[index],
                })
                .collect();
        }
    }

    inputs.iter().copied().map(profile_score_rust).collect()
}

fn profile_score_rust(input: ProfileScoreInput) -> ProfileScore {
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
    ProfileScore {
        total_pressure: input
            .reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(input.weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: min(input.weekly_remaining, input.five_hour_remaining),
    }
}

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64 {
    #[cfg(not(prodex_mojo_fallback))]
    {
        unsafe { prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes) }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        body_bytes.saturating_add(3) / 4
    }
}

pub fn smart_context_pressure_snapshot(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_tokens: u64,
    effective_input_source: i64,
    unknown_token_window: bool,
    zero_context_window: bool,
    reserved_output_consumes_window: bool,
) -> Option<SmartContextPressureSnapshot> {
    #[cfg(not(prodex_mojo_fallback))]
    {
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
            return None;
        }
        Some(SmartContextPressureSnapshot {
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

    #[cfg(prodex_mojo_fallback)]
    {
        let effective_usable_context_tokens = model_context_window_tokens
            .and_then(|window| window.checked_sub(reserved_output_tokens));
        let pressure_basis_points = effective_usable_context_tokens.and_then(|usable| {
            (usable > 0).then(|| {
                effective_input_tokens
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
        let estimator_confidence =
            if unknown_token_window || zero_context_window || reserved_output_consumes_window {
                2
            } else {
                match effective_input_source {
                    0 | 2 => 0,
                    1 => 1,
                    _ => 2,
                }
            };
        let usable =
            model_context_window_tokens.map(|window| window.saturating_sub(reserved_output_tokens));
        let absolute_safety_floor_tokens = usable
            .map(|usable| (usable / 20).clamp(1_000, 8_000))
            .unwrap_or(2_000);
        Some(SmartContextPressureSnapshot {
            effective_usable_context_tokens,
            effective_used_tokens: effective_input_tokens,
            pressure_basis_points,
            pressure_band,
            absolute_safety_floor_tokens,
            estimator_confidence,
        })
    }
}

pub fn runtime_candidate_plan_batch(
    fields: &[i64],
    route_kind: i64,
) -> Option<RuntimeCandidatePlan> {
    if !fields
        .len()
        .is_multiple_of(RUNTIME_CANDIDATE_PLAN_FIELD_COUNT)
    {
        return None;
    }
    let count = fields.len() / RUNTIME_CANDIDATE_PLAN_FIELD_COUNT;
    if count > RUNTIME_CANDIDATE_PLAN_MAX_COUNT {
        return None;
    }

    #[cfg(not(prodex_mojo_fallback))]
    {
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
                i64::try_from(count).ok()?,
                route_kind,
            )
        };
        if status != 0
            || ready_count < 0
            || fallback_count < 0
            || ready_count as usize > count
            || fallback_count as usize > count
        {
            return None;
        }
        let ready_indices = match runtime_candidate_plan_indices(&ready_indices, ready_count, count)
        {
            Some(indices) => indices,
            None => {
                return None;
            }
        };
        let fallback_indices =
            match runtime_candidate_plan_indices(&fallback_indices, fallback_count, count) {
                Some(indices) => indices,
                None => {
                    return None;
                }
            };
        if fallback_indices.len() != count {
            return None;
        }
        let mut seen_ready = vec![false; count];
        for index in &ready_indices {
            if seen_ready[*index] {
                return None;
            }
            seen_ready[*index] = true;
        }
        let mut seen_fallback = vec![false; count];
        for index in &fallback_indices {
            if seen_fallback[*index] {
                return None;
            }
            seen_fallback[*index] = true;
        }
        if seen_fallback.iter().any(|value| !value) {
            return None;
        }
        Some(RuntimeCandidatePlan {
            ready_indices,
            fallback_indices,
        })
    }

    #[cfg(prodex_mojo_fallback)]
    {
        let _ = route_kind;
        let _ = fields;
        None
    }
}

#[cfg(not(prodex_mojo_fallback))]
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

#[cfg(all(test, not(prodex_mojo_fallback)))]
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
            let model_context_window_tokens = if case % 5 == 0 {
                None
            } else {
                Some(next_random(&mut state) % 200_000)
            };
            let reserved_output_tokens = if case % 7 == 0 {
                u64::MAX
            } else {
                next_random(&mut state) % 200_000
            };
            let effective_input_tokens = if case % 11 == 0 {
                u64::MAX
            } else {
                next_random(&mut state) % 400_000
            };
            let source = (case % 4) as i64;
            let unknown_window = case % 6 == 0;
            let zero_context_window = case % 9 == 0;
            let reserved_output_consumes_window = case % 8 == 0;
            let usable = model_context_window_tokens
                .and_then(|window| window.checked_sub(reserved_output_tokens));
            let pressure_basis_points = usable.and_then(|usable| {
                (usable > 0).then(|| {
                    effective_input_tokens
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
            let estimator_confidence =
                if unknown_window || zero_context_window || reserved_output_consumes_window {
                    2
                } else {
                    match source {
                        0 | 2 => 0,
                        1 => 1,
                        _ => 2,
                    }
                };
            let usable_for_floor = model_context_window_tokens
                .map(|window| window.saturating_sub(reserved_output_tokens));
            let expected = SmartContextPressureSnapshot {
                effective_usable_context_tokens: usable,
                effective_used_tokens: effective_input_tokens,
                pressure_basis_points,
                pressure_band,
                absolute_safety_floor_tokens: usable_for_floor
                    .map(|value| (value / 20).clamp(1_000, 8_000))
                    .unwrap_or(2_000),
                estimator_confidence,
            };
            let actual = smart_context_pressure_snapshot(
                model_context_window_tokens,
                reserved_output_tokens,
                effective_input_tokens,
                source,
                unknown_window,
                zero_context_window,
                reserved_output_consumes_window,
            )
            .expect("strict Mojo pressure snapshot should accept generated input");
            assert_eq!(actual, expected, "pressure case {case}");
        }
    }
}
