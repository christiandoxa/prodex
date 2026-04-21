use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePreviousResponseNotFoundRoute {
    Responses,
    Websocket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimePreviousResponseNotFoundDecision {
    pub(crate) retry_delay: Option<Duration>,
    pub(crate) retry_reason: Option<&'static str>,
    pub(crate) chain_retry_reason: Option<&'static str>,
    pub(crate) request_requires_locked_previous_response_affinity: bool,
    pub(crate) stale_continuation: bool,
    pub(crate) fresh_fallback_allowed: bool,
    pub(crate) fresh_fallback_blocked_without_affinity: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimePreviousResponseNotFoundDecisionInput<'a> {
    pub(crate) route: RuntimePreviousResponseNotFoundRoute,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) has_turn_state_retry: bool,
    pub(crate) request_requires_previous_response_affinity: bool,
    pub(crate) trusted_previous_response_affinity: bool,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) previous_response_fresh_fallback_used: bool,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(crate) retry_index: usize,
}

pub(crate) fn runtime_record_previous_response_not_found_retry_state(
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

pub(crate) fn runtime_previous_response_not_found_decision(
    input: RuntimePreviousResponseNotFoundDecisionInput<'_>,
) -> RuntimePreviousResponseNotFoundDecision {
    let session_replayable_fallback = matches!(
        input.fresh_fallback_shape,
        Some(RuntimePreviousResponseFreshFallbackShape::SessionReplayable)
    ) && input.previous_response_id.is_some()
        && !input.previous_response_fresh_fallback_used;
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
    let locked_previous_response_retry = !session_replayable_fallback
        && matches!(input.route, RuntimePreviousResponseNotFoundRoute::Websocket)
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
    let stale_continuation = !session_replayable_fallback
        && matches!(input.route, RuntimePreviousResponseNotFoundRoute::Websocket)
        && runtime_websocket_previous_response_not_found_requires_stale_continuation(
            input.previous_response_id,
            input.has_turn_state_retry,
            request_requires_locked_previous_response_affinity,
            input.fresh_fallback_shape,
        );
    let fresh_fallback_allowed = session_replayable_fallback
        || runtime_previous_response_not_found_fresh_fallback_allowed(
            input.previous_response_id,
            input.previous_response_fresh_fallback_used,
            request_requires_locked_previous_response_affinity,
            input.fresh_fallback_shape,
        );

    RuntimePreviousResponseNotFoundDecision {
        retry_delay,
        retry_reason,
        chain_retry_reason,
        request_requires_locked_previous_response_affinity,
        stale_continuation,
        fresh_fallback_allowed,
        fresh_fallback_blocked_without_affinity: !input.has_turn_state_retry
            && !fresh_fallback_allowed
            && !request_requires_locked_previous_response_affinity,
    }
}
