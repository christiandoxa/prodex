#[cfg(feature = "mojo-runtime")]
pub use crate::runtime_decisions::*;

mod auto_redeem;
mod candidate_plan;
mod profile_rotation;
mod quota_route_score;
pub use auto_redeem::{
    AutoRedeemCandidateInput, RUNTIME_AUTO_REDEEM_PLAN_MAX_COUNT, auto_redeem_plan_batch,
    auto_redeem_plan_self_test,
};
pub use profile_rotation::profile_selection_order_batch;
pub use quota_route_score::quota_route_score_batch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileScoreInput {
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub scale_bps: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub windows_complete: bool,
    pub weekly_weight: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileScheduleInput {
    pub score: ProfileScoreInput,
    pub provider_priority: i64,
    pub in_selection_cooldown: bool,
    pub last_selected_at: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
    pub quota_source: i64,
    pub preferred: bool,
    pub affinity_preferred: bool,
    pub order_index: i64,
}

pub const RUNTIME_PROFILE_SCHEDULE_FIELD_COUNT: usize = 16;
pub const RUNTIME_PROFILE_SCHEDULE_MAX_COUNT: usize = 256;

pub fn provider_aware_profile_order_batch(
    provider_priorities: &[usize],
    order_indices: &[usize],
) -> Result<Vec<usize>, crate::MojoError> {
    if provider_priorities.len() != order_indices.len()
        || provider_priorities.len() > RUNTIME_PROFILE_SCHEDULE_MAX_COUNT
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let priorities = provider_priorities
        .iter()
        .map(|priority| i64::try_from(*priority).unwrap_or(i64::MAX))
        .collect::<Vec<_>>();
    let indices = order_indices
        .iter()
        .map(|index| i64::try_from(*index).unwrap_or(i64::MAX))
        .collect::<Vec<_>>();
    let mut ordered = vec![0_i64; indices.len()];
    let status = unsafe {
        prodex_runtime_profile_provider_order_batch(
            priorities.as_ptr(),
            indices.as_ptr(),
            ordered.as_mut_ptr(),
            i64::try_from(indices.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    let mut seen = vec![false; priorities.len()];
    ordered
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index)
                .ok()
                .filter(|index| *index < priorities.len())
                .ok_or(crate::MojoError::InvalidOutput)?;
            if seen[index] {
                return Err(crate::MojoError::InvalidOutput);
            }
            seen[index] = true;
            Ok(index)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaScoreInput {
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_has_value: bool,
    pub five_hour_has_value: bool,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaScore {
    pub pressure_band: i64,
    pub total_pressure: i64,
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub reserve_floor: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

pub const RUNTIME_QUOTA_SCORE_FIELD_COUNT: usize = 8;
pub const RUNTIME_QUOTA_SCORE_MAX_COUNT: usize = 256;

/// Raw route-scoring observations resolved by the route-selection adapter.
///
/// Unlike [`QuotaScoreInput`], this contract includes plan scaling and keeps
/// window presence explicit so route selection can distinguish an incomplete
/// observation from a healthy complete pair. The Mojo implementation then
/// shares the same score arithmetic as `quota_score_batch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaRouteScoreInput {
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub scale_bps: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_has_value: bool,
    pub five_hour_has_value: bool,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

pub const RUNTIME_QUOTA_ROUTE_SCORE_FIELD_COUNT: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextPressureSnapshot {
    pub effective_usable_context_tokens: Option<u64>,
    pub effective_used_tokens: u64,
    pub pressure_basis_points: Option<u32>,
    pub pressure_band: i64,
    pub absolute_safety_floor_tokens: u64,
    pub estimator_confidence: i64,
}

pub const RUNTIME_CANDIDATE_PLAN_FIELD_COUNT: usize = 24;
pub const RUNTIME_CANDIDATE_PLAN_MAX_COUNT: usize = 256;
pub const RUNTIME_CANDIDATE_DECISION_FIELD_COUNT: usize = 5;

pub const RUNTIME_CANDIDATE_AVAILABILITY_READY: i64 = 0;
pub const RUNTIME_CANDIDATE_AVAILABILITY_QUOTA_EXHAUSTED: i64 = 1;
pub const RUNTIME_CANDIDATE_AVAILABILITY_TRANSIENT_BACKOFF: i64 = 2;
pub const RUNTIME_CANDIDATE_AVAILABILITY_AUTH_INVALID: i64 = 3;
pub const RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN: i64 = 4;

pub const RUNTIME_CANDIDATE_SKIP_NONE: i64 = 0;
pub const RUNTIME_CANDIDATE_SKIP_AUTH_FAILURE: i64 = 1;
pub const RUNTIME_CANDIDATE_SKIP_SELECTION_BACKOFF: i64 = 2;
pub const RUNTIME_CANDIDATE_SKIP_QUOTA_EXHAUSTED: i64 = 3;
pub const RUNTIME_CANDIDATE_SKIP_QUOTA_CRITICAL_FLOOR: i64 = 4;
pub const RUNTIME_CANDIDATE_SKIP_INFLIGHT: i64 = 5;
pub const RUNTIME_CANDIDATE_SKIP_EXCLUDED: i64 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCandidateDecision {
    pub eligible: bool,
    pub availability: i64,
    pub quota_guard_reason: i64,
    pub ready_skip_reason: i64,
    pub fallback_skip_reason: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCandidatePlan {
    pub ready_indices: Vec<usize>,
    pub fallback_indices: Vec<usize>,
    pub decisions: Vec<RuntimeCandidateDecision>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RuntimeStringView {
    ptr: u64,
    len: u64,
}

// This borrowed view intentionally matches the rich ABI v6 string-view layout.
const _: () = {
    assert!(std::mem::size_of::<RuntimeStringView>() == 16);
    assert!(std::mem::align_of::<RuntimeStringView>() == 8);
    assert!(std::mem::offset_of!(RuntimeStringView, ptr) == 0);
    assert!(std::mem::offset_of!(RuntimeStringView, len) == 8);
};

pub fn candidate_plan_self_test() -> bool {
    let mut fields = vec![0_i64; RUNTIME_CANDIDATE_PLAN_FIELD_COUNT * 2];
    fields[1] = 1;
    fields[RUNTIME_CANDIDATE_PLAN_FIELD_COUNT + 16] = 1;
    let excluded = [0_i64, 0];
    runtime_candidate_plan_batch(&fields, &excluded, 0, 3, 2).is_ok_and(|plan| {
        plan.ready_indices == [1, 0]
            && plan.fallback_indices == [1, 0]
            && plan.decisions.iter().all(|decision| {
                decision.eligible
                    && decision.availability == RUNTIME_CANDIDATE_AVAILABILITY_READY
                    && decision.ready_skip_reason == RUNTIME_CANDIDATE_SKIP_NONE
                    && decision.fallback_skip_reason == RUNTIME_CANDIDATE_SKIP_NONE
            })
    })
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

pub fn quota_score_self_test() -> bool {
    quota_score_batch(
        &[QuotaScoreInput {
            weekly_pressure: 100,
            five_hour_pressure: 200,
            weekly_remaining: 80,
            five_hour_remaining: 90,
            weekly_has_value: true,
            five_hour_has_value: true,
            weekly_reset_at: 10,
            five_hour_reset_at: 20,
        }],
        0,
    )
    .is_ok_and(|scores| {
        scores
            == [QuotaScore {
                pressure_band: 0,
                total_pressure: 1_200,
                weekly_pressure: 100,
                five_hour_pressure: 200,
                reserve_floor: 80,
                weekly_remaining: 80,
                five_hour_remaining: 90,
                weekly_reset_at: 10,
                five_hour_reset_at: 20,
            }]
    })
}

unsafe extern "C" {
    fn prodex_runtime_quota_pressure_band_for_route(
        five_hour_remaining_percent: i64,
        five_hour_has_value: i64,
        weekly_remaining_percent: i64,
        weekly_has_value: i64,
        route_kind: i64,
    ) -> i64;
    fn prodex_runtime_quota_profile_schedule_batch(
        fields: *const i64,
        total_pressure: *mut i64,
        scaled_weekly_pressure: *mut i64,
        scaled_five_hour_pressure: *mut i64,
        reserve_floor: *mut i64,
        ordered_indices: *mut i64,
        ordered_count: *mut i64,
        count: i64,
    ) -> i64;
    fn prodex_runtime_profile_provider_order_batch(
        priorities: *const i64,
        order_indices: *const i64,
        ordered_indices: *mut i64,
        count: i64,
    ) -> i64;
    fn prodex_runtime_quota_score_batch(
        fields_address: u64,
        pressure_band_address: u64,
        total_pressure_address: u64,
        weekly_pressure_address: u64,
        five_hour_pressure_address: u64,
        reserve_floor_address: u64,
        weekly_remaining_address: u64,
        five_hour_remaining_address: u64,
        weekly_reset_at_address: u64,
        five_hour_reset_at_address: u64,
        count: i64,
        route_kind: i64,
    ) -> i64;
    fn prodex_runtime_quota_route_score_resolution_batch(
        fields_address: u64,
        pressure_band_address: u64,
        total_pressure_address: u64,
        weekly_pressure_address: u64,
        five_hour_pressure_address: u64,
        reserve_floor_address: u64,
        weekly_remaining_address: u64,
        five_hour_remaining_address: u64,
        weekly_reset_at_address: u64,
        five_hour_reset_at_address: u64,
        count: i64,
        route_kind: i64,
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
        excluded: *const i64,
        decision_tags: *mut i64,
        ready_indices: *mut i64,
        ready_count: *mut i64,
        fallback_indices: *mut i64,
        fallback_count: *mut i64,
        count: i64,
        route_kind: i64,
        inflight_soft_limit: i64,
        responses_critical_floor_percent: i64,
    ) -> i64;
    fn prodex_runtime_prompt_cache_affinity_batch_v1(
        profile_views: u64,
        key_view: u64,
        key_present: i64,
        owner_view: u64,
        owner_present: i64,
        priorities: u64,
        scores: u64,
        count: i64,
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

pub fn quota_score_batch(
    inputs: &[QuotaScoreInput],
    route_kind: i64,
) -> Result<Vec<QuotaScore>, crate::MojoError> {
    if inputs.len() > RUNTIME_QUOTA_SCORE_MAX_COUNT
        || !(0..=3).contains(&route_kind)
        || inputs.iter().any(|input| {
            input.weekly_pressure < 0
                || input.five_hour_pressure < 0
                || input.weekly_remaining < 0
                || input.weekly_remaining > 100
                || input.five_hour_remaining < 0
                || input.five_hour_remaining > 100
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }

    let mut fields = Vec::with_capacity(inputs.len() * RUNTIME_QUOTA_SCORE_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            input.weekly_pressure,
            input.five_hour_pressure,
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
        prodex_runtime_quota_score_batch(
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

pub fn profile_schedule_self_test() -> bool {
    profile_schedule_batch(&[
        ProfileScheduleInput {
            score: ProfileScoreInput {
                weekly_pressure: 100,
                five_hour_pressure: 100,
                scale_bps: 10_000,
                weekly_remaining: 90,
                five_hour_remaining: 90,
                windows_complete: true,
                weekly_weight: 10,
            },
            provider_priority: 0,
            in_selection_cooldown: false,
            last_selected_at: i64::MIN,
            weekly_reset_at: 10,
            five_hour_reset_at: 20,
            quota_source: 0,
            preferred: false,
            affinity_preferred: false,
            order_index: 0,
        },
        ProfileScheduleInput {
            score: ProfileScoreInput {
                weekly_pressure: 200,
                five_hour_pressure: 200,
                scale_bps: 10_000,
                weekly_remaining: 80,
                five_hour_remaining: 80,
                windows_complete: true,
                weekly_weight: 10,
            },
            provider_priority: 1,
            in_selection_cooldown: false,
            last_selected_at: i64::MIN,
            weekly_reset_at: 10,
            five_hour_reset_at: 20,
            quota_source: 0,
            preferred: false,
            affinity_preferred: false,
            order_index: 1,
        },
    ])
    .is_ok_and(|order| order == [0, 1])
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
    excluded: &[i64],
    route_kind: i64,
    inflight_soft_limit: usize,
    responses_critical_floor_percent: i64,
) -> Result<RuntimeCandidatePlan, crate::MojoError> {
    let count = candidate_plan::input_count(fields, excluded, route_kind)?;
    let inflight_soft_limit =
        i64::try_from(inflight_soft_limit).map_err(|_| crate::MojoError::InvalidInput)?;
    let mut decision_tags = vec![0_i64; count * RUNTIME_CANDIDATE_DECISION_FIELD_COUNT];
    let mut ready_indices = vec![0_i64; count];
    let mut fallback_indices = vec![0_i64; count];
    let mut ready_count = 0_i64;
    let mut fallback_count = 0_i64;
    let status = unsafe {
        prodex_runtime_candidate_plan_batch(
            fields.as_ptr(),
            excluded.as_ptr(),
            decision_tags.as_mut_ptr(),
            ready_indices.as_mut_ptr(),
            &mut ready_count,
            fallback_indices.as_mut_ptr(),
            &mut fallback_count,
            i64::try_from(count).map_err(|_| crate::MojoError::InvalidInput)?,
            route_kind,
            inflight_soft_limit,
            responses_critical_floor_percent,
        )
    };
    let (ready_indices, fallback_indices, decisions) = candidate_plan::output(
        status,
        ready_count,
        fallback_count,
        &ready_indices,
        &fallback_indices,
        &decision_tags,
        count,
    )?;
    candidate_plan::validate(fields, &ready_indices, &fallback_indices, &decisions)?;
    Ok(RuntimeCandidatePlan {
        ready_indices,
        fallback_indices,
        decisions,
    })
}

pub fn prompt_cache_affinity_batch(
    prompt_cache_key: Option<&str>,
    prompt_cache_owner_profile: Option<&str>,
    profiles: &[&str],
) -> Result<Vec<(u8, u64)>, crate::MojoError> {
    if profiles.len() > RUNTIME_CANDIDATE_PLAN_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    if profiles.is_empty() {
        return Ok(Vec::new());
    }
    let profile_views = profiles
        .iter()
        .map(|profile| RuntimeStringView {
            ptr: profile.as_ptr() as usize as u64,
            len: profile.len() as u64,
        })
        .collect::<Vec<_>>();
    let key_view = prompt_cache_key.map(|value| RuntimeStringView {
        ptr: value.as_ptr() as usize as u64,
        len: u64::try_from(value.len()).unwrap_or(u64::MAX),
    });
    let owner_view = prompt_cache_owner_profile.map(|value| RuntimeStringView {
        ptr: value.as_ptr() as usize as u64,
        len: u64::try_from(value.len()).unwrap_or(u64::MAX),
    });
    let mut priorities = vec![0_i64; profiles.len()];
    let mut scores = vec![0_u64; profiles.len()];
    let status = unsafe {
        prodex_runtime_prompt_cache_affinity_batch_v1(
            profile_views.as_ptr() as usize as u64,
            key_view
                .as_ref()
                .map_or(0, |view| view as *const RuntimeStringView as usize as u64),
            i64::from(key_view.is_some()),
            owner_view
                .as_ref()
                .map_or(0, |view| view as *const RuntimeStringView as usize as u64),
            i64::from(owner_view.is_some()),
            priorities.as_mut_ptr() as usize as u64,
            scores.as_mut_ptr() as usize as u64,
            i64::try_from(profiles.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || priorities.iter().any(|priority| !matches!(priority, 0 | 1)) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(priorities
        .into_iter()
        .zip(scores)
        .map(|(priority, score)| (u8::try_from(priority).unwrap_or(u8::MAX), score))
        .collect())
}
