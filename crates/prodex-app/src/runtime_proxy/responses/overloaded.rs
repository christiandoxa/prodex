//! Responses-route overload handling.

use super::{
    RUNTIME_PROFILE_BAD_PAIRING_PENALTY, RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
    RuntimeResponsesAffinityState, RuntimeResponsesReply, RuntimeRotationProxyShared,
    RuntimeRouteKind, RuntimeUpstreamFailureResponse, bump_runtime_profile_bad_pairing_score,
    bump_runtime_profile_health_score, mark_runtime_profile_retry_backoff,
    release_runtime_retryable_failure_affinity, runtime_proxy_log,
    runtime_responses_full_context_retry_available, runtime_responses_full_context_retry_reply,
};
use anyhow::Result;
use std::collections::BTreeSet;

pub(super) struct RuntimeResponsesOverloaded<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) response: RuntimeResponsesReply,
    pub(super) prompt_cache_key: Option<&'a str>,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) affinity_state: &'a mut RuntimeResponsesAffinityState,
    pub(super) excluded_profiles: &'a mut BTreeSet<String>,
    pub(super) last_failure: &'a mut Option<(RuntimeUpstreamFailureResponse, bool)>,
}

pub(super) fn handle_runtime_responses_overloaded(
    overloaded: RuntimeResponsesOverloaded<'_>,
) -> Result<Option<RuntimeResponsesReply>> {
    let RuntimeResponsesOverloaded {
        request_id,
        shared,
        profile_name,
        response,
        prompt_cache_key,
        previous_response_id,
        request_turn_state,
        request_session_id,
        request_model_name,
        affinity_state,
        excluded_profiles,
        last_failure,
    } = overloaded;

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_overloaded route=responses profile={profile_name}"
        ),
    );
    mark_runtime_profile_retry_backoff(shared, &profile_name)?;
    let _ = bump_runtime_profile_health_score(
        shared,
        &profile_name,
        RuntimeRouteKind::Responses,
        RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
        "responses_overload",
    );
    let _ = bump_runtime_profile_bad_pairing_score(
        shared,
        &profile_name,
        RuntimeRouteKind::Responses,
        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
        "responses_overload",
    );

    if affinity_state.candidate_has_hard_affinity(&profile_name) {
        if runtime_responses_full_context_retry_available(
            shared,
            &profile_name,
            prompt_cache_key,
            previous_response_id,
            request_session_id,
            request_model_name,
            excluded_profiles,
        )? {
            let released_affinity = release_runtime_retryable_failure_affinity(
                shared,
                &profile_name,
                previous_response_id,
                request_turn_state,
                request_session_id,
                "overload_full_context_retry",
            )?;
            affinity_state.clear_profile_affinity(&profile_name, true);
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http upstream_overload_full_context_retry_signal profile={profile_name} affinity_released={released_affinity}"
                ),
            );
            return Ok(Some(runtime_responses_full_context_retry_reply()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http upstream_overload_passthrough route=responses profile={profile_name} reason=hard_affinity"
            ),
        );
        return Ok(Some(response));
    }

    excluded_profiles.insert(profile_name);
    *last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
    Ok(None)
}
