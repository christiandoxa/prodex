use crate::MojoError;

pub const SOFT_AFFINITY_POLICY_ALLOWED: i64 = 0;
pub const SOFT_AFFINITY_POLICY_QUOTA_WINDOWS_UNAVAILABLE: i64 = 1;
pub const SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED_BEFORE_SEND: i64 = 2;
pub const SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED: i64 = 3;
pub const SOFT_AFFINITY_POLICY_QUOTA_HEALTHY: i64 = 4;
pub const SOFT_AFFINITY_POLICY_QUOTA_THIN: i64 = 5;
pub const SOFT_AFFINITY_POLICY_QUOTA_CRITICAL: i64 = 6;
pub const SOFT_AFFINITY_POLICY_QUOTA_UNKNOWN: i64 = 7;

pub const QUOTA_SELECTION_MODE_BAND_REASON: i64 = 0;
pub const QUOTA_SELECTION_MODE_PRECOMMIT_FLOOR: i64 = 1;
pub const QUOTA_SELECTION_MODE_WINDOW_GUARD: i64 = 2;
pub const QUOTA_SELECTION_MODE_PRECOMMIT_REASON: i64 = 3;
pub const QUOTA_SELECTION_MODE_WINDOW_USABLE: i64 = 4;
pub const QUOTA_SELECTION_MODE_SUMMARY_ALLOWS: i64 = 5;
pub const QUOTA_SELECTION_MODE_REJECTION_REASON: i64 = 6;

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
pub struct QuotaSelectionPolicyInput {
    pub route_kind: i64,
    pub five_hour_status: i64,
    pub weekly_status: i64,
    pub quota_band: i64,
    pub quota_source_present: bool,
    pub responses_critical_floor_percent: i64,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AffinityOutcomeInput {
    pub hard_binding_conflict: bool,
    pub exact_binding_mismatch: bool,
    pub profile_usable: bool,
    pub excluded: bool,
    pub hard_affinity: bool,
    pub soft_policy_allowed: bool,
    /// 0 = none, 1 = selection backoff, 2 = half-open probe wait.
    pub local_rejection: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffinityOutcomePlan {
    /// 0 = unavailable, 1 = select hard, 2 = select soft, 3 = reject soft quota.
    pub action: i64,
    /// Stable reason tag; zero means no unavailable reason.
    pub reason: i64,
    pub unavailable_hard: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AffinitySelectionInput {
    pub route_kind: i64,
    pub strict_candidate_match: bool,
    pub pinned_candidate_match: bool,
    pub turn_state_candidate_match: bool,
    pub session_candidate_match: bool,
    pub trusted_previous_response_affinity: bool,
    pub fresh_fallback_shape_present: bool,
    pub previous_response_present: bool,
    pub pinned_profile_present: bool,
    pub request_turn_state_present: bool,
    pub turn_state_profile_present: bool,
    pub session_profile_present: bool,
    pub saw_inflight_saturation: bool,
    pub saw_upstream_failure: bool,
    pub previous_response_fresh_fallback_used: bool,
    pub reuse_terminal_idle_ms: Option<u64>,
    pub reuse_stale_after_ms: u64,
    pub compact_session_matches_session: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffinitySelectionPlan {
    pub no_rotate_affinity: i64,
    pub release_quota_affinity: bool,
    pub continuation_priority: bool,
    pub direct_current_fallback: bool,
    pub reuse_nonreplayable: bool,
    pub reuse_stale: bool,
    pub wait_owner: i64,
    pub noncompact_session_priority: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebsocketResponsePlanInput {
    pub reuse_existing_session: bool,
    pub request_previous_response_present: bool,
    pub request_session_present: bool,
    pub request_turn_state_present: bool,
    pub turn_state_override_present: bool,
    pub promote_committed_profile: bool,
    pub bound_profile_present: bool,
    pub turn_state_profile_present: bool,
    pub compact_followup_profile_present: bool,
    pub bound_session_profile_present: bool,
    pub direct_fallback_reason: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketResponsePlan {
    pub hold_promotion_allowed: bool,
    pub transport_retry_allowed: bool,
    pub committed_profile_promotion_allowed: bool,
    pub reset_retry_index_on_local_block: bool,
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
    fn prodex_runtime_quota_selection_policy_v1(
        mode: i64,
        route_kind: i64,
        five_hour_status: i64,
        weekly_status: i64,
        quota_band: i64,
        quota_source_present: i64,
        responses_critical_floor_percent: i64,
        output: *mut i64,
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
    fn prodex_runtime_affinity_outcome_plan_v1(
        hard_binding_conflict: i64,
        exact_binding_mismatch: i64,
        profile_usable: i64,
        excluded: i64,
        hard_affinity: i64,
        soft_policy_allowed: i64,
        local_rejection: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_affinity_selection_plan_v1(
        route_kind: i64,
        strict_candidate_match: i64,
        pinned_candidate_match: i64,
        turn_state_candidate_match: i64,
        session_candidate_match: i64,
        trusted_previous_response_affinity: i64,
        fresh_fallback_shape_present: i64,
        previous_response_present: i64,
        pinned_profile_present: i64,
        request_turn_state_present: i64,
        turn_state_profile_present: i64,
        session_profile_present: i64,
        saw_inflight_saturation: i64,
        saw_upstream_failure: i64,
        previous_response_fresh_fallback_used: i64,
        reuse_terminal_idle_present: i64,
        reuse_terminal_idle_ms: u64,
        reuse_stale_after_ms: u64,
        compact_session_matches_session: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_websocket_response_plan_v1(
        reuse_existing_session: i64,
        request_previous_response_present: i64,
        request_session_present: i64,
        request_turn_state_present: i64,
        turn_state_override_present: i64,
        promote_committed_profile: i64,
        bound_profile_present: i64,
        turn_state_profile_present: i64,
        compact_followup_profile_present: i64,
        bound_session_profile_present: i64,
        direct_fallback_reason: i64,
        output: *mut i64,
    ) -> i64;
}

pub fn websocket_response_plan(
    input: WebsocketResponsePlanInput,
) -> Result<WebsocketResponsePlan, MojoError> {
    if !(0..=1).contains(&input.direct_fallback_reason) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 4];
    let status = unsafe {
        prodex_runtime_websocket_response_plan_v1(
            i64::from(input.reuse_existing_session),
            i64::from(input.request_previous_response_present),
            i64::from(input.request_session_present),
            i64::from(input.request_turn_state_present),
            i64::from(input.turn_state_override_present),
            i64::from(input.promote_committed_profile),
            i64::from(input.bound_profile_present),
            i64::from(input.turn_state_profile_present),
            i64::from(input.compact_followup_profile_present),
            i64::from(input.bound_session_profile_present),
            input.direct_fallback_reason,
            output.as_mut_ptr(),
        )
    };
    if status != 0 || output.iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WebsocketResponsePlan {
        hold_promotion_allowed: output[0] == 1,
        transport_retry_allowed: output[1] == 1,
        committed_profile_promotion_allowed: output[2] == 1,
        reset_retry_index_on_local_block: output[3] == 1,
    })
}

pub fn affinity_outcome_plan(
    input: AffinityOutcomeInput,
) -> Result<AffinityOutcomePlan, MojoError> {
    if !(0..=2).contains(&input.local_rejection) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_runtime_affinity_outcome_plan_v1(
            i64::from(input.hard_binding_conflict),
            i64::from(input.exact_binding_mismatch),
            i64::from(input.profile_usable),
            i64::from(input.excluded),
            i64::from(input.hard_affinity),
            i64::from(input.soft_policy_allowed),
            input.local_rejection,
            output.as_mut_ptr(),
        )
    };
    if status != 0
        || !(0..=3).contains(&output[0])
        || !(0..=6).contains(&output[1])
        || !matches!(output[2], 0 | 1)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(AffinityOutcomePlan {
        action: output[0],
        reason: output[1],
        unavailable_hard: output[2] == 1,
    })
}

pub fn affinity_selection_plan(
    input: AffinitySelectionInput,
) -> Result<AffinitySelectionPlan, MojoError> {
    if !(0..=3).contains(&input.route_kind) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 8];
    let status = unsafe {
        prodex_runtime_affinity_selection_plan_v1(
            input.route_kind,
            i64::from(input.strict_candidate_match),
            i64::from(input.pinned_candidate_match),
            i64::from(input.turn_state_candidate_match),
            i64::from(input.session_candidate_match),
            i64::from(input.trusted_previous_response_affinity),
            i64::from(input.fresh_fallback_shape_present),
            i64::from(input.previous_response_present),
            i64::from(input.pinned_profile_present),
            i64::from(input.request_turn_state_present),
            i64::from(input.turn_state_profile_present),
            i64::from(input.session_profile_present),
            i64::from(input.saw_inflight_saturation),
            i64::from(input.saw_upstream_failure),
            i64::from(input.previous_response_fresh_fallback_used),
            i64::from(input.reuse_terminal_idle_ms.is_some()),
            input.reuse_terminal_idle_ms.unwrap_or_default(),
            input.reuse_stale_after_ms,
            i64::from(input.compact_session_matches_session),
            output.as_mut_ptr(),
        )
    };
    if status != 0
        || !(0..=4).contains(&output[0])
        || output[1..=5].iter().any(|value| !matches!(value, 0 | 1))
        || !(0..=4).contains(&output[6])
        || !matches!(output[7], 0 | 1)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(AffinitySelectionPlan {
        no_rotate_affinity: output[0],
        release_quota_affinity: output[1] == 1,
        continuation_priority: output[2] == 1,
        direct_current_fallback: output[3] == 1,
        reuse_nonreplayable: output[4] == 1,
        reuse_stale: output[5] == 1,
        wait_owner: output[6],
        noncompact_session_priority: output[7] == 1,
    })
}

pub fn quota_selection_policy(
    mode: i64,
    input: QuotaSelectionPolicyInput,
) -> Result<i64, MojoError> {
    if !(QUOTA_SELECTION_MODE_BAND_REASON..=QUOTA_SELECTION_MODE_REJECTION_REASON).contains(&mode)
        || !(0..=3).contains(&input.route_kind)
        || !(0..=4).contains(&input.five_hour_status)
        || !(0..=4).contains(&input.weekly_status)
        || !(0..=4).contains(&input.quota_band)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_runtime_quota_selection_policy_v1(
            mode,
            input.route_kind,
            input.five_hour_status,
            input.weekly_status,
            input.quota_band,
            i64::from(input.quota_source_present),
            input.responses_critical_floor_percent,
            &mut output,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidOutput);
    }
    match mode {
        QUOTA_SELECTION_MODE_PRECOMMIT_FLOOR => Ok(output),
        QUOTA_SELECTION_MODE_WINDOW_GUARD
        | QUOTA_SELECTION_MODE_WINDOW_USABLE
        | QUOTA_SELECTION_MODE_SUMMARY_ALLOWS
            if matches!(output, 0 | 1) =>
        {
            Ok(output)
        }
        QUOTA_SELECTION_MODE_BAND_REASON
        | QUOTA_SELECTION_MODE_PRECOMMIT_REASON
        | QUOTA_SELECTION_MODE_REJECTION_REASON
            if (SOFT_AFFINITY_POLICY_ALLOWED..=SOFT_AFFINITY_POLICY_QUOTA_UNKNOWN)
                .contains(&output) =>
        {
            Ok(output)
        }
        _ => Err(MojoError::InvalidOutput),
    }
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

#[cfg(test)]
mod quota_selection_tests {
    use super::*;

    fn input(
        route_kind: i64,
        five_hour_status: i64,
        weekly_status: i64,
        quota_band: i64,
        quota_source_present: bool,
        responses_critical_floor_percent: i64,
    ) -> QuotaSelectionPolicyInput {
        QuotaSelectionPolicyInput {
            route_kind,
            five_hour_status,
            weekly_status,
            quota_band,
            quota_source_present,
            responses_critical_floor_percent,
        }
    }

    #[test]
    fn quota_selection_policy_keeps_authoritative_window_contract() {
        let healthy = input(0, 0, 0, 0, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_FLOOR, healthy).unwrap(),
            10
        );
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_SUMMARY_ALLOWS, healthy).unwrap(),
            1
        );

        let critical = input(0, 2, 0, 2, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_REASON, critical).unwrap(),
            SOFT_AFFINITY_POLICY_ALLOWED
        );

        let weekly_exhausted = input(0, 0, 3, 3, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_REJECTION_REASON, weekly_exhausted)
                .unwrap(),
            SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED
        );

        let five_hour_exhausted = input(0, 3, 0, 3, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_REASON, five_hour_exhausted)
                .unwrap(),
            SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED_BEFORE_SEND
        );

        let unknown = input(2, 4, 0, 4, false, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_REJECTION_REASON, unknown).unwrap(),
            SOFT_AFFINITY_POLICY_QUOTA_WINDOWS_UNAVAILABLE
        );

        let compact = input(1, 0, 0, 0, true, 99);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_FLOOR, compact).unwrap(),
            1
        );
    }
}
