use super::{
    MojoError, prodex_runtime_websocket_failure_disposition_v1,
    prodex_runtime_websocket_full_context_signal_v1,
    prodex_runtime_websocket_invalid_previous_response_plan_v1,
    prodex_runtime_websocket_quota_fallback_plan_v1, prodex_runtime_websocket_response_plan_v1,
    prodex_runtime_websocket_transport_failure_plan_v1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketTransportFailurePlan {
    Error,
    ReuseWatchdog,
    RetryTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureDispositionPlan {
    pub continue_selection: bool,
    pub mark_backoff: bool,
    pub exclude_profile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketInvalidPreviousResponseAction {
    PassThrough,
    FullContextRetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketChainReuseReason {
    UpstreamReconnect,
    BoundProfileAffinity,
    UnboundPreviousResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketInvalidPreviousResponsePlan {
    pub recovery_signal: bool,
    pub crossed_transport_generation: bool,
    pub chain_reuse_reason: WebsocketChainReuseReason,
    pub action: WebsocketInvalidPreviousResponseAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketQuotaFallbackPlan {
    Ready,
    LastChance,
    Unavailable,
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
pub fn websocket_transport_failure_plan(
    committed: bool,
    reuse_existing_session: bool,
    precommit_transport_retry_allowed: bool,
) -> Result<WebsocketTransportFailurePlan, MojoError> {
    match unsafe {
        prodex_runtime_websocket_transport_failure_plan_v1(
            i64::from(committed),
            i64::from(reuse_existing_session),
            i64::from(precommit_transport_retry_allowed),
        )
    } {
        0 => Ok(WebsocketTransportFailurePlan::Error),
        1 => Ok(WebsocketTransportFailurePlan::ReuseWatchdog),
        2 => Ok(WebsocketTransportFailurePlan::RetryTransport),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn websocket_failure_disposition_plan(
    affinity_releasable: bool,
    inflight_saturated: bool,
) -> Result<WebsocketFailureDispositionPlan, MojoError> {
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_runtime_websocket_failure_disposition_v1(
            i64::from(affinity_releasable),
            i64::from(inflight_saturated),
            output.as_mut_ptr(),
        )
    };
    if status != 0 || output.iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WebsocketFailureDispositionPlan {
        continue_selection: output[0] == 1,
        mark_backoff: output[1] == 1,
        exclude_profile: output[2] == 1,
    })
}

pub fn websocket_invalid_previous_response_plan(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
    owner_generation_present: bool,
    owner_generation_matches: bool,
) -> Result<WebsocketInvalidPreviousResponsePlan, MojoError> {
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_runtime_websocket_invalid_previous_response_plan_v1(
            i64::from(previous_response_present),
            i64::from(session_present),
            i64::from(owner_matches),
            i64::from(owner_generation_present),
            i64::from(owner_generation_matches),
            output.as_mut_ptr(),
        )
    };
    if status != 0 || !matches!(output[0], 0 | 1) || !matches!(output[1], 0 | 1) {
        return Err(MojoError::InvalidOutput);
    }
    let chain_reuse_reason = match output[2] {
        0 => WebsocketChainReuseReason::UpstreamReconnect,
        1 => WebsocketChainReuseReason::BoundProfileAffinity,
        2 => WebsocketChainReuseReason::UnboundPreviousResponse,
        _ => return Err(MojoError::InvalidOutput),
    };
    let action = match output[3] {
        0 => WebsocketInvalidPreviousResponseAction::PassThrough,
        1 => WebsocketInvalidPreviousResponseAction::FullContextRetry,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(WebsocketInvalidPreviousResponsePlan {
        recovery_signal: output[0] == 1,
        crossed_transport_generation: output[1] == 1,
        chain_reuse_reason,
        action,
    })
}

pub fn websocket_full_context_signal_eligible(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_websocket_full_context_signal_v1(
            i64::from(previous_response_present),
            i64::from(session_present),
            i64::from(owner_matches),
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn websocket_quota_fallback_plan(
    route_eligible_fallback: bool,
    has_context_constraint: bool,
) -> Result<WebsocketQuotaFallbackPlan, MojoError> {
    match unsafe {
        prodex_runtime_websocket_quota_fallback_plan_v1(
            i64::from(route_eligible_fallback),
            i64::from(has_context_constraint),
        )
    } {
        0 => Ok(WebsocketQuotaFallbackPlan::Ready),
        1 => Ok(WebsocketQuotaFallbackPlan::LastChance),
        2 => Ok(WebsocketQuotaFallbackPlan::Unavailable),
        _ => Err(MojoError::InvalidOutput),
    }
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
