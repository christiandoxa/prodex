use std::time::Duration;

#[repr(i64)]
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
    FailClosed,
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
        matches!(self, Self::FailClosed)
            && !has_turn_state_retry
            && !request_requires_locked_previous_response_affinity
    }

    pub fn is_fail_closed(self) -> bool {
        matches!(self, Self::FailClosed)
    }
}

pub fn runtime_previous_response_fresh_fallback_policy(
    input: RuntimePreviousResponseFreshFallbackPolicyInput,
) -> RuntimePreviousResponseFreshFallbackPolicy {
    let plan = prodex_mojo_core::rich::previous_response_plan(
        prodex_mojo_core::rich::PreviousResponsePlanInput {
            previous_response_present: input.has_previous_response_context,
            request_requires_previous_response_affinity: input
                .request_requires_locked_previous_response_affinity,
            fresh_fallback_shape: runtime_previous_response_fallback_shape_tag(
                input.fresh_fallback_shape,
            ),
            ..Default::default()
        },
    )
    .expect("Mojo previous-response fallback planning returned an invalid result");
    if plan.fresh_fail_closed {
        RuntimePreviousResponseFreshFallbackPolicy::FailClosed
    } else {
        RuntimePreviousResponseFreshFallbackPolicy::NotApplicable
    }
}

fn runtime_previous_response_fallback_shape_tag(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> i64 {
    shape.map_or(-1, |shape| shape as i64)
}

fn runtime_previous_response_fallback_shape_from_tag(
    shape: i64,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    match shape {
        -1 => None,
        0 => Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
        1 => Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
        2 => Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        3 => Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        _ => unreachable!("validated Mojo previous-response shape"),
    }
}

pub fn runtime_previous_response_fresh_fallback_shape_label(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> &'static str {
    let value = shape.map_or(0, |shape| shape as i64 + 1);
    prodex_mojo_core::observability::runtime_previous_response_fallback_shape_label(value)
        .expect("Mojo previous-response fallback-shape label returned invalid output")
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
    let plan = prodex_mojo_core::rich::previous_response_plan(
        prodex_mojo_core::rich::PreviousResponsePlanInput {
            previous_response_present: shape.is_some(),
            fresh_fallback_shape: runtime_previous_response_fallback_shape_tag(shape),
            has_session_affinity,
            ..Default::default()
        },
    )
    .expect("Mojo previous-response shape planning returned an invalid result");
    runtime_previous_response_fallback_shape_from_tag(plan.effective_shape)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePreviousResponseNotFoundFallbackPolicy {
    pub stale_continuation: bool,
    pub fresh_fallback: RuntimePreviousResponseFreshFallbackPolicy,
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
    let plan = prodex_mojo_core::rich::previous_response_plan(
        prodex_mojo_core::rich::PreviousResponsePlanInput {
            previous_response_present: request.previous_response_id.is_some(),
            has_turn_state_retry: request.has_turn_state_retry,
            request_requires_previous_response_affinity: request
                .request_requires_locked_previous_response_affinity,
            previous_response_fresh_fallback_used: request.previous_response_fresh_fallback_used,
            fresh_fallback_shape: runtime_previous_response_fallback_shape_tag(
                request.fresh_fallback_shape,
            ),
            ..Default::default()
        },
    )
    .expect("Mojo previous-response policy planning returned an invalid result");
    RuntimePreviousResponseNotFoundFallbackPolicy {
        stale_continuation: plan.stale_policy == 2,
        fresh_fallback: if plan.fresh_fail_closed {
            RuntimePreviousResponseFreshFallbackPolicy::FailClosed
        } else {
            RuntimePreviousResponseFreshFallbackPolicy::NotApplicable
        },
    }
}

pub fn runtime_websocket_request_requires_locked_previous_response_affinity(
    request_requires_previous_response_affinity: bool,
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    prodex_mojo_core::rich::previous_response_plan(
        prodex_mojo_core::rich::PreviousResponsePlanInput {
            route: 1,
            previous_response_present: previous_response_id.is_some(),
            request_requires_previous_response_affinity,
            trusted_previous_response_affinity,
            request_turn_state_present: request_turn_state.is_some(),
            ..Default::default()
        },
    )
    .expect("Mojo websocket locked-affinity planning returned an invalid result")
    .request_requires_locked_affinity
}

#[repr(i64)]
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
    pub observability_outcome: Option<&'static str>,
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
    let plan = prodex_mojo_core::rich::previous_response_plan(
        prodex_mojo_core::rich::PreviousResponsePlanInput {
            route: input.route as i64,
            previous_response_present: input.previous_response_id.is_some(),
            has_turn_state_retry: input.has_turn_state_retry,
            request_requires_previous_response_affinity: input
                .request_requires_previous_response_affinity,
            trusted_previous_response_affinity: input.trusted_previous_response_affinity,
            request_turn_state_present: input.request_turn_state.is_some(),
            previous_response_fresh_fallback_used: input.previous_response_fresh_fallback_used,
            fresh_fallback_shape: runtime_previous_response_fallback_shape_tag(
                input.fresh_fallback_shape,
            ),
            retry_index: input.retry_index,
            ..Default::default()
        },
    )
    .expect("Mojo previous-response attempt planning returned an invalid result");
    RuntimePreviousResponseNotFoundDecision {
        retry_delay: plan.retry_delay_ms.map(Duration::from_millis),
        retry_reason: (plan.retry_reason > 0).then(|| {
            prodex_mojo_core::observability::runtime_previous_response_retry_reason_label(
                plan.retry_reason - 1,
            )
            .expect("Mojo previous-response retry-reason label returned invalid output")
        }),
        chain_retry_reason: (plan.chain_reason > 0).then(|| {
            prodex_mojo_core::observability::runtime_previous_response_chain_reason_label(
                plan.chain_reason - 1,
            )
            .expect("Mojo previous-response chain-reason label returned invalid output")
        }),
        request_requires_locked_previous_response_affinity: plan.request_requires_locked_affinity,
        stale_continuation: plan.stale_policy == 2,
        fresh_fallback_allowed: false,
        fresh_fallback_blocked_without_affinity: plan.fresh_blocked_without_affinity,
        observability_outcome: (plan.observability > 0).then(|| {
            prodex_mojo_core::observability::runtime_previous_response_outcome_label(
                plan.observability - 1,
            )
            .expect("Mojo previous-response outcome label returned invalid output")
        }),
    }
}

pub fn runtime_previous_response_not_found_observability_outcome(
    decision: RuntimePreviousResponseNotFoundDecision,
    _fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> Option<&'static str> {
    decision.observability_outcome
}

#[cfg(test)]
#[path = "../tests/src/attempt_outcome.rs"]
mod tests;
