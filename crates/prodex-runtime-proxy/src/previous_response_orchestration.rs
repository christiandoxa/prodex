use crate::{
    RuntimePreviousResponseFreshFallbackShape, RuntimePreviousResponseNotFoundDecision,
    RuntimePreviousResponseNotFoundDecisionInput, RuntimePreviousResponseNotFoundRoute,
    runtime_previous_response_not_found_decision,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePreviousResponseNotFoundAction {
    RetryOwner,
    StaleContinuation,
    Rotate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePreviousResponseNotFoundPolicy {
    pub reset_previous_response_retry_index_on_rotate: bool,
    pub log_fresh_fallback_blocked: bool,
    pub fail_stale_continuation: bool,
    pub clear_trusted_affinity_on_rotate: bool,
}

impl RuntimePreviousResponseNotFoundPolicy {
    pub fn websocket(
        reset_previous_response_retry_index_on_rotate: bool,
        clear_trusted_affinity_on_rotate: bool,
    ) -> Self {
        Self {
            reset_previous_response_retry_index_on_rotate,
            log_fresh_fallback_blocked: true,
            fail_stale_continuation: true,
            clear_trusted_affinity_on_rotate,
        }
    }

    pub fn responses(clear_trusted_affinity_on_rotate: bool) -> Self {
        Self {
            reset_previous_response_retry_index_on_rotate: false,
            log_fresh_fallback_blocked: true,
            fail_stale_continuation: false,
            clear_trusted_affinity_on_rotate,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RuntimePreviousResponseNotFoundPlanInput<'a> {
    pub route: RuntimePreviousResponseNotFoundRoute,
    pub previous_response_id: Option<&'a str>,
    pub has_turn_state_retry: bool,
    pub request_requires_previous_response_affinity: bool,
    pub trusted_previous_response_affinity: bool,
    pub request_turn_state: Option<&'a str>,
    pub previous_response_fresh_fallback_used: bool,
    pub fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    pub retry_index: usize,
    pub policy: RuntimePreviousResponseNotFoundPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePreviousResponseNotFoundPlan {
    pub action: RuntimePreviousResponseNotFoundAction,
    pub decision: RuntimePreviousResponseNotFoundDecision,
    pub next_retry_index: usize,
    pub reset_retry_state: bool,
    pub log_fresh_fallback_blocked: bool,
    pub release_affinity: bool,
    pub clear_response_profile_affinity: bool,
    pub clear_trusted_affinity: bool,
}

pub fn runtime_previous_response_not_found_plan(
    input: RuntimePreviousResponseNotFoundPlanInput<'_>,
) -> RuntimePreviousResponseNotFoundPlan {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: input.route,
            previous_response_id: input.previous_response_id,
            has_turn_state_retry: input.has_turn_state_retry,
            request_requires_previous_response_affinity: input
                .request_requires_previous_response_affinity,
            trusted_previous_response_affinity: input.trusted_previous_response_affinity,
            request_turn_state: input.request_turn_state,
            previous_response_fresh_fallback_used: input.previous_response_fresh_fallback_used,
            fresh_fallback_shape: input.fresh_fallback_shape,
            retry_index: input.retry_index,
        },
    );

    if decision.retry_delay.is_some() {
        return RuntimePreviousResponseNotFoundPlan {
            action: RuntimePreviousResponseNotFoundAction::RetryOwner,
            decision,
            next_retry_index: input.retry_index + 1,
            reset_retry_state: false,
            log_fresh_fallback_blocked: false,
            release_affinity: false,
            clear_response_profile_affinity: false,
            clear_trusted_affinity: false,
        };
    }

    if input.policy.fail_stale_continuation && decision.stale_continuation {
        return RuntimePreviousResponseNotFoundPlan {
            action: RuntimePreviousResponseNotFoundAction::StaleContinuation,
            decision,
            next_retry_index: 0,
            reset_retry_state: true,
            log_fresh_fallback_blocked: false,
            release_affinity: false,
            clear_response_profile_affinity: false,
            clear_trusted_affinity: false,
        };
    }

    RuntimePreviousResponseNotFoundPlan {
        action: RuntimePreviousResponseNotFoundAction::Rotate,
        decision,
        next_retry_index: 0,
        reset_retry_state: true,
        log_fresh_fallback_blocked: input.policy.log_fresh_fallback_blocked
            && decision.fresh_fallback_blocked_without_affinity,
        release_affinity: true,
        clear_response_profile_affinity: true,
        clear_trusted_affinity: input.policy.clear_trusted_affinity_on_rotate,
    }
}

#[cfg(test)]
#[path = "../tests/src/previous_response_orchestration.rs"]
mod tests;
