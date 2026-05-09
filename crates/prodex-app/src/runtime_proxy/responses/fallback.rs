use super::*;

#[derive(Clone, Copy)]
pub(super) enum RuntimeResponsesDirectCurrentFallbackReason {
    PrecommitBudgetExhausted,
    CandidateExhausted,
}

impl RuntimeResponsesDirectCurrentFallbackReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::PrecommitBudgetExhausted => "precommit_budget_exhausted",
            Self::CandidateExhausted => "candidate_exhausted",
        }
    }
}

pub(super) struct RuntimeResponsesDirectCurrentFallback<'a> {
    pub(super) request_id: u64,
    pub(super) request: &'a RuntimeProxyRequest,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) reason: RuntimeResponsesDirectCurrentFallbackReason,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) prompt_cache_key: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_requires_previous_response_affinity: bool,
    pub(super) previous_response_fresh_fallback_shape:
        Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(super) saw_inflight_saturation: bool,
}

pub(super) enum RuntimeResponsesDirectCurrentFallbackAction {
    Continue,
    Return(Box<RuntimeResponsesReply>),
}

pub(super) fn try_runtime_responses_direct_current_profile_fallback(
    fallback: RuntimeResponsesDirectCurrentFallback<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    excluded_profiles: &mut BTreeSet<String>,
    last_failure: &mut Option<(RuntimeUpstreamFailureResponse, bool)>,
) -> Result<Option<RuntimeResponsesDirectCurrentFallbackAction>> {
    if !affinity_state.allows_direct_current_profile_fallback(
        fallback.previous_response_id,
        fallback.request_turn_state,
        fallback.saw_inflight_saturation,
        last_failure.is_some(),
    ) {
        return Ok(None);
    }
    let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
        fallback.shared,
        excluded_profiles,
        RuntimeRouteKind::Responses,
    )?
    else {
        return Ok(None);
    };
    runtime_proxy_log(
        fallback.shared,
        format!(
            "request={} transport=http direct_current_profile_fallback profile={} reason={}",
            fallback.request_id,
            current_profile,
            fallback.reason.as_str(),
        ),
    );
    match attempt_runtime_responses_request(
        fallback.request_id,
        fallback.request,
        fallback.shared,
        &current_profile,
        fallback.request_turn_state,
        fallback.prompt_cache_key,
    )? {
        RuntimeResponsesAttempt::Success {
            profile_name,
            response,
        } => {
            affinity_state.remember_successful_previous_response_owner(
                fallback.shared,
                &profile_name,
                fallback.previous_response_id,
            )?;
            commit_runtime_proxy_profile_selection_with_notice(
                fallback.shared,
                &profile_name,
                RuntimeRouteKind::Responses,
            )?;
            runtime_proxy_log(
                fallback.shared,
                format!(
                    "request={} transport=http committed profile={} via=direct_current_profile_fallback",
                    fallback.request_id, profile_name
                ),
            );
            Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Return(
                Box::new(response),
            )))
        }
        RuntimeResponsesAttempt::QuotaBlocked {
            profile_name,
            response,
        } => {
            mark_runtime_profile_retry_backoff(fallback.shared, &profile_name)?;
            if !affinity_state.quota_blocked_affinity_is_releasable(
                &profile_name,
                fallback.request_requires_previous_response_affinity,
                fallback.previous_response_fresh_fallback_shape,
            ) {
                return Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Return(
                    Box::new(response),
                )));
            }
            let released_affinity = release_runtime_quota_blocked_affinity(
                fallback.shared,
                &profile_name,
                fallback.previous_response_id,
                fallback.request_turn_state,
                fallback.request_session_id,
            )?;
            affinity_state.clear_profile_affinity(&profile_name, true);
            if released_affinity {
                runtime_proxy_log(
                    fallback.shared,
                    format!(
                        "request={} transport=http quota_blocked_affinity_released profile={} via=direct_current_profile_fallback",
                        fallback.request_id, profile_name
                    ),
                );
            }
            if !runtime_has_route_eligible_quota_fallback(
                fallback.shared,
                &profile_name,
                &BTreeSet::new(),
                RuntimeRouteKind::Responses,
            )? {
                return Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Return(
                    Box::new(response),
                )));
            }
            excluded_profiles.insert(profile_name);
            *last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
            Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Continue))
        }
        RuntimeResponsesAttempt::AuthFailed {
            profile_name,
            response,
        } => {
            runtime_proxy_log(
                fallback.shared,
                format!(
                    "request={} transport=http auth_failed profile={} via=direct_current_profile_fallback",
                    fallback.request_id, profile_name
                ),
            );
            if !affinity_state.quota_blocked_affinity_is_releasable(
                &profile_name,
                fallback.request_requires_previous_response_affinity,
                fallback.previous_response_fresh_fallback_shape,
            ) {
                return Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Return(
                    Box::new(response),
                )));
            }
            let released_affinity = release_runtime_auth_failed_affinity(
                fallback.shared,
                &profile_name,
                fallback.previous_response_id,
                fallback.request_turn_state,
                fallback.request_session_id,
            )?;
            affinity_state.clear_profile_affinity(&profile_name, true);
            if released_affinity {
                runtime_proxy_log(
                    fallback.shared,
                    format!(
                        "request={} transport=http auth_failed_affinity_released profile={} via=direct_current_profile_fallback",
                        fallback.request_id, profile_name
                    ),
                );
            }
            excluded_profiles.insert(profile_name);
            *last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
            Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Continue))
        }
        RuntimeResponsesAttempt::PreviousResponseNotFound {
            profile_name,
            response,
            turn_state,
        } => {
            match handle_runtime_previous_response_not_found(
                runtime_responses_previous_response_not_found_context(
                    RuntimeResponsesPreviousResponseNotFoundContextInput {
                        shared: fallback.shared,
                        request_id: fallback.request_id,
                        profile_name: &profile_name,
                        turn_state,
                        via: Some("direct_current_profile_fallback"),
                        previous_response_id: fallback.previous_response_id,
                        request_turn_state: fallback.request_turn_state,
                        request_session_id: fallback.request_session_id,
                        request_requires_previous_response_affinity: fallback
                            .request_requires_previous_response_affinity,
                        trusted_previous_response_affinity: affinity_state
                            .trusted_previous_response_affinity(),
                        fresh_fallback_shape: fallback.previous_response_fresh_fallback_shape,
                        policy: RuntimePreviousResponseNotFoundPolicy::responses(false),
                    },
                ),
                affinity_state.previous_response_not_found_state(excluded_profiles, false),
            )? {
                RuntimePreviousResponseNotFoundAction::RetryOwner
                | RuntimePreviousResponseNotFoundAction::Rotate => {
                    *last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
                    Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Continue))
                }
                RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                    unreachable!("responses previous_response policy cannot return this action")
                }
            }
        }
        RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name,
            reason,
        } => {
            mark_runtime_profile_retry_backoff(fallback.shared, &profile_name)?;
            match runtime_responses_local_selection_action(
                affinity_state.quota_blocked_affinity_is_releasable(
                    &profile_name,
                    fallback.request_requires_previous_response_affinity,
                    fallback.previous_response_fresh_fallback_shape,
                ),
            ) {
                RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable => {
                    Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Return(
                        Box::new(runtime_responses_local_selection_failure_reply()),
                    )))
                }
                RuntimeResponsesLocalSelectionAction::Rotate => {
                    let released_affinity = release_runtime_quota_blocked_affinity(
                        fallback.shared,
                        &profile_name,
                        fallback.previous_response_id,
                        fallback.request_turn_state,
                        fallback.request_session_id,
                    )?;
                    affinity_state.clear_profile_affinity(&profile_name, true);
                    if released_affinity {
                        runtime_proxy_log(
                            fallback.shared,
                            format!(
                                "request={} transport=http quota_blocked_affinity_released profile={} reason={} via=direct_current_profile_fallback",
                                fallback.request_id, profile_name, reason
                            ),
                        );
                    }
                    excluded_profiles.insert(profile_name);
                    Ok(Some(RuntimeResponsesDirectCurrentFallbackAction::Continue))
                }
            }
        }
    }
}
