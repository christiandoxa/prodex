use super::*;

mod affinity_state;
mod attempt;
mod fallback;
mod local_selection;
mod previous_response;

use self::affinity_state::RuntimeResponsesAffinityState;
pub(crate) use self::attempt::attempt_runtime_responses_request;
use self::fallback::{
    RuntimeResponsesDirectCurrentFallback, RuntimeResponsesDirectCurrentFallbackAction,
    RuntimeResponsesDirectCurrentFallbackReason,
    try_runtime_responses_direct_current_profile_fallback,
};
use self::local_selection::{
    RuntimeResponsesLocalSelectionAction, runtime_responses_local_selection_action,
    runtime_responses_local_selection_failure_reply,
};
use self::previous_response::runtime_responses_previous_response_not_found_context;

pub(crate) fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let request = request.clone();
    let request_requires_previous_response_affinity =
        runtime_request_requires_previous_response_affinity(&request);
    let previous_response_fresh_fallback_shape =
        runtime_request_previous_response_fresh_fallback_shape(&request);
    let previous_response_id = runtime_request_previous_response_id(&request);
    let mut request_turn_state = runtime_request_turn_state(&request);
    let explicit_request_session_id = runtime_request_explicit_session_id(&request);
    let request_session_id = runtime_request_session_id(&request);
    let prompt_cache_key = runtime_smart_context_effective_prompt_cache_key(
        &request,
        shared,
        previous_response_id.is_none()
            && request_turn_state.is_none()
            && request_session_id.is_none(),
    );
    let bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Responses)
        })
        .transpose()?
        .flatten();
    let trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
        shared,
        previous_response_id.as_deref(),
        bound_profile.as_deref(),
    )?;
    if request_turn_state.is_none()
        && let Some(turn_state) = runtime_previous_response_turn_state(
            shared,
            previous_response_id.as_deref(),
            bound_profile.as_deref(),
        )?
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http route=responses previous_response_turn_state_rehydrated response_id={} profile={} turn_state={turn_state}",
                previous_response_id.as_deref().unwrap_or("-"),
                bound_profile.as_deref().unwrap_or("-"),
            ),
        );
        request_turn_state = Some(turn_state);
    }
    let turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut affinity_state = RuntimeResponsesAffinityState::new(
        bound_profile,
        trusted_previous_response_affinity,
        turn_state_profile,
    );
    affinity_state.refresh_route_affinity(
        shared,
        request_id,
        "initial",
        previous_response_id.as_deref(),
        request_turn_state.as_deref(),
        request_session_id.as_deref(),
        explicit_request_session_id.as_ref(),
    )?;
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure: Option<(RuntimeUpstreamFailureResponse, bool)> = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Responses);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            affinity_state.has_continuation_priority(
                previous_response_id.as_deref(),
                request_turn_state.as_deref(),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if let Some((profile_name, source)) = affinity_state.compact_followup_profile() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                return Ok(runtime_proxy_final_responses_failure_reply(
                    last_failure,
                    saw_inflight_saturation,
                ));
            }
            if let Some(action) = try_runtime_responses_direct_current_profile_fallback(
                RuntimeResponsesDirectCurrentFallback {
                    request_id,
                    request: &request,
                    shared,
                    reason: RuntimeResponsesDirectCurrentFallbackReason::PrecommitBudgetExhausted,
                    previous_response_id: previous_response_id.as_deref(),
                    prompt_cache_key: prompt_cache_key.as_deref(),
                    request_turn_state: request_turn_state.as_deref(),
                    request_session_id: request_session_id.as_deref(),
                    request_requires_previous_response_affinity,
                    previous_response_fresh_fallback_shape,
                    saw_inflight_saturation,
                },
                &mut affinity_state,
                &mut excluded_profiles,
                &mut last_failure,
            )? {
                match action {
                    RuntimeResponsesDirectCurrentFallbackAction::Continue => continue,
                    RuntimeResponsesDirectCurrentFallbackAction::Return(response) => {
                        return Ok(*response);
                    }
                }
            }
            return Ok(runtime_proxy_final_responses_failure_reply(
                last_failure,
                saw_inflight_saturation,
            ));
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route_with_selection(
            shared,
            affinity_state.candidate_selection(
                &excluded_profiles,
                previous_response_id.as_deref(),
                prompt_cache_key.as_deref(),
            ),
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
                        Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
                        None => "none",
                    }
                ),
            );
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait::new(
                    request_id,
                    &request,
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Responses,
                    selection_started_at,
                    affinity_state.has_continuation_priority(
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    ),
                    affinity_state.wait_affinity_owner(),
                ),
            )? {
                continue;
            }
            if let Some((profile_name, source)) = affinity_state.compact_followup_profile() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                return Ok(runtime_proxy_final_responses_failure_reply(
                    last_failure,
                    saw_inflight_saturation,
                ));
            }
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Responses,
                )?;
            if remaining_cold_start_profiles > 0 {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http candidate_exhausted_continue route=responses remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Responses);
                continue;
            }
            if let Some(action) = try_runtime_responses_direct_current_profile_fallback(
                RuntimeResponsesDirectCurrentFallback {
                    request_id,
                    request: &request,
                    shared,
                    reason: RuntimeResponsesDirectCurrentFallbackReason::CandidateExhausted,
                    previous_response_id: previous_response_id.as_deref(),
                    prompt_cache_key: prompt_cache_key.as_deref(),
                    request_turn_state: request_turn_state.as_deref(),
                    request_session_id: request_session_id.as_deref(),
                    request_requires_previous_response_affinity,
                    previous_response_fresh_fallback_shape,
                    saw_inflight_saturation,
                },
                &mut affinity_state,
                &mut excluded_profiles,
                &mut last_failure,
            )? {
                match action {
                    RuntimeResponsesDirectCurrentFallbackAction::Continue => continue,
                    RuntimeResponsesDirectCurrentFallbackAction::Return(response) => {
                        return Ok(*response);
                    }
                }
            }
            return Ok(runtime_proxy_final_responses_failure_reply(
                last_failure,
                saw_inflight_saturation,
            ));
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            affinity_state.turn_state_override_for(&candidate_name, request_turn_state.as_deref());
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                affinity_state.pinned_profile(),
                affinity_state.turn_state_profile(),
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        if previous_response_id.is_none()
            && affinity_state.pinned_profile().is_none()
            && affinity_state.turn_state_profile().is_none()
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "responses_http",
            )?
        {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "profile_inflight_saturated",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", candidate_name.as_str()),
                        runtime_proxy_log_field(
                            "hard_limit",
                            runtime_proxy_profile_inflight_hard_limit().to_string(),
                        ),
                    ],
                ),
            );
            saw_inflight_saturation = true;
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait::new(
                    request_id,
                    &request,
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Responses,
                    selection_started_at,
                    affinity_state.has_continuation_priority(
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    ),
                    affinity_state.wait_affinity_owner(),
                ),
            )? {
                continue;
            }
            excluded_profiles.insert(candidate_name);
            continue;
        }

        match attempt_runtime_responses_request(
            request_id,
            &request,
            shared,
            &candidate_name,
            turn_state_override,
            prompt_cache_key.as_deref(),
        )? {
            RuntimeResponsesAttempt::Success {
                profile_name,
                response,
            } => {
                affinity_state.remember_successful_previous_response_owner(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                )?;
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                )?;
                runtime_proxy_log(
                    shared,
                    format!("request={request_id} transport=http committed profile={profile_name}"),
                );
                return Ok(response);
            }
            RuntimeResponsesAttempt::QuotaBlocked {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http quota_blocked profile={profile_name}"
                    ),
                );
                let quota_message =
                    extract_runtime_proxy_quota_message_from_response_reply(&response);
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
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http upstream_usage_limit_passthrough route=responses profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    return Ok(response);
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
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
                    return Ok(response);
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
            }
            RuntimeResponsesAttempt::AuthFailed {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http auth_failed profile={profile_name}"
                    ),
                );
                if !affinity_state.quota_blocked_affinity_is_releasable(
                    &profile_name,
                    request_requires_previous_response_affinity,
                    previous_response_fresh_fallback_shape,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http upstream_auth_failure_passthrough route=responses profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    return Ok(response);
                }
                let released_affinity = release_runtime_auth_failed_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                affinity_state.clear_profile_affinity(&profile_name, true);
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http auth_failed_affinity_released profile={profile_name}"
                        ),
                    );
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
            }
            RuntimeResponsesAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=responses reason={reason}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                match runtime_responses_local_selection_action(
                    affinity_state.quota_blocked_affinity_is_releasable(
                        &profile_name,
                        request_requires_previous_response_affinity,
                        previous_response_fresh_fallback_shape,
                    ),
                ) {
                    RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable => {
                        return Ok(runtime_responses_local_selection_failure_reply());
                    }
                    RuntimeResponsesLocalSelectionAction::Rotate => {}
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                affinity_state.clear_profile_affinity(&profile_name, true);
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason}"
                        ),
                    );
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name,
                response,
                turn_state,
            } => {
                match handle_runtime_previous_response_not_found(
                    runtime_responses_previous_response_not_found_context(
                        shared,
                        request_id,
                        &profile_name,
                        turn_state,
                        None,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                        request_session_id.as_deref(),
                        request_requires_previous_response_affinity,
                        affinity_state.trusted_previous_response_affinity(),
                        previous_response_fresh_fallback_shape,
                        RuntimePreviousResponseNotFoundPolicy::responses(true),
                    ),
                    affinity_state.previous_response_not_found_state(&mut excluded_profiles, true),
                )? {
                    RuntimePreviousResponseNotFoundAction::RetryOwner
                    | RuntimePreviousResponseNotFoundAction::Rotate => {
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Http(response), false));
                        continue;
                    }
                    RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                        unreachable!("responses previous_response policy cannot return this action")
                    }
                }
            }
        }
    }
}
