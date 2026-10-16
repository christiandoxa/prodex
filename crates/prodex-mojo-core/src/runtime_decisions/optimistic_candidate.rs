use super::{
    OptimisticCandidateDecisionPlan, OptimisticCandidateInput,
    prodex_runtime_optimistic_candidate_reason_include_quota_v1,
    prodex_runtime_optimistic_current_candidate_plan_v2,
};

pub fn optimistic_current_candidate_decision(
    input: OptimisticCandidateInput,
) -> Result<OptimisticCandidateDecisionPlan, crate::MojoError> {
    let inflight_count =
        i64::try_from(input.inflight_count).map_err(|_| crate::MojoError::InvalidInput)?;
    let inflight_soft_limit =
        i64::try_from(input.inflight_soft_limit).map_err(|_| crate::MojoError::InvalidInput)?;
    let mut output = [-2_i64; 2];
    let status = unsafe {
        prodex_runtime_optimistic_current_candidate_plan_v2(
            input.route_kind,
            i64::from(input.auth_failure_active),
            i64::from(input.in_selection_backoff),
            i64::from(input.circuit_open),
            i64::from(input.health_score),
            i64::from(input.performance_score),
            i64::from(input.current_profile_quota_compatible),
            i64::from(input.has_alternative_quota_compatible_profile),
            input.quota_band,
            i64::from(input.quota_source.is_some()),
            input.quota_source.unwrap_or(0),
            inflight_count,
            inflight_soft_limit,
            i64::from(input.prompt_cache_present),
            i64::from(input.prompt_cache_owner_matches),
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 || !matches!(output[0], 0 | 1) {
        return Err(crate::MojoError::InvalidOutput);
    }
    if output[0] == 1 {
        return (output[1] == -1)
            .then_some(OptimisticCandidateDecisionPlan {
                keep: true,
                reason_kind: None,
            })
            .ok_or(crate::MojoError::InvalidOutput);
    }
    let reason_kind = u8::try_from(output[1])
        .ok()
        .filter(|kind| *kind <= 33)
        .ok_or(crate::MojoError::InvalidOutput)?;
    Ok(OptimisticCandidateDecisionPlan {
        keep: false,
        reason_kind: Some(reason_kind),
    })
}

pub fn optimistic_candidate_reason_include_quota(
    reason_kind: u8,
) -> Result<bool, crate::MojoError> {
    match unsafe {
        prodex_runtime_optimistic_candidate_reason_include_quota_v1(i64::from(reason_kind))
    } {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
