use super::*;

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
