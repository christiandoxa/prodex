use std::time::Duration;

pub const RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS: [u64; 3] = [75, 200, 500];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePreviousResponseFreshFallbackShape {
    ToolOutputOnly,
    EmptyInputOnly,
    SessionScopedFreshReplay,
    ContextDependentContinuation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePreviousResponseFreshFallbackPolicy {
    NotApplicable,
    FailClosed {
        request_shape: RuntimePreviousResponseFreshFallbackPolicyShape,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePreviousResponseFreshFallbackPolicyShape {
    Unknown,
    ToolOutputOnly,
    EmptyInputOnly,
    SessionScopedFreshReplay,
    ContextDependentContinuation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePreviousResponseFreshFallbackPolicyInput {
    pub has_previous_response_context: bool,
    pub request_requires_locked_previous_response_affinity: bool,
    pub fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

impl RuntimePreviousResponseFreshFallbackPolicy {
    pub fn allows_fresh_fallback(self) -> bool {
        false
    }

    pub fn blocks_without_affinity(
        self,
        has_turn_state_retry: bool,
        request_requires_locked_previous_response_affinity: bool,
    ) -> bool {
        matches!(self, Self::FailClosed { .. })
            && !has_turn_state_retry
            && !request_requires_locked_previous_response_affinity
    }

    pub fn is_fail_closed(self) -> bool {
        matches!(self, Self::FailClosed { .. })
    }
}

pub fn runtime_previous_response_fresh_fallback_policy(
    input: RuntimePreviousResponseFreshFallbackPolicyInput,
) -> RuntimePreviousResponseFreshFallbackPolicy {
    if !input.has_previous_response_context
        && !input.request_requires_locked_previous_response_affinity
        && input.fresh_fallback_shape.is_none()
    {
        return RuntimePreviousResponseFreshFallbackPolicy::NotApplicable;
    }

    RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
        request_shape: match input.fresh_fallback_shape {
            Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::ToolOutputOnly
            }
            Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::EmptyInputOnly
            }
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::SessionScopedFreshReplay
            }
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::ContextDependentContinuation
            }
            None => RuntimePreviousResponseFreshFallbackPolicyShape::Unknown,
        },
    }
}

pub fn runtime_previous_response_fresh_fallback_shape_label(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> &'static str {
    match shape {
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly) => "tool_output_only",
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly) => "empty_input",
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay) => {
            "session_replayable"
        }
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation) => {
            "continuation_only"
        }
        None => "none",
    }
}

pub fn runtime_previous_response_fresh_fallback_shape_allows_recovery(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: shape.is_some(),
            request_requires_locked_previous_response_affinity: false,
            fresh_fallback_shape: shape,
        },
    )
    .allows_fresh_fallback()
}

pub fn runtime_previous_response_fresh_fallback_shape_with_session(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    has_session_affinity: bool,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    if !has_session_affinity {
        return shape;
    }

    match shape {
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly) => {
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay)
        }
        other => other,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePreviousResponseNotFoundFallbackPolicy {
    pub stale_continuation: RuntimePreviousResponseStaleContinuationPolicy,
    pub fresh_fallback: RuntimePreviousResponseFreshFallbackPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePreviousResponseStaleContinuationPolicy {
    NotApplicable,
    RetryWithTurnState,
    FailClosed,
}

impl RuntimePreviousResponseStaleContinuationPolicy {
    pub fn requires_stale_continuation(self) -> bool {
        matches!(self, Self::FailClosed)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimePreviousResponseNotFoundFallbackRequest<'a> {
    pub previous_response_id: Option<&'a str>,
    pub has_turn_state_retry: bool,
    pub request_requires_locked_previous_response_affinity: bool,
    pub previous_response_fresh_fallback_used: bool,
    pub fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub fn runtime_previous_response_not_found_fallback_policy(
    request: RuntimePreviousResponseNotFoundFallbackRequest<'_>,
) -> RuntimePreviousResponseNotFoundFallbackPolicy {
    let stale_continuation = match (request.previous_response_id, request.has_turn_state_retry) {
        (Some(_), false) => RuntimePreviousResponseStaleContinuationPolicy::FailClosed,
        (Some(_), true) => RuntimePreviousResponseStaleContinuationPolicy::RetryWithTurnState,
        (None, _) => RuntimePreviousResponseStaleContinuationPolicy::NotApplicable,
    };
    let fresh_fallback = runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: request.previous_response_id.is_some()
                || request.previous_response_fresh_fallback_used,
            request_requires_locked_previous_response_affinity: request
                .request_requires_locked_previous_response_affinity,
            fresh_fallback_shape: request.fresh_fallback_shape,
        },
    );

    RuntimePreviousResponseNotFoundFallbackPolicy {
        stale_continuation,
        fresh_fallback,
    }
}

pub fn runtime_websocket_previous_response_requires_previous_response_affinity(
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    trusted_previous_response_affinity
        && previous_response_id.is_some()
        && request_turn_state.is_none()
}

pub fn runtime_websocket_request_requires_locked_previous_response_affinity(
    request_requires_previous_response_affinity: bool,
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    request_requires_previous_response_affinity
        || runtime_websocket_previous_response_requires_previous_response_affinity(
            trusted_previous_response_affinity,
            previous_response_id,
            request_turn_state,
        )
}

pub fn runtime_previous_response_retry_delay(retry_index: usize) -> Option<Duration> {
    RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
        .get(retry_index)
        .copied()
        .map(Duration::from_millis)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePreviousResponseNotFoundRoute {
    Responses,
    Websocket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePreviousResponseNotFoundDecision {
    pub retry_delay: Option<Duration>,
    pub retry_reason: Option<&'static str>,
    pub chain_retry_reason: Option<&'static str>,
    pub request_requires_locked_previous_response_affinity: bool,
    pub stale_continuation: bool,
    pub fresh_fallback_allowed: bool,
    pub fresh_fallback_blocked_without_affinity: bool,
}

#[derive(Clone, Copy)]
pub struct RuntimePreviousResponseNotFoundDecisionInput<'a> {
    pub route: RuntimePreviousResponseNotFoundRoute,
    pub previous_response_id: Option<&'a str>,
    pub has_turn_state_retry: bool,
    pub request_requires_previous_response_affinity: bool,
    pub trusted_previous_response_affinity: bool,
    pub request_turn_state: Option<&'a str>,
    pub previous_response_fresh_fallback_used: bool,
    pub fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    pub retry_index: usize,
}

pub fn runtime_record_previous_response_not_found_retry_state(
    profile_name: &str,
    turn_state: Option<String>,
    previous_response_retry_candidate: &mut Option<String>,
    previous_response_retry_index: &mut usize,
    candidate_turn_state_retry_profile: &mut Option<String>,
    candidate_turn_state_retry_value: &mut Option<String>,
) -> bool {
    if previous_response_retry_candidate.as_deref() != Some(profile_name) {
        *previous_response_retry_candidate = Some(profile_name.to_string());
        *previous_response_retry_index = 0;
    }
    let has_turn_state_retry = turn_state.is_some();
    if has_turn_state_retry {
        *candidate_turn_state_retry_profile = Some(profile_name.to_string());
        *candidate_turn_state_retry_value = turn_state;
    }
    has_turn_state_retry
}

pub fn runtime_previous_response_not_found_decision(
    input: RuntimePreviousResponseNotFoundDecisionInput<'_>,
) -> RuntimePreviousResponseNotFoundDecision {
    let request_requires_locked_previous_response_affinity = match input.route {
        RuntimePreviousResponseNotFoundRoute::Responses => {
            input.request_requires_previous_response_affinity
        }
        RuntimePreviousResponseNotFoundRoute::Websocket => {
            runtime_websocket_request_requires_locked_previous_response_affinity(
                input.request_requires_previous_response_affinity,
                input.trusted_previous_response_affinity,
                input.previous_response_id,
                input.request_turn_state,
            )
        }
    };
    let locked_previous_response_retry =
        matches!(input.route, RuntimePreviousResponseNotFoundRoute::Websocket)
            && input.request_requires_previous_response_affinity
            && !input.has_turn_state_retry;
    let retry_delay = (input.has_turn_state_retry || locked_previous_response_retry)
        .then(|| runtime_previous_response_retry_delay(input.retry_index))
        .flatten();
    let retry_reason = if input.has_turn_state_retry {
        Some("non_blocking_retry")
    } else if locked_previous_response_retry {
        Some("locked_affinity_no_turn_state")
    } else {
        None
    };
    let chain_retry_reason = match input.route {
        RuntimePreviousResponseNotFoundRoute::Responses if input.has_turn_state_retry => {
            Some("previous_response_not_found")
        }
        RuntimePreviousResponseNotFoundRoute::Websocket if locked_previous_response_retry => {
            Some("previous_response_not_found_locked_affinity")
        }
        _ => None,
    };
    let fallback_policy = runtime_previous_response_not_found_fallback_policy(
        RuntimePreviousResponseNotFoundFallbackRequest {
            previous_response_id: input.previous_response_id,
            has_turn_state_retry: input.has_turn_state_retry,
            request_requires_locked_previous_response_affinity,
            previous_response_fresh_fallback_used: input.previous_response_fresh_fallback_used,
            fresh_fallback_shape: input.fresh_fallback_shape,
        },
    );

    RuntimePreviousResponseNotFoundDecision {
        retry_delay,
        retry_reason,
        chain_retry_reason,
        request_requires_locked_previous_response_affinity,
        stale_continuation: fallback_policy
            .stale_continuation
            .requires_stale_continuation(),
        fresh_fallback_allowed: fallback_policy.fresh_fallback.allows_fresh_fallback(),
        fresh_fallback_blocked_without_affinity: fallback_policy
            .fresh_fallback
            .blocks_without_affinity(
                input.has_turn_state_retry,
                request_requires_locked_previous_response_affinity,
            ),
    }
}

pub fn runtime_previous_response_not_found_observability_outcome(
    decision: RuntimePreviousResponseNotFoundDecision,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> Option<&'static str> {
    if decision.fresh_fallback_blocked_without_affinity
        && matches!(
            fresh_fallback_shape,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
        )
    {
        Some("blocked_nonreplayable_without_affinity")
    } else if decision.fresh_fallback_blocked_without_affinity {
        Some("blocked_without_affinity")
    } else {
        None
    }
}

#[cfg(test)]
#[path = "../tests/src/attempt_outcome.rs"]
mod tests;
