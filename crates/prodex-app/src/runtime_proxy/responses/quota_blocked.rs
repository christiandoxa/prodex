//! Responses-route quota-blocked attempt handling.

use super::*;

pub(super) struct RuntimeResponsesQuotaBlocked<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) response: RuntimeResponsesReply,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_requires_previous_response_affinity: bool,
    pub(super) previous_response_fresh_fallback_shape:
        Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(super) affinity_state: &'a mut RuntimeResponsesAffinityState,
    pub(super) auto_redeemed_profiles: &'a mut BTreeSet<String>,
    pub(super) excluded_profiles: &'a mut BTreeSet<String>,
    pub(super) last_failure: &'a mut Option<(RuntimeUpstreamFailureResponse, bool)>,
}

pub(super) fn handle_runtime_responses_quota_blocked(
    quota_blocked: RuntimeResponsesQuotaBlocked<'_>,
) -> Result<Option<RuntimeResponsesReply>> {
    let RuntimeResponsesQuotaBlocked {
        request_id,
        shared,
        profile_name,
        response,
        request_model_name,
        previous_response_id,
        request_turn_state,
        request_session_id,
        request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape,
        affinity_state,
        auto_redeemed_profiles,
        excluded_profiles,
        last_failure,
    } = quota_blocked;

    runtime_proxy_log(
        shared,
        format!("request={request_id} transport=http quota_blocked profile={profile_name}"),
    );
    if !auto_redeemed_profiles.contains(&profile_name)
        && runtime_auto_redeem_usage_limit_reset_credit(
            shared,
            &profile_name,
            RuntimeRouteKind::Responses,
            "responses_quota_blocked",
            false,
        )? == RuntimeAutoRedeemResetCreditOutcome::Redeemed
    {
        auto_redeemed_profiles.insert(profile_name);
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http quota_blocked_auto_redeemed_retry route=responses"
            ),
        );
        return Ok(None);
    }

    let quota_message = extract_runtime_proxy_quota_message_from_response_reply(&response);
    mark_runtime_profile_quota_quarantine_for_request_model(
        shared,
        &profile_name,
        RuntimeRouteKind::Responses,
        quota_message.as_deref(),
        request_model_name,
    )?;
    if !affinity_state.quota_blocked_affinity_is_releasable(
        &profile_name,
        request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http upstream_usage_limit_passthrough route=responses profile={profile_name} reason=hard_affinity"
            ),
        );
        return Ok(Some(response));
    }

    let released_affinity = release_runtime_quota_blocked_affinity(
        shared,
        &profile_name,
        previous_response_id,
        request_turn_state,
        request_session_id,
    )?;
    affinity_state.clear_profile_affinity(&profile_name, true);
    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name}"
            ),
        );
    }
    if !runtime_has_route_eligible_quota_fallback(
        shared,
        &profile_name,
        &BTreeSet::new(),
        RuntimeRouteKind::Responses,
    )? {
        return Ok(Some(response));
    }

    excluded_profiles.insert(profile_name);
    *last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
    Ok(None)
}
