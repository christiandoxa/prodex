use crate::runtime_proxy::{
    RuntimePreviousResponseNotFoundAction, RuntimePreviousResponseNotFoundContext,
    RuntimePreviousResponseNotFoundPolicy,
};
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use runtime_proxy_crate::{
    RuntimePreviousResponseFreshFallbackShape, RuntimePreviousResponseLogContext,
    RuntimePreviousResponseNotFoundRoute,
};

use super::{
    RuntimePrecommitLoopState, RuntimeResponsesAffinityState, RuntimeResponsesReply,
    RuntimeResponsesRequestContext, RuntimeUpstreamFailureResponse,
    clear_runtime_dead_response_bindings, handle_runtime_previous_response_not_found,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
    runtime_responses_stale_continuation_reply,
};

pub(super) struct RuntimeResponsesPreviousResponseNotFoundContextInput<'a> {
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) request_id: u64,
    pub(super) profile_name: &'a str,
    pub(super) turn_state: Option<String>,
    pub(super) via: Option<&'a str>,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_requires_previous_response_affinity: bool,
    pub(super) trusted_previous_response_affinity: bool,
    pub(super) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(super) policy: RuntimePreviousResponseNotFoundPolicy,
}

pub(super) fn runtime_responses_previous_response_not_found_context<'a>(
    input: RuntimeResponsesPreviousResponseNotFoundContextInput<'a>,
) -> RuntimePreviousResponseNotFoundContext<'a> {
    RuntimePreviousResponseNotFoundContext {
        shared: input.shared,
        log_context: RuntimePreviousResponseLogContext {
            request_id: input.request_id,
            transport: "http",
            route: "responses",
            websocket_session: None,
            via: input.via,
        },
        route: RuntimePreviousResponseNotFoundRoute::Responses,
        route_kind: RuntimeRouteKind::Responses,
        profile_name: input.profile_name,
        turn_state: input.turn_state,
        previous_response_id: input.previous_response_id,
        request_turn_state: input.request_turn_state,
        request_session_id: input.request_session_id,
        request_requires_previous_response_affinity: input
            .request_requires_previous_response_affinity,
        trusted_previous_response_affinity: input.trusted_previous_response_affinity,
        previous_response_fresh_fallback_used: false,
        fresh_fallback_shape: input.fresh_fallback_shape,
        policy: input.policy,
    }
}

pub(super) fn handle_runtime_responses_previous_response_attempt(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    response: RuntimeResponsesReply,
    turn_state: Option<String>,
    invalid_previous_response_id: bool,
) -> anyhow::Result<Option<RuntimeResponsesReply>> {
    if invalid_previous_response_id {
        if let Some(previous_response_id) = context.previous_response_id {
            clear_runtime_dead_response_bindings(
                context.shared,
                &profile_name,
                &[previous_response_id.to_string()],
                "invalid_previous_response_id",
            )?;
        }
        runtime_proxy_log(
            context.shared,
            runtime_proxy_structured_log_message(
                "responses_invalid_previous_response_id",
                [
                    runtime_proxy_log_field("request", context.request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field(
                        "profile_hash",
                        runtime_proxy_crate::runtime_proxy_identifier_hash(Some(&profile_name)),
                    ),
                    runtime_proxy_log_field("action", "pass_through_once"),
                ],
            ),
        );
        return Ok(Some(response));
    }
    match handle_runtime_previous_response_not_found(
        runtime_responses_previous_response_not_found_context(
            RuntimeResponsesPreviousResponseNotFoundContextInput {
                shared: context.shared,
                request_id: context.request_id,
                profile_name: &profile_name,
                turn_state,
                via: None,
                previous_response_id: context.previous_response_id,
                request_turn_state: context.request_turn_state,
                request_session_id: context.request_session_id,
                request_requires_previous_response_affinity: context
                    .request_requires_previous_response_affinity,
                trusted_previous_response_affinity: affinity_state
                    .trusted_previous_response_affinity(),
                fresh_fallback_shape: context.previous_response_fresh_fallback_shape,
                policy: RuntimePreviousResponseNotFoundPolicy::responses(true),
            },
        ),
        affinity_state.previous_response_not_found_state(&mut loop_state.excluded_profiles, true),
    )? {
        RuntimePreviousResponseNotFoundAction::RetryOwner
        | RuntimePreviousResponseNotFoundAction::Rotate => {
            loop_state.last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
        }
        RuntimePreviousResponseNotFoundAction::StaleContinuation => {
            return Ok(Some(runtime_responses_stale_continuation_reply()));
        }
    }
    Ok(None)
}
