//! Responses-route overload handling.

use super::{
    RUNTIME_PROFILE_BAD_PAIRING_PENALTY, RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
    RuntimeResponsesAffinityState, RuntimeResponsesReply, RuntimeRotationProxyShared,
    RuntimeRouteKind, RuntimeUpstreamFailureResponse, bump_runtime_profile_bad_pairing_score,
    bump_runtime_profile_health_score, mark_runtime_profile_retry_backoff,
    runtime_candidate_has_hard_affinity, runtime_proxy_log,
};
use anyhow::Result;
use std::collections::BTreeSet;

pub(super) struct RuntimeResponsesOverloaded<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) response: RuntimeResponsesReply,
    pub(super) affinity_state: &'a RuntimeResponsesAffinityState,
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

    if runtime_candidate_has_hard_affinity(affinity_state.candidate_affinity(&profile_name)) {
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
