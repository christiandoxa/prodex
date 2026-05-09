use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_responses_previous_response_not_found_context<'a>(
    shared: &'a RuntimeRotationProxyShared,
    request_id: u64,
    profile_name: &'a str,
    turn_state: Option<String>,
    via: Option<&'a str>,
    previous_response_id: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_requires_previous_response_affinity: bool,
    trusted_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    policy: RuntimePreviousResponseNotFoundPolicy,
) -> RuntimePreviousResponseNotFoundContext<'a> {
    RuntimePreviousResponseNotFoundContext {
        shared,
        log_context: RuntimePreviousResponseLogContext {
            request_id,
            transport: "http",
            route: "responses",
            websocket_session: None,
            via,
        },
        route: RuntimePreviousResponseNotFoundRoute::Responses,
        route_kind: RuntimeRouteKind::Responses,
        profile_name,
        turn_state,
        previous_response_id,
        request_turn_state,
        request_session_id,
        request_requires_previous_response_affinity,
        trusted_previous_response_affinity,
        previous_response_fresh_fallback_used: false,
        fresh_fallback_shape,
        policy,
    }
}
