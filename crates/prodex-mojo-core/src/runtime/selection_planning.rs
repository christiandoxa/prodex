use crate::MojoError;

pub const SOFT_AFFINITY_POLICY_ALLOWED: i64 = 0;
pub const SOFT_AFFINITY_POLICY_QUOTA_WINDOWS_UNAVAILABLE: i64 = 1;
pub const SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED_BEFORE_SEND: i64 = 2;
pub const SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED: i64 = 3;
pub const SOFT_AFFINITY_POLICY_QUOTA_HEALTHY: i64 = 4;
pub const SOFT_AFFINITY_POLICY_QUOTA_THIN: i64 = 5;
pub const SOFT_AFFINITY_POLICY_QUOTA_CRITICAL: i64 = 6;
pub const SOFT_AFFINITY_POLICY_QUOTA_UNKNOWN: i64 = 7;

pub const ADAPTIVE_QUALITY_FIELD_COUNT: usize = 9;
pub const ADAPTIVE_ROUTING_MAX_COUNT: usize = 256;
pub const ADAPTIVE_PLAN_REASON_INSUFFICIENT_SAMPLES: i64 = 0;
pub const ADAPTIVE_PLAN_REASON_SHADOW_ONLY: i64 = 1;
pub const ADAPTIVE_PLAN_REASON_ADAPTIVE_ENABLED: i64 = 2;
pub const ADAPTIVE_PLAN_REASON_SHADOW_EXPLORATION: i64 = 3;
pub const ADAPTIVE_PLAN_REASON_ADAPTIVE_EXPLORATION: i64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftAffinityPolicyInput {
    pub affinity_kind: i64,
    pub route_kind: i64,
    pub five_hour_status: i64,
    pub weekly_status: i64,
    pub quota_band: i64,
    pub quota_source_present: bool,
    pub current_profile_matches_candidate: bool,
    pub has_route_eligible_quota_fallback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveQualityInput {
    pub has_window: bool,
    pub samples: u64,
    pub task_completed: u64,
    pub corrective_user_messages: u64,
    pub additional_turns: u64,
    pub previous_response_not_found: u64,
    pub invalid_tool_call_continuation: u64,
    pub errors: u64,
    pub token_savings: u64,
    pub latency_ms_total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveRoutingPlan {
    pub recommended_index: Option<usize>,
    pub quality_score_bps: Option<i64>,
    pub reason: i64,
}

unsafe extern "C" {
    fn prodex_runtime_soft_affinity_policy_v1(
        affinity_kind: i64,
        route_kind: i64,
        five_hour_status: i64,
        weekly_status: i64,
        quota_band: i64,
        quota_source_present: i64,
        current_profile_matches_candidate: i64,
        has_route_eligible_quota_fallback: i64,
    ) -> i64;
    fn prodex_runtime_gateway_adaptive_plan_v1(
        quality_fields: *const u64,
        window_present: *const i64,
        recommended_index: *mut i64,
        quality_score_bps: *mut i64,
        quality_score_present: *mut i64,
        reason: *mut i64,
        count: i64,
        actual_index: i64,
        shadow_mode: i64,
        min_samples: u64,
        exploration_rate_bps: i64,
        diagnostic_seed: u64,
    ) -> i64;
}

pub fn soft_affinity_policy(input: SoftAffinityPolicyInput) -> Result<i64, MojoError> {
    if !(0..=3).contains(&input.affinity_kind)
        || !(0..=3).contains(&input.route_kind)
        || !(0..=4).contains(&input.five_hour_status)
        || !(0..=4).contains(&input.weekly_status)
        || !(0..=4).contains(&input.quota_band)
    {
        return Err(MojoError::InvalidInput);
    }
    let result = unsafe {
        prodex_runtime_soft_affinity_policy_v1(
            input.affinity_kind,
            input.route_kind,
            input.five_hour_status,
            input.weekly_status,
            input.quota_band,
            i64::from(input.quota_source_present),
            i64::from(input.current_profile_matches_candidate),
            i64::from(input.has_route_eligible_quota_fallback),
        )
    };
    (SOFT_AFFINITY_POLICY_ALLOWED..=SOFT_AFFINITY_POLICY_QUOTA_UNKNOWN)
        .contains(&result)
        .then_some(result)
        .ok_or(MojoError::InvalidOutput)
}

pub fn adaptive_routing_plan(
    inputs: &[AdaptiveQualityInput],
    actual_index: Option<usize>,
    shadow_mode: bool,
    min_samples: u64,
    exploration_rate_bps: u16,
    diagnostic_seed: u64,
) -> Result<AdaptiveRoutingPlan, MojoError> {
    if inputs.len() > ADAPTIVE_ROUTING_MAX_COUNT {
        return Err(MojoError::InvalidInput);
    }
    let actual_index = actual_index
        .map(|index| i64::try_from(index).map_err(|_| MojoError::InvalidInput))
        .transpose()?
        .unwrap_or(-1);
    let mut quality_fields = Vec::with_capacity(inputs.len() * ADAPTIVE_QUALITY_FIELD_COUNT);
    let mut window_present = Vec::with_capacity(inputs.len());
    for input in inputs {
        window_present.push(i64::from(input.has_window));
        quality_fields.extend([
            input.samples,
            input.task_completed,
            input.corrective_user_messages,
            input.additional_turns,
            input.previous_response_not_found,
            input.invalid_tool_call_continuation,
            input.errors,
            input.token_savings,
            input.latency_ms_total,
        ]);
    }
    let mut recommended_index = -1;
    let mut quality_score_bps = 0;
    let mut quality_score_present = 0;
    let mut reason = -1;
    let status = unsafe {
        prodex_runtime_gateway_adaptive_plan_v1(
            quality_fields.as_ptr(),
            window_present.as_ptr(),
            &mut recommended_index,
            &mut quality_score_bps,
            &mut quality_score_present,
            &mut reason,
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
            actual_index,
            i64::from(shadow_mode),
            min_samples,
            i64::from(exploration_rate_bps),
            diagnostic_seed,
        )
    };
    if status != 0
        || !matches!(quality_score_present, 0 | 1)
        || !(ADAPTIVE_PLAN_REASON_INSUFFICIENT_SAMPLES..=ADAPTIVE_PLAN_REASON_ADAPTIVE_EXPLORATION)
            .contains(&reason)
    {
        return Err(MojoError::InvalidOutput);
    }
    let recommended_index = if recommended_index == -1 {
        None
    } else {
        Some(
            usize::try_from(recommended_index)
                .ok()
                .filter(|index| *index < inputs.len())
                .ok_or(MojoError::InvalidOutput)?,
        )
    };
    Ok(AdaptiveRoutingPlan {
        recommended_index,
        quality_score_bps: (quality_score_present == 1).then_some(quality_score_bps),
        reason,
    })
}
