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

#[cfg(any(not(feature = "mojo"), test))]
mod rust_oracle {
    use super::*;

    pub(super) fn runtime_previous_response_fresh_fallback_policy_rust(
        input: RuntimePreviousResponseFreshFallbackPolicyInput,
    ) -> RuntimePreviousResponseFreshFallbackPolicy {
        if !input.has_previous_response_context
            && !input.request_requires_locked_previous_response_affinity
            && input.fresh_fallback_shape.is_none()
        {
            return RuntimePreviousResponseFreshFallbackPolicy::NotApplicable;
        }

        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
            request_shape: runtime_previous_response_fallback_policy_shape(
                input.fresh_fallback_shape,
            ),
        }
    }

    #[cfg(not(feature = "mojo"))]
    pub(super) fn runtime_previous_response_fresh_fallback_shape_with_session_rust(
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

    pub(super) fn runtime_previous_response_not_found_fallback_policy_rust(
        request: RuntimePreviousResponseNotFoundFallbackRequest<'_>,
    ) -> RuntimePreviousResponseNotFoundFallbackPolicy {
        let stale_continuation = match (request.previous_response_id, request.has_turn_state_retry)
        {
            (Some(_), false) => RuntimePreviousResponseStaleContinuationPolicy::FailClosed,
            (Some(_), true) => RuntimePreviousResponseStaleContinuationPolicy::RetryWithTurnState,
            (None, _) => RuntimePreviousResponseStaleContinuationPolicy::NotApplicable,
        };
        let fresh_fallback = runtime_previous_response_fresh_fallback_policy_rust(
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

    pub(super) fn runtime_websocket_previous_response_requires_previous_response_affinity_rust(
        trusted_previous_response_affinity: bool,
        previous_response_id: Option<&str>,
        request_turn_state: Option<&str>,
    ) -> bool {
        trusted_previous_response_affinity
            && previous_response_id.is_some()
            && request_turn_state.is_none()
    }

    pub(super) fn runtime_websocket_request_requires_locked_previous_response_affinity_rust(
        request_requires_previous_response_affinity: bool,
        trusted_previous_response_affinity: bool,
        previous_response_id: Option<&str>,
        request_turn_state: Option<&str>,
    ) -> bool {
        request_requires_previous_response_affinity
            || runtime_websocket_previous_response_requires_previous_response_affinity_rust(
                trusted_previous_response_affinity,
                previous_response_id,
                request_turn_state,
            )
    }

    pub(super) fn runtime_previous_response_not_found_decision_rust(
        input: RuntimePreviousResponseNotFoundDecisionInput<'_>,
    ) -> RuntimePreviousResponseNotFoundDecision {
        let request_requires_locked_previous_response_affinity = match input.route {
            RuntimePreviousResponseNotFoundRoute::Responses => {
                input.request_requires_previous_response_affinity
            }
            RuntimePreviousResponseNotFoundRoute::Websocket => {
                runtime_websocket_request_requires_locked_previous_response_affinity_rust(
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
        let fallback_policy = runtime_previous_response_not_found_fallback_policy_rust(
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
}

#[cfg(all(test, feature = "mojo"))]
use rust_oracle::runtime_previous_response_not_found_decision_rust;

pub fn runtime_previous_response_fresh_fallback_policy(
    input: RuntimePreviousResponseFreshFallbackPolicyInput,
) -> RuntimePreviousResponseFreshFallbackPolicy {
    #[cfg(feature = "mojo")]
    {
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
            RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
                request_shape: runtime_previous_response_fallback_policy_shape(
                    input.fresh_fallback_shape,
                ),
            }
        } else {
            RuntimePreviousResponseFreshFallbackPolicy::NotApplicable
        }
    }

    #[cfg(not(feature = "mojo"))]
    rust_oracle::runtime_previous_response_fresh_fallback_policy_rust(input)
}

fn runtime_previous_response_fallback_policy_shape(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> RuntimePreviousResponseFreshFallbackPolicyShape {
    match shape {
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
    }
}

#[cfg(feature = "mojo")]
fn runtime_previous_response_fallback_shape_tag(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> i64 {
    match shape {
        None => -1,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly) => 0,
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly) => 1,
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay) => 2,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation) => 3,
    }
}

#[cfg(feature = "mojo")]
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
    #[cfg(feature = "mojo")]
    {
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

    #[cfg(not(feature = "mojo"))]
    rust_oracle::runtime_previous_response_fresh_fallback_shape_with_session_rust(
        shape,
        has_session_affinity,
    )
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
    #[cfg(feature = "mojo")]
    {
        let plan = prodex_mojo_core::rich::previous_response_plan(
            prodex_mojo_core::rich::PreviousResponsePlanInput {
                previous_response_present: request.previous_response_id.is_some(),
                has_turn_state_retry: request.has_turn_state_retry,
                request_requires_previous_response_affinity: request
                    .request_requires_locked_previous_response_affinity,
                previous_response_fresh_fallback_used: request
                    .previous_response_fresh_fallback_used,
                fresh_fallback_shape: runtime_previous_response_fallback_shape_tag(
                    request.fresh_fallback_shape,
                ),
                ..Default::default()
            },
        )
        .expect("Mojo previous-response policy planning returned an invalid result");
        RuntimePreviousResponseNotFoundFallbackPolicy {
            stale_continuation: runtime_previous_response_stale_policy_from_tag(plan.stale_policy),
            fresh_fallback: if plan.fresh_fail_closed {
                RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
                    request_shape: runtime_previous_response_fallback_policy_shape(
                        request.fresh_fallback_shape,
                    ),
                }
            } else {
                RuntimePreviousResponseFreshFallbackPolicy::NotApplicable
            },
        }
    }

    #[cfg(not(feature = "mojo"))]
    rust_oracle::runtime_previous_response_not_found_fallback_policy_rust(request)
}

#[cfg(feature = "mojo")]
fn runtime_previous_response_stale_policy_from_tag(
    policy: i64,
) -> RuntimePreviousResponseStaleContinuationPolicy {
    match policy {
        0 => RuntimePreviousResponseStaleContinuationPolicy::NotApplicable,
        1 => RuntimePreviousResponseStaleContinuationPolicy::RetryWithTurnState,
        2 => RuntimePreviousResponseStaleContinuationPolicy::FailClosed,
        _ => unreachable!("validated Mojo previous-response stale policy"),
    }
}

pub fn runtime_websocket_request_requires_locked_previous_response_affinity(
    request_requires_previous_response_affinity: bool,
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    #[cfg(feature = "mojo")]
    {
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

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracle::runtime_websocket_request_requires_locked_previous_response_affinity_rust(
            request_requires_previous_response_affinity,
            trusted_previous_response_affinity,
            previous_response_id,
            request_turn_state,
        )
    }
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
    #[cfg(feature = "mojo")]
    {
        let plan = prodex_mojo_core::rich::previous_response_plan(
            prodex_mojo_core::rich::PreviousResponsePlanInput {
                route: match input.route {
                    RuntimePreviousResponseNotFoundRoute::Responses => 0,
                    RuntimePreviousResponseNotFoundRoute::Websocket => 1,
                },
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
            retry_reason: match plan.retry_reason {
                0 => None,
                1 => Some("non_blocking_retry"),
                2 => Some("locked_affinity_no_turn_state"),
                _ => unreachable!("validated Mojo previous-response retry reason"),
            },
            chain_retry_reason: match plan.chain_reason {
                0 => None,
                1 => Some("previous_response_not_found"),
                2 => Some("previous_response_not_found_locked_affinity"),
                _ => unreachable!("validated Mojo previous-response chain reason"),
            },
            request_requires_locked_previous_response_affinity: plan
                .request_requires_locked_affinity,
            stale_continuation: plan.stale_policy == 2,
            fresh_fallback_allowed: false,
            fresh_fallback_blocked_without_affinity: plan.fresh_blocked_without_affinity,
        }
    }

    #[cfg(not(feature = "mojo"))]
    rust_oracle::runtime_previous_response_not_found_decision_rust(input)
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
