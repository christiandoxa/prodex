//! Responses-route quota-blocked attempt handling.

use super::*;

pub(super) fn runtime_responses_full_context_retry_reply() -> RuntimeResponsesReply {
    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
        400,
        "previous_response_not_found",
        "Previous response was not found. Retrying the full request.",
    ))
}

fn runtime_responses_full_context_fallback_available(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    excluded_profiles: &BTreeSet<String>,
    request_model_name: Option<&str>,
) -> Result<bool> {
    let mut excluded_profiles = excluded_profiles.clone();
    excluded_profiles.insert(profile_name.to_string());
    if runtime_has_route_eligible_quota_fallback_for_model(
        shared,
        profile_name,
        &excluded_profiles,
        RuntimeRouteKind::Responses,
        request_model_name,
    )? {
        return Ok(true);
    }
    Ok(runtime_quota_last_chance_profile_for_route(
        shared,
        &excluded_profiles,
        RuntimeRouteKind::Responses,
        prompt_cache_key,
        request_model_name,
    )?
    .is_some())
}

pub(super) fn runtime_responses_full_context_retry_available(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_model_name: Option<&str>,
    excluded_profiles: &BTreeSet<String>,
) -> Result<bool> {
    let owner_matches = previous_response_id
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Responses)
                .map(|owner| owner.as_deref() == Some(profile_name))
        })
        .transpose()?
        .unwrap_or(false);
    if !runtime_proxy_crate::runtime_full_context_retry_signal_eligible(
        previous_response_id.is_some(),
        request_session_id.is_some(),
        owner_matches,
    ) {
        return Ok(false);
    }
    runtime_responses_full_context_fallback_available(
        shared,
        profile_name,
        prompt_cache_key,
        excluded_profiles,
        request_model_name,
    )
}

pub(super) struct RuntimeResponsesFullContextRetry<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: &'a str,
    pub(super) prompt_cache_key: Option<&'a str>,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) affinity_state: &'a mut RuntimeResponsesAffinityState,
    pub(super) excluded_profiles: &'a BTreeSet<String>,
    pub(super) reason: &'static str,
}

pub(super) fn try_signal_runtime_responses_full_context_retry(
    retry: RuntimeResponsesFullContextRetry<'_>,
) -> Result<Option<RuntimeResponsesReply>> {
    let RuntimeResponsesFullContextRetry {
        request_id,
        shared,
        profile_name,
        prompt_cache_key,
        previous_response_id,
        request_turn_state,
        request_session_id,
        request_model_name,
        affinity_state,
        excluded_profiles,
        reason,
    } = retry;
    if !runtime_responses_full_context_retry_available(
        shared,
        profile_name,
        prompt_cache_key,
        previous_response_id,
        request_session_id,
        request_model_name,
        excluded_profiles,
    )? {
        return Ok(None);
    }

    let released_affinity = release_runtime_quota_blocked_affinity(
        shared,
        profile_name,
        previous_response_id,
        request_turn_state,
        request_session_id,
    )?;
    affinity_state.clear_profile_affinity(profile_name, true);
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http quota_blocked_full_context_retry_signal profile={profile_name} reason={reason} affinity_released={released_affinity}"
        ),
    );
    Ok(Some(runtime_responses_full_context_retry_reply()))
}

pub(super) enum RuntimeResponsesQuotaBlockedAction {
    Continue,
    ReplayWithoutTurnState,
    Return(Box<RuntimeResponsesReply>),
}

pub(super) struct RuntimeResponsesQuotaBlocked<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) response: RuntimeResponsesReply,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) prompt_cache_key: Option<&'a str>,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_requires_previous_response_affinity: bool,
    pub(super) request_reconstructable_full_history: bool,
    pub(super) previous_response_fresh_fallback_shape:
        Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(super) affinity_state: &'a mut RuntimeResponsesAffinityState,
    pub(super) auto_redeemed_profiles: &'a mut BTreeSet<String>,
    pub(super) quota_last_chance_profile: &'a mut Option<String>,
    pub(super) excluded_profiles: &'a mut BTreeSet<String>,
    pub(super) last_failure: &'a mut Option<(RuntimeUpstreamFailureResponse, bool)>,
}

pub(super) fn handle_runtime_responses_quota_blocked(
    quota_blocked: RuntimeResponsesQuotaBlocked<'_>,
) -> Result<RuntimeResponsesQuotaBlockedAction> {
    let RuntimeResponsesQuotaBlocked {
        request_id,
        shared,
        profile_name,
        response,
        request_model_name,
        prompt_cache_key,
        previous_response_id,
        request_turn_state,
        request_session_id,
        request_requires_previous_response_affinity,
        request_reconstructable_full_history,
        previous_response_fresh_fallback_shape,
        affinity_state,
        auto_redeemed_profiles,
        quota_last_chance_profile,
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
            request_model_name,
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
        return Ok(RuntimeResponsesQuotaBlockedAction::Continue);
    }

    let quota_message = extract_runtime_proxy_quota_message_from_response_reply(&response);
    mark_runtime_profile_quota_quarantine(
        shared,
        &profile_name,
        RuntimeRouteKind::Responses,
        quota_message.as_deref(),
    )?;
    if !affinity_state.quota_blocked_affinity_is_releasable(
        &profile_name,
        request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape,
    ) {
        if let Some(retry) =
            try_signal_runtime_responses_full_context_retry(RuntimeResponsesFullContextRetry {
                request_id,
                shared,
                profile_name: &profile_name,
                prompt_cache_key,
                previous_response_id,
                request_turn_state,
                request_session_id,
                request_model_name,
                affinity_state,
                excluded_profiles,
                reason: "upstream_quota",
            })?
        {
            return Ok(RuntimeResponsesQuotaBlockedAction::Return(Box::new(retry)));
        }

        // The owner fact is independent of stricter pinned/session affinity.
        // Using the highest-priority no-rotate classification here would lose
        // valid turn-state ownership when another affinity also exists.
        let turn_state_owner_matches =
            affinity_state.turn_state_profile() == Some(profile_name.as_str());
        let turn_state_full_context_replay =
            prodex_mojo_core::runtime_responses_quota::turn_state_full_context_replay_candidate(
                previous_response_id.is_some(),
                request_turn_state.is_some(),
                turn_state_owner_matches,
                affinity_state.compact_followup_profile_name().is_some(),
                request_reconstructable_full_history,
            )
            .map_err(|error| {
                anyhow::anyhow!("Mojo Responses quota replay planning failed: {error:?}")
            })? && runtime_responses_full_context_fallback_available(
                shared,
                &profile_name,
                prompt_cache_key,
                excluded_profiles,
                request_model_name,
            )?;
        if turn_state_full_context_replay {
            let released_affinity = release_runtime_quota_blocked_affinity(
                shared,
                &profile_name,
                previous_response_id,
                request_turn_state,
                request_session_id,
            )?;
            affinity_state.clear_profile_affinity(&profile_name, true);
            if prepare_runtime_responses_quota_fallback(
                shared,
                request_id,
                &profile_name,
                prompt_cache_key,
                excluded_profiles,
                quota_last_chance_profile,
                request_model_name,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http quota_blocked_turn_state_full_context_replay profile={profile_name} affinity_released={released_affinity}"
                    ),
                );
                *last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
                return Ok(RuntimeResponsesQuotaBlockedAction::ReplayWithoutTurnState);
            }
            return Ok(RuntimeResponsesQuotaBlockedAction::Return(Box::new(
                response,
            )));
        }

        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http upstream_usage_limit_passthrough route=responses profile={profile_name} reason=hard_affinity"
            ),
        );
        return Ok(RuntimeResponsesQuotaBlockedAction::Return(Box::new(
            response,
        )));
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
    if !prepare_runtime_responses_quota_fallback(
        shared,
        request_id,
        &profile_name,
        prompt_cache_key,
        excluded_profiles,
        quota_last_chance_profile,
        request_model_name,
    )? {
        return Ok(RuntimeResponsesQuotaBlockedAction::Return(Box::new(
            response,
        )));
    }

    *last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
    Ok(RuntimeResponsesQuotaBlockedAction::Continue)
}

pub(super) fn prepare_runtime_responses_quota_fallback(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    excluded_profiles: &mut BTreeSet<String>,
    quota_last_chance_profile: &mut Option<String>,
    request_model_name: Option<&str>,
) -> Result<bool> {
    excluded_profiles.insert(profile_name.to_string());
    if runtime_has_route_eligible_quota_fallback_for_model(
        shared,
        profile_name,
        excluded_profiles,
        RuntimeRouteKind::Responses,
        request_model_name,
    )? {
        return Ok(true);
    }
    let Some(fallback_profile) = runtime_quota_last_chance_profile_for_route(
        shared,
        excluded_profiles,
        RuntimeRouteKind::Responses,
        prompt_cache_key,
        request_model_name,
    )?
    else {
        return Ok(false);
    };
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http quota_last_chance profile={fallback_profile} failed_profile={profile_name}"
        ),
    );
    *quota_last_chance_profile = Some(fallback_profile);
    Ok(true)
}

pub(super) fn handle_runtime_responses_quota_attempt(
    context: &mut RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    auto_redeemed_profiles: &mut BTreeSet<String>,
    quota_last_chance_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    response: RuntimeResponsesReply,
) -> Result<Option<RuntimeResponsesReply>> {
    let result = handle_runtime_responses_quota_blocked(RuntimeResponsesQuotaBlocked {
        request_id: context.request_id,
        shared: context.shared,
        profile_name,
        response,
        request_model_name: context.request_model_name.as_deref(),
        prompt_cache_key: context.prompt_cache_key,
        previous_response_id: context.previous_response_id,
        request_turn_state: context.request_turn_state,
        request_session_id: context.request_session_id,
        request_requires_previous_response_affinity: context
            .request_requires_previous_response_affinity,
        request_reconstructable_full_history:
            runtime_proxy_crate::runtime_request_has_reconstructable_full_history(&context.request),
        previous_response_fresh_fallback_shape: context.previous_response_fresh_fallback_shape,
        affinity_state,
        auto_redeemed_profiles,
        quota_last_chance_profile,
        excluded_profiles: &mut loop_state.excluded_profiles,
        last_failure: &mut loop_state.last_failure,
    })?;
    match result {
        RuntimeResponsesQuotaBlockedAction::Continue => Ok(None),
        RuntimeResponsesQuotaBlockedAction::Return(response) => Ok(Some(*response)),
        RuntimeResponsesQuotaBlockedAction::ReplayWithoutTurnState => {
            context.request =
                runtime_proxy_crate::runtime_request_without_turn_state(&context.request);
            context.request_turn_state = None;
            runtime_proxy_log(
                context.shared,
                format!(
                    "request={} transport=http dead_turn_state_replay scrubbed=true",
                    context.request_id
                ),
            );
            Ok(None)
        }
    }
}
