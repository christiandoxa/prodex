use super::*;

mod affinity_state;
mod attempt;
mod fallback;
mod local_selection;
mod overloaded;
mod previous_response;
mod quota_blocked;

use self::affinity_state::{
    RuntimeResponsesAffinityState, RuntimeResponsesRefreshRouteAffinityInput,
};
pub(crate) use self::attempt::{
    RuntimeResponsesAttemptOptions, RuntimeResponsesContinuationTrace,
    attempt_runtime_responses_request, log_runtime_responses_continuation_trace,
    runtime_response_trace_provider_labels,
};
use self::fallback::{
    RuntimeResponsesDirectCurrentFallback, RuntimeResponsesDirectCurrentFallbackAction,
    RuntimeResponsesDirectCurrentFallbackReason,
    try_runtime_responses_direct_current_profile_fallback,
};
use self::local_selection::{
    RuntimeResponsesLocalSelectionBlocked, handle_runtime_responses_local_selection_blocked,
    runtime_responses_local_selection_failure_reply,
};
use self::overloaded::{RuntimeResponsesOverloaded, handle_runtime_responses_overloaded};
use self::previous_response::{
    RuntimeResponsesPreviousResponseNotFoundContextInput,
    handle_runtime_responses_previous_response_attempt,
    runtime_responses_previous_response_not_found_context,
};
use self::quota_blocked::{
    RuntimeResponsesQuotaBlocked, handle_runtime_responses_quota_blocked,
    prepare_runtime_responses_quota_fallback,
};

fn runtime_responses_stale_continuation_reply() -> RuntimeResponsesReply {
    RuntimeResponsesReply::Buffered(RuntimeHeapTrimmedBufferedResponseParts::from_crate_parts(
        runtime_proxy_crate::runtime_proxy_stale_continuation_http_parts(),
    ))
}

struct RuntimeResponsesRequestContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    request_requires_previous_response_affinity: bool,
    previous_response_fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    previous_response_id: Option<&'a str>,
    prompt_cache_key: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_model_name: Option<&'a str>,
}

enum RuntimeResponsesLoopControl {
    Continue,
    Return(Box<RuntimeResponsesReply>),
}

pub(crate) fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let request_requires_previous_response_affinity =
        runtime_request_requires_previous_response_affinity(request);
    let previous_response_fresh_fallback_shape =
        runtime_request_previous_response_fresh_fallback_shape(request);
    let previous_response_id = runtime_request_previous_response_id(request);
    let mut request_turn_state = runtime_request_turn_state(request);
    let explicit_request_session_id = runtime_request_explicit_session_id(request);
    let request_session_id = runtime_request_session_id(request);
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
    let prompt_cache_key = runtime_smart_context_effective_prompt_cache_key(
        request,
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
    let turn_state_profile = runtime_turn_state_affinity_profile(
        shared,
        request_turn_state.as_deref(),
        bound_profile.as_deref(),
    )?;
    let mut affinity_state = RuntimeResponsesAffinityState::new(
        bound_profile,
        trusted_previous_response_affinity,
        turn_state_profile,
    );
    affinity_state.refresh_route_affinity(RuntimeResponsesRefreshRouteAffinityInput {
        shared,
        request_id,
        reason: "initial",
        previous_response_id: previous_response_id.as_deref(),
        request_turn_state: request_turn_state.as_deref(),
        request_session_id: request_session_id.as_deref(),
        explicit_request_session_id: explicit_request_session_id.as_ref(),
    })?;
    let mut auto_redeemed_profiles = BTreeSet::new();
    let mut quota_last_chance_profile = None;
    let mut loop_state = RuntimePrecommitLoopState::<RuntimeUpstreamFailureResponse>::new();
    let context = RuntimeResponsesRequestContext {
        request_id,
        request,
        shared,
        request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape,
        previous_response_id: previous_response_id.as_deref(),
        prompt_cache_key: prompt_cache_key.as_deref(),
        request_turn_state: request_turn_state.as_deref(),
        request_session_id: request_session_id.as_deref(),
        request_model_name: request_model_name.as_deref(),
    };

    run_runtime_responses_loop(
        &context,
        &mut affinity_state,
        &mut auto_redeemed_profiles,
        &mut quota_last_chance_profile,
        &mut loop_state,
    )
}

fn run_runtime_responses_loop(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    auto_redeemed_profiles: &mut BTreeSet<String>,
    quota_last_chance_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
) -> Result<RuntimeResponsesReply> {
    loop {
        if loop_state.local_capacity_wait_timed_out {
            return Ok(RuntimeResponsesReply::Buffered(
                build_runtime_proxy_json_error_parts(
                    503,
                    "local_capacity_timeout",
                    runtime_proxy_local_capacity_timeout_message(),
                ),
            ));
        }
        if let Some(control) = handle_runtime_responses_budget_exhausted(
            context,
            affinity_state,
            loop_state,
            quota_last_chance_profile,
        )? {
            match control {
                RuntimeResponsesLoopControl::Continue => continue,
                RuntimeResponsesLoopControl::Return(response) => return Ok(*response),
            }
        }

        let Some(candidate_name) = runtime_responses_next_candidate(
            context,
            affinity_state,
            loop_state,
            quota_last_chance_profile,
        )?
        else {
            match handle_runtime_responses_candidate_exhausted(
                context,
                affinity_state,
                loop_state,
                quota_last_chance_profile,
            )? {
                RuntimeResponsesLoopControl::Continue => continue,
                RuntimeResponsesLoopControl::Return(response) => return Ok(*response),
            }
        };
        if runtime_responses_candidate_saturated(
            context,
            affinity_state,
            loop_state,
            &candidate_name,
        )? {
            continue;
        }
        let turn_state_override = affinity_state
            .turn_state_override_for(&candidate_name, context.request_turn_state)
            .map(str::to_owned);
        runtime_proxy_log(
            context.shared,
            format!(
                "request={} transport=http candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                context.request_id,
                candidate_name,
                affinity_state.pinned_profile(),
                affinity_state.turn_state_profile(),
                turn_state_override,
                loop_state.excluded_profiles.len()
            ),
        );
        if let Some(response) = handle_runtime_responses_attempt(
            context,
            &candidate_name,
            turn_state_override.as_deref(),
            affinity_state,
            auto_redeemed_profiles,
            quota_last_chance_profile,
            loop_state,
        )? {
            return Ok(response);
        }
    }
}

fn runtime_responses_next_candidate(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &RuntimeResponsesAffinityState,
    loop_state: &RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    quota_last_chance_profile: &mut Option<String>,
) -> Result<Option<String>> {
    let session_profile = affinity_state.session_profile().map(str::to_owned);
    let selected_profile = if let Some(profile_name) = quota_last_chance_profile.take() {
        Some(profile_name)
    } else {
        select_runtime_response_candidate_for_route_with_request(
            context.shared,
            affinity_state.candidate_selection(
                &loop_state.excluded_profiles,
                context.previous_response_id,
                context.prompt_cache_key,
            ),
            Some(context.request_id),
            context.request_model_name,
        )?
    };
    let _ = release_runtime_rotated_session_affinity(
        context.shared,
        session_profile.as_deref(),
        selected_profile.as_deref(),
        context.request_session_id,
    )?;
    Ok(selected_profile)
}

fn runtime_responses_candidate_saturated(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    candidate_name: &str,
) -> Result<bool> {
    if affinity_state.candidate_has_hard_affinity(candidate_name)
        || !runtime_profile_inflight_hard_limited_for_context(
            context.shared,
            candidate_name,
            "responses_http",
        )?
    {
        return Ok(false);
    }
    runtime_proxy_log(
        context.shared,
        runtime_proxy_structured_log_message(
            "profile_inflight_saturated",
            [
                runtime_proxy_log_field("request", context.request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("profile", candidate_name),
                runtime_proxy_log_field(
                    "hard_limit",
                    context
                        .shared
                        .runtime_config
                        .tuning
                        .profile_inflight_hard_limit
                        .to_string(),
                ),
            ],
        ),
    );
    loop_state.record_inflight_saturation();
    match runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
        request_id: context.request_id,
        request: context.request,
        shared: context.shared,
        excluded_profiles: &loop_state.excluded_profiles,
        route_kind: RuntimeRouteKind::Responses,
        selection_started_at: loop_state.selection_started_at,
        continuation: affinity_state
            .has_continuation_priority(context.previous_response_id, context.request_turn_state),
        wait_affinity_owner: affinity_state.wait_affinity_owner(),
        selected_profile: None,
    })? {
        RuntimeInflightReliefWaitResult::Relieved
        | RuntimeInflightReliefWaitResult::NotWaitable => Ok(true),
        RuntimeInflightReliefWaitResult::DeadlineExpired => {
            loop_state.record_local_capacity_wait_timeout();
            Ok(true)
        }
    }
}

fn handle_runtime_responses_budget_exhausted(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    quota_last_chance_profile: &mut Option<String>,
) -> Result<Option<RuntimeResponsesLoopControl>> {
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_route(context.shared, RuntimeRouteKind::Responses);
    if !loop_state.budget_exhausted(
        context.shared,
        affinity_state
            .has_continuation_priority(context.previous_response_id, context.request_turn_state),
        pressure_mode,
    )? {
        return Ok(None);
    }
    runtime_proxy_log(
        context.shared,
        format!(
            "request={} transport=http precommit_budget_exhausted attempts={} elapsed_ms={} pressure_mode={pressure_mode}",
            context.request_id,
            loop_state.selection_attempts,
            loop_state.selection_started_at.elapsed().as_millis()
        ),
    );
    if let Some((profile_name, source)) = affinity_state.compact_followup_profile() {
        runtime_proxy_log(
            context.shared,
            format!(
                "request={} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted",
                context.request_id
            ),
        );
        return Ok(Some(RuntimeResponsesLoopControl::Return(Box::new(
            runtime_proxy_final_responses_failure_reply(
                loop_state.last_failure.take(),
                loop_state.saw_inflight_saturation,
            ),
        ))));
    }
    if affinity_state.wait_affinity_owner().is_none()
        && loop_state.maybe_wait_for_transient_recovery(
            context.request_id,
            context.shared,
            RuntimeRouteKind::Responses,
        )?
    {
        return Ok(Some(RuntimeResponsesLoopControl::Continue));
    }
    if let Some(action) = try_runtime_responses_direct_current_profile_fallback(
        RuntimeResponsesDirectCurrentFallback {
            request_id: context.request_id,
            request: context.request,
            shared: context.shared,
            reason: RuntimeResponsesDirectCurrentFallbackReason::PrecommitBudgetExhausted,
            previous_response_id: context.previous_response_id,
            prompt_cache_key: context.prompt_cache_key,
            request_turn_state: context.request_turn_state,
            request_session_id: context.request_session_id,
            request_requires_previous_response_affinity: context
                .request_requires_previous_response_affinity,
            previous_response_fresh_fallback_shape: context.previous_response_fresh_fallback_shape,
            saw_inflight_saturation: loop_state.saw_inflight_saturation,
        },
        &mut *affinity_state,
        &mut loop_state.excluded_profiles,
        &mut loop_state.last_failure,
        quota_last_chance_profile,
    )? {
        return Ok(Some(match action {
            RuntimeResponsesDirectCurrentFallbackAction::Continue => {
                RuntimeResponsesLoopControl::Continue
            }
            RuntimeResponsesDirectCurrentFallbackAction::Return(response) => {
                RuntimeResponsesLoopControl::Return(response)
            }
        }));
    }
    Ok(Some(RuntimeResponsesLoopControl::Return(Box::new(
        runtime_proxy_final_responses_failure_reply(
            loop_state.last_failure.take(),
            loop_state.saw_inflight_saturation,
        ),
    ))))
}

fn handle_runtime_responses_candidate_exhausted(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    quota_last_chance_profile: &mut Option<String>,
) -> Result<RuntimeResponsesLoopControl> {
    runtime_proxy_log(
        context.shared,
        format!(
            "request={} transport=http candidate_exhausted last_failure={}",
            context.request_id,
            match &loop_state.last_failure {
                Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
                Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
                None => "none",
            }
        ),
    );
    if affinity_state.wait_affinity_owner().is_none()
        && loop_state.maybe_wait_for_transient_recovery(
            context.request_id,
            context.shared,
            RuntimeRouteKind::Responses,
        )?
    {
        return Ok(RuntimeResponsesLoopControl::Continue);
    }
    match runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
        request_id: context.request_id,
        request: context.request,
        shared: context.shared,
        excluded_profiles: &loop_state.excluded_profiles,
        route_kind: RuntimeRouteKind::Responses,
        selection_started_at: loop_state.selection_started_at,
        continuation: affinity_state
            .has_continuation_priority(context.previous_response_id, context.request_turn_state),
        wait_affinity_owner: affinity_state.wait_affinity_owner(),
        selected_profile: None,
    })? {
        RuntimeInflightReliefWaitResult::Relieved => {
            return Ok(RuntimeResponsesLoopControl::Continue);
        }
        RuntimeInflightReliefWaitResult::DeadlineExpired => {
            return Ok(RuntimeResponsesLoopControl::Return(Box::new(
                RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                    503,
                    "local_capacity_timeout",
                    runtime_proxy_local_capacity_timeout_message(),
                )),
            )));
        }
        RuntimeInflightReliefWaitResult::NotWaitable => {}
    }
    if let Some((profile_name, source)) = affinity_state.compact_followup_profile() {
        runtime_proxy_log(
            context.shared,
            format!(
                "request={} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted",
                context.request_id
            ),
        );
        return Ok(RuntimeResponsesLoopControl::Return(Box::new(
            runtime_proxy_final_responses_failure_reply(
                loop_state.last_failure.take(),
                loop_state.saw_inflight_saturation,
            ),
        )));
    }
    let remaining_cold_start_profiles = runtime_remaining_sync_probe_cold_start_profiles_for_route(
        context.shared,
        &loop_state.excluded_profiles,
        RuntimeRouteKind::Responses,
    )?;
    if remaining_cold_start_profiles > 0 {
        runtime_proxy_log(
            context.shared,
            format!(
                "request={} transport=http candidate_exhausted_continue route=responses remaining_cold_start_profiles={remaining_cold_start_profiles}",
                context.request_id
            ),
        );
        runtime_proxy_probe_refresh_pause(context.shared, RuntimeRouteKind::Responses);
        return Ok(RuntimeResponsesLoopControl::Continue);
    }
    if let Some(action) = try_runtime_responses_direct_current_profile_fallback(
        RuntimeResponsesDirectCurrentFallback {
            request_id: context.request_id,
            request: context.request,
            shared: context.shared,
            reason: RuntimeResponsesDirectCurrentFallbackReason::CandidateExhausted,
            previous_response_id: context.previous_response_id,
            prompt_cache_key: context.prompt_cache_key,
            request_turn_state: context.request_turn_state,
            request_session_id: context.request_session_id,
            request_requires_previous_response_affinity: context
                .request_requires_previous_response_affinity,
            previous_response_fresh_fallback_shape: context.previous_response_fresh_fallback_shape,
            saw_inflight_saturation: loop_state.saw_inflight_saturation,
        },
        affinity_state,
        &mut loop_state.excluded_profiles,
        &mut loop_state.last_failure,
        quota_last_chance_profile,
    )? {
        return Ok(match action {
            RuntimeResponsesDirectCurrentFallbackAction::Continue => {
                RuntimeResponsesLoopControl::Continue
            }
            RuntimeResponsesDirectCurrentFallbackAction::Return(response) => {
                RuntimeResponsesLoopControl::Return(response)
            }
        });
    }
    Ok(RuntimeResponsesLoopControl::Return(Box::new(
        runtime_proxy_final_responses_failure_reply(
            loop_state.last_failure.take(),
            loop_state.saw_inflight_saturation,
        ),
    )))
}

fn handle_runtime_responses_attempt(
    context: &RuntimeResponsesRequestContext<'_>,
    candidate_name: &str,
    turn_state_override: Option<&str>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    auto_redeemed_profiles: &mut BTreeSet<String>,
    quota_last_chance_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
) -> Result<Option<RuntimeResponsesReply>> {
    let hard_affinity = affinity_state.candidate_has_hard_affinity(candidate_name);
    let attempt = attempt_runtime_responses_request(
        context.request_id,
        context.request,
        context.shared,
        candidate_name,
        RuntimeResponsesAttemptOptions {
            turn_state_override,
            prompt_cache_key: context.prompt_cache_key,
            hard_affinity,
            selection_attempt: loop_state.selection_attempts,
        },
    )?;
    if !matches!(
        &attempt,
        RuntimeResponsesAttempt::LocalSelectionBlocked { .. }
    ) {
        loop_state.record_attempt();
    }
    match attempt {
        RuntimeResponsesAttempt::Success {
            profile_name,
            response,
        } => handle_runtime_responses_success(context, affinity_state, profile_name, response),
        RuntimeResponsesAttempt::QuotaBlocked {
            profile_name,
            response,
        } => handle_runtime_responses_quota_attempt(
            context,
            affinity_state,
            auto_redeemed_profiles,
            quota_last_chance_profile,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeResponsesAttempt::Overloaded {
            profile_name,
            response,
        } => {
            loop_state.record_overload_failure();
            handle_runtime_responses_overloaded_attempt(
                context,
                affinity_state,
                loop_state,
                profile_name,
                response,
            )
        }
        RuntimeResponsesAttempt::AuthFailed {
            profile_name,
            response,
        } => handle_runtime_responses_auth_failed(
            context,
            affinity_state,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name,
            reason,
        } => handle_runtime_responses_local_selection_attempt(
            context,
            affinity_state,
            loop_state,
            profile_name,
            reason,
        ),
        RuntimeResponsesAttempt::TransportFailed {
            profile_name,
            stage,
        } => {
            runtime_proxy_log(
                context.shared,
                format!(
                    "request={} transport=http responses_transport_failure profile={profile_name} stage={stage} hard_affinity={hard_affinity}",
                    context.request_id,
                ),
            );
            if hard_affinity {
                return Ok(Some(runtime_responses_local_selection_failure_reply()));
            }
            loop_state.record_transport_failure_at(stage);
            loop_state.excluded_profiles.insert(profile_name);
            Ok(None)
        }
        RuntimeResponsesAttempt::PreviousResponseNotFound {
            profile_name,
            response,
            turn_state,
            invalid_previous_response_id,
        } => handle_runtime_responses_previous_response_attempt(
            context,
            affinity_state,
            loop_state,
            profile_name,
            response,
            turn_state,
            invalid_previous_response_id,
        ),
    }
}

fn handle_runtime_responses_success(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    profile_name: String,
    response: RuntimeResponsesReply,
) -> Result<Option<RuntimeResponsesReply>> {
    affinity_state.remember_successful_previous_response_owner(
        context.shared,
        &profile_name,
        context.previous_response_id,
    )?;
    commit_runtime_proxy_profile_selection_with_notice(
        context.shared,
        &profile_name,
        RuntimeRouteKind::Responses,
    )?;
    runtime_proxy_log(
        context.shared,
        format!(
            "request={} transport=http committed profile={profile_name}",
            context.request_id
        ),
    );
    Ok(Some(response))
}

fn handle_runtime_responses_quota_attempt(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    auto_redeemed_profiles: &mut BTreeSet<String>,
    quota_last_chance_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    response: RuntimeResponsesReply,
) -> Result<Option<RuntimeResponsesReply>> {
    handle_runtime_responses_quota_blocked(RuntimeResponsesQuotaBlocked {
        request_id: context.request_id,
        shared: context.shared,
        profile_name,
        response,
        request_model_name: context.request_model_name,
        prompt_cache_key: context.prompt_cache_key,
        previous_response_id: context.previous_response_id,
        request_turn_state: context.request_turn_state,
        request_session_id: context.request_session_id,
        request_requires_previous_response_affinity: context
            .request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape: context.previous_response_fresh_fallback_shape,
        affinity_state,
        auto_redeemed_profiles,
        quota_last_chance_profile,
        excluded_profiles: &mut loop_state.excluded_profiles,
        last_failure: &mut loop_state.last_failure,
    })
}

fn handle_runtime_responses_overloaded_attempt(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    response: RuntimeResponsesReply,
) -> Result<Option<RuntimeResponsesReply>> {
    handle_runtime_responses_overloaded(RuntimeResponsesOverloaded {
        request_id: context.request_id,
        shared: context.shared,
        profile_name,
        response,
        affinity_state,
        excluded_profiles: &mut loop_state.excluded_profiles,
        last_failure: &mut loop_state.last_failure,
    })
}

fn handle_runtime_responses_auth_failed(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    response: RuntimeResponsesReply,
) -> Result<Option<RuntimeResponsesReply>> {
    runtime_proxy_log(
        context.shared,
        format!(
            "request={} transport=http auth_failed profile={profile_name}",
            context.request_id
        ),
    );
    if !affinity_state.quota_blocked_affinity_is_releasable(
        &profile_name,
        context.request_requires_previous_response_affinity,
        context.previous_response_fresh_fallback_shape,
    ) {
        runtime_proxy_log(
            context.shared,
            format!(
                "request={} transport=http upstream_auth_failure_passthrough route=responses profile={profile_name} reason=hard_affinity",
                context.request_id
            ),
        );
        return Ok(Some(response));
    }
    let released_affinity = release_runtime_auth_failed_affinity(
        context.shared,
        &profile_name,
        context.previous_response_id,
        context.request_turn_state,
        context.request_session_id,
    )?;
    affinity_state.clear_profile_affinity(&profile_name, true);
    if released_affinity {
        runtime_proxy_log(
            context.shared,
            format!(
                "request={} transport=http auth_failed_affinity_released profile={profile_name}",
                context.request_id
            ),
        );
    }
    loop_state.excluded_profiles.insert(profile_name);
    loop_state.last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
    Ok(None)
}

fn handle_runtime_responses_local_selection_attempt(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    reason: &'static str,
) -> Result<Option<RuntimeResponsesReply>> {
    handle_runtime_responses_local_selection_blocked(RuntimeResponsesLocalSelectionBlocked {
        request_id: context.request_id,
        request: context.request,
        shared: context.shared,
        selection_started_at: loop_state.selection_started_at,
        profile_name,
        reason,
        previous_response_id: context.previous_response_id,
        request_turn_state: context.request_turn_state,
        request_session_id: context.request_session_id,
        request_requires_previous_response_affinity: context
            .request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape: context.previous_response_fresh_fallback_shape,
        affinity_state,
        excluded_profiles: &mut loop_state.excluded_profiles,
    })
}
