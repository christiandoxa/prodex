use super::*;

pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseNotFoundAction, RuntimePreviousResponseNotFoundPolicy,
};

pub(crate) struct RuntimePreviousResponseNotFoundContext<'a> {
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) log_context: RuntimePreviousResponseLogContext<'a>,
    pub(crate) route: RuntimePreviousResponseNotFoundRoute,
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) profile_name: &'a str,
    pub(crate) turn_state: Option<String>,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) request_session_id: Option<&'a str>,
    pub(crate) request_requires_previous_response_affinity: bool,
    pub(crate) trusted_previous_response_affinity: bool,
    pub(crate) previous_response_fresh_fallback_used: bool,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(crate) policy: RuntimePreviousResponseNotFoundPolicy,
}

pub(crate) struct RuntimePreviousResponseNotFoundState<'a> {
    pub(crate) saw_previous_response_not_found: &'a mut bool,
    pub(crate) previous_response_retry_candidate: &'a mut Option<String>,
    pub(crate) previous_response_retry_index: &'a mut usize,
    pub(crate) candidate_turn_state_retry_profile: &'a mut Option<String>,
    pub(crate) candidate_turn_state_retry_value: &'a mut Option<String>,
    pub(crate) bound_profile: &'a mut Option<String>,
    pub(crate) session_profile: &'a mut Option<String>,
    pub(crate) pinned_profile: &'a mut Option<String>,
    pub(crate) turn_state_profile: &'a mut Option<String>,
    pub(crate) compact_followup_profile: Option<&'a mut Option<(String, &'static str)>>,
    pub(crate) excluded_profiles: &'a mut BTreeSet<String>,
    pub(crate) trusted_previous_response_affinity: Option<&'a mut bool>,
}

pub(crate) fn handle_runtime_previous_response_not_found(
    context: RuntimePreviousResponseNotFoundContext<'_>,
    state: RuntimePreviousResponseNotFoundState<'_>,
) -> Result<RuntimePreviousResponseNotFoundAction> {
    let RuntimePreviousResponseNotFoundContext {
        shared,
        log_context,
        route,
        route_kind,
        profile_name,
        turn_state,
        previous_response_id,
        request_turn_state,
        request_session_id,
        request_requires_previous_response_affinity,
        trusted_previous_response_affinity,
        previous_response_fresh_fallback_used,
        fresh_fallback_shape,
        policy,
    } = context;
    let RuntimePreviousResponseNotFoundState {
        saw_previous_response_not_found,
        previous_response_retry_candidate,
        previous_response_retry_index,
        candidate_turn_state_retry_profile,
        candidate_turn_state_retry_value,
        bound_profile,
        session_profile,
        pinned_profile,
        turn_state_profile,
        compact_followup_profile,
        excluded_profiles,
        trusted_previous_response_affinity: trusted_previous_response_affinity_mut,
    } = state;

    runtime_proxy_log(
        shared,
        runtime_previous_response_not_found_log_message(
            log_context,
            profile_name,
            *previous_response_retry_index,
            turn_state.as_deref(),
        ),
    );
    *saw_previous_response_not_found = true;

    let has_turn_state_retry = runtime_record_previous_response_not_found_retry_state(
        profile_name,
        turn_state,
        previous_response_retry_candidate,
        previous_response_retry_index,
        candidate_turn_state_retry_profile,
        candidate_turn_state_retry_value,
    );
    let plan = runtime_proxy_crate::runtime_previous_response_not_found_plan(
        runtime_proxy_crate::RuntimePreviousResponseNotFoundPlanInput {
            route,
            previous_response_id,
            has_turn_state_retry,
            request_requires_previous_response_affinity,
            trusted_previous_response_affinity,
            request_turn_state,
            previous_response_fresh_fallback_used,
            fresh_fallback_shape,
            retry_index: *previous_response_retry_index,
            policy,
        },
    );
    let decision = plan.decision;

    if let Some(delay) = decision.retry_delay {
        *previous_response_retry_index = plan.next_retry_index;
        runtime_proxy_log(
            shared,
            runtime_previous_response_retry_immediate_log_message(
                log_context,
                profile_name,
                delay.as_millis(),
                decision.retry_reason.unwrap_or("-"),
            ),
        );
        if let Some(chain_reason) = decision.chain_retry_reason {
            runtime_proxy_log_chain_retried_owner(
                shared,
                RuntimeProxyChainLog {
                    request_id: log_context.request_id,
                    transport: log_context.transport,
                    route: log_context.route,
                    websocket_session: log_context.websocket_session,
                    profile_name,
                    previous_response_id,
                    reason: chain_reason,
                    via: log_context.via,
                },
                delay.as_millis(),
            );
        }
        return Ok(RuntimePreviousResponseNotFoundAction::RetryOwner);
    }

    if plan.reset_retry_state {
        *previous_response_retry_candidate = None;
        *previous_response_retry_index = plan.next_retry_index;
    }

    if plan.action == RuntimePreviousResponseNotFoundAction::StaleContinuation {
        runtime_proxy_log_previous_response_stale_continuation(shared, log_context, profile_name);
        runtime_proxy_log_chain_dead_upstream_confirmed(
            shared,
            RuntimeProxyChainLog {
                request_id: log_context.request_id,
                transport: log_context.transport,
                route: log_context.route,
                websocket_session: log_context.websocket_session,
                profile_name,
                previous_response_id,
                reason: "previous_response_not_found_locked_affinity",
                via: log_context.via,
            },
            None,
        );
        return Ok(RuntimePreviousResponseNotFoundAction::StaleContinuation);
    }

    if plan.log_fresh_fallback_blocked {
        runtime_proxy_log(
            shared,
            runtime_previous_response_not_found_fresh_fallback_log_message(
                log_context,
                decision,
                fresh_fallback_shape,
                profile_name,
                true,
            ),
        );
    }

    if plan.release_affinity {
        let released_affinity = release_runtime_previous_response_affinity(
            shared,
            profile_name,
            previous_response_id,
            request_turn_state,
            request_session_id,
            route_kind,
        )?;
        if released_affinity {
            runtime_proxy_log(
                shared,
                runtime_previous_response_affinity_released_log_message(log_context, profile_name),
            );
        }
    }
    if plan.clear_response_profile_affinity {
        clear_runtime_response_profile_affinity(RuntimeResponseProfileAffinityClear {
            profile_name,
            bound_profile,
            session_profile,
            candidate_turn_state_retry_profile,
            candidate_turn_state_retry_value,
            pinned_profile,
            previous_response_retry_index,
            reset_previous_response_retry_index: policy
                .reset_previous_response_retry_index_on_rotate,
            turn_state_profile,
            compact_followup_profile,
        });
    }
    if plan.clear_trusted_affinity
        && let Some(trusted_previous_response_affinity) = trusted_previous_response_affinity_mut
    {
        *trusted_previous_response_affinity = false;
    }
    excluded_profiles.insert(profile_name.to_string());

    Ok(RuntimePreviousResponseNotFoundAction::Rotate)
}
