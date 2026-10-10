use super::*;
use prodex_mojo_core::runtime::ResponsesLoopAction::*;

pub(super) struct RuntimeResponsesRequestContext<'a> {
    pub(super) request_id: u64,
    pub(super) request: RuntimeProxyRequest,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) request_requires_previous_response_affinity: bool,
    pub(super) previous_response_fresh_fallback_shape:
        Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) prompt_cache_key: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_model_name: Option<String>,
}
pub(super) enum RuntimeResponsesLoopControl {
    Continue,
    Return(Box<RuntimeResponsesReply>),
}
struct RuntimeResponsesLoopPhaseFacts {
    phase: ResponsesLoopPhase,
    budget_exhausted: bool,
}
fn runtime_responses_loop_action(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &RuntimeResponsesAffinityState,
    loop_state: &RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    phase_facts: RuntimeResponsesLoopPhaseFacts,
    transient_recovery_pending: bool,
    inflight_relief_pending: bool,
    cold_start_pending: bool,
) -> Result<ResponsesLoopAction> {
    prodex_mojo_core::runtime::responses_loop_action(ResponsesLoopInput {
        phase: phase_facts.phase,
        budget_exhausted: phase_facts.budget_exhausted,
        hard_affinity: affinity_state.wait_affinity_owner().is_some(),
        compact_followup: affinity_state.compact_followup_profile().is_some(),
        transient_recovery_pending,
        inflight_relief_pending,
        cold_start_pending,
        direct_fallback_allowed: affinity_state.allows_direct_current_profile_fallback(
            context.previous_response_id,
            context.request_turn_state,
            loop_state.saw_inflight_saturation,
            loop_state.last_failure.is_some(),
        ),
        stream_committed: false,
    })
    .map_err(|error| anyhow::anyhow!("Mojo Responses loop planning failed: {error:?}"))
}
fn runtime_responses_final_failure_control(
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
) -> RuntimeResponsesLoopControl {
    RuntimeResponsesLoopControl::Return(Box::new(runtime_proxy_final_responses_failure_reply(
        loop_state.last_failure.take(),
        loop_state.saw_inflight_saturation,
    )))
}
fn try_runtime_responses_direct_fallback(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    reason: RuntimeResponsesDirectCurrentFallbackReason,
    quota_last_chance_profile: &mut Option<String>,
) -> Result<Option<RuntimeResponsesDirectCurrentFallbackAction>> {
    try_runtime_responses_direct_current_profile_fallback(
        RuntimeResponsesDirectCurrentFallback {
            request_id: context.request_id,
            request: &context.request,
            shared: context.shared,
            reason,
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
    )
}
fn runtime_responses_direct_fallback_control(
    action: RuntimeResponsesDirectCurrentFallbackAction,
) -> RuntimeResponsesLoopControl {
    match action {
        Continue => RuntimeResponsesLoopControl::Continue,
        Return(response) => RuntimeResponsesLoopControl::Return(response),
    }
}
pub(crate) fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let mut request = request.clone();
    let mut request_turn_state = runtime_request_turn_state(&request);
    if let Some(turn_state) = request_turn_state.as_deref()
        && runtime_turn_state_is_dead_recovery_token(shared, turn_state)?
    {
        request = runtime_proxy_crate::runtime_request_without_turn_state(&request);
        runtime_proxy_log(
            shared,
            format!("request={request_id} transport=http dead_turn_state_replay scrubbed=true"),
        );
        request_turn_state = None;
    }
    let request_requires_previous_response_affinity =
        runtime_request_requires_previous_response_affinity(&request);
    let previous_response_fresh_fallback_shape =
        runtime_request_previous_response_fresh_fallback_shape(&request);
    let previous_response_id = runtime_request_previous_response_id(&request);
    let explicit_request_session_id = runtime_request_explicit_session_id(&request);
    let request_session_id = runtime_request_session_id(&request);
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
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
    let mut context = RuntimeResponsesRequestContext {
        request_id,
        request,
        shared,
        request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape,
        previous_response_id: previous_response_id.as_deref(),
        prompt_cache_key: prompt_cache_key.as_deref(),
        request_turn_state: request_turn_state.as_deref(),
        request_session_id: request_session_id.as_deref(),
        request_model_name,
    };
    run_runtime_responses_loop(
        &mut context,
        &mut affinity_state,
        &mut auto_redeemed_profiles,
        &mut quota_last_chance_profile,
        &mut loop_state,
    )
}

pub(super) fn run_runtime_responses_loop(
    context: &mut RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    auto_redeemed_profiles: &mut BTreeSet<String>,
    quota_last_chance_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
) -> Result<RuntimeResponsesReply> {
    loop {
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

        let release_revision = runtime_profile_inflight_release_revision(context.shared);
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
                release_revision,
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
            context.request_model_name.as_deref(),
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
        observed_release_revision: None,
        request_id: context.request_id,
        shared: context.shared,
        excluded_profiles: &loop_state.excluded_profiles,
        route_kind: RuntimeRouteKind::Responses,
        selection_started_at: &mut loop_state.selection_started_at,
        continuation: affinity_state
            .has_continuation_priority(context.previous_response_id, context.request_turn_state),
        wait_affinity_owner: affinity_state.wait_affinity_owner(),
        selected_profile: None,
    })? {
        RuntimeInflightReliefWaitResult::Relieved
        | RuntimeInflightReliefWaitResult::NotWaitable => Ok(true),
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
    let budget_exhausted = loop_state.budget_exhausted(
        context.shared,
        RuntimeRouteKind::Responses,
        affinity_state
            .has_continuation_priority(context.previous_response_id, context.request_turn_state),
        pressure_mode,
    )?;
    if !budget_exhausted {
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
    let mut transient_recovery_pending = affinity_state.wait_affinity_owner().is_none()
        && runtime_route_has_retryable_profile(context.shared, RuntimeRouteKind::Responses)?;
    loop {
        match runtime_responses_loop_action(
            context,
            affinity_state,
            loop_state,
            RuntimeResponsesLoopPhaseFacts {
                phase: ResponsesLoopPhase::BudgetExhausted,
                budget_exhausted,
            },
            transient_recovery_pending,
            false,
            false,
        )? {
            WaitTransientRecovery => {
                if affinity_state.wait_affinity_owner().is_none()
                    && loop_state.maybe_wait_for_transient_recovery(
                        context.request_id,
                        context.shared,
                        RuntimeRouteKind::Responses,
                    )?
                {
                    return Ok(Some(RuntimeResponsesLoopControl::Continue));
                }
                transient_recovery_pending = false;
            }
            ReturnCompactFailure => {
                log_runtime_responses_compact_fallback_blocked(
                    context,
                    affinity_state,
                    "precommit_budget_exhausted",
                );
                return Ok(Some(runtime_responses_final_failure_control(loop_state)));
            }
            DirectFallback => {
                let action = try_runtime_responses_direct_fallback(
                    context,
                    affinity_state,
                    loop_state,
                    RuntimeResponsesDirectCurrentFallbackReason::PrecommitBudgetExhausted,
                    quota_last_chance_profile,
                )?;
                return Ok(Some(
                    action
                        .map(runtime_responses_direct_fallback_control)
                        .unwrap_or_else(|| runtime_responses_final_failure_control(loop_state)),
                ));
            }
            ReturnFinalFailure | ReturnWithoutRotation => {
                return Ok(Some(runtime_responses_final_failure_control(loop_state)));
            }
            Attempt | WaitInflightRelief | WaitColdStart => {
                unreachable!("invalid Mojo Responses budget loop action")
            }
        }
    }
}
fn log_runtime_responses_compact_fallback_blocked(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &RuntimeResponsesAffinityState,
    reason: &str,
) {
    if let Some((profile_name, source)) = affinity_state.compact_followup_profile() {
        runtime_proxy_log(
            context.shared,
            format!(
                "request={} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason={reason}",
                context.request_id
            ),
        );
    }
}

fn handle_runtime_responses_candidate_exhausted(
    context: &mut RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    quota_last_chance_profile: &mut Option<String>,
    release_revision: u64,
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
    let mut transient_recovery_pending = affinity_state.wait_affinity_owner().is_none()
        && runtime_route_has_retryable_profile(context.shared, RuntimeRouteKind::Responses)?;
    let mut inflight_relief_pending = true;
    let mut cold_start_pending = true;
    loop {
        match runtime_responses_loop_action(
            context,
            affinity_state,
            loop_state,
            RuntimeResponsesLoopPhaseFacts {
                phase: ResponsesLoopPhase::CandidateExhausted,
                budget_exhausted: true,
            },
            transient_recovery_pending,
            inflight_relief_pending,
            cold_start_pending,
        )? {
            WaitTransientRecovery => {
                if affinity_state.wait_affinity_owner().is_none()
                    && loop_state.maybe_wait_for_transient_recovery(
                        context.request_id,
                        context.shared,
                        RuntimeRouteKind::Responses,
                    )?
                {
                    return Ok(RuntimeResponsesLoopControl::Continue);
                }
                transient_recovery_pending = false;
            }
            WaitInflightRelief => {
                match runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                    RuntimeInflightReliefWait {
                        observed_release_revision: Some(release_revision),
                        request_id: context.request_id,
                        shared: context.shared,
                        excluded_profiles: &loop_state.excluded_profiles,
                        route_kind: RuntimeRouteKind::Responses,
                        selection_started_at: &mut loop_state.selection_started_at,
                        continuation: affinity_state.has_continuation_priority(
                            context.previous_response_id,
                            context.request_turn_state,
                        ),
                        wait_affinity_owner: affinity_state.wait_affinity_owner(),
                        selected_profile: None,
                    },
                )? {
                    RuntimeInflightReliefWaitResult::Relieved => {
                        return Ok(RuntimeResponsesLoopControl::Continue);
                    }
                    RuntimeInflightReliefWaitResult::NotWaitable => {
                        inflight_relief_pending = false;
                    }
                }
            }
            ReturnCompactFailure => {
                log_runtime_responses_compact_fallback_blocked(
                    context,
                    affinity_state,
                    "candidate_exhausted",
                );
                return Ok(runtime_responses_final_failure_control(loop_state));
            }
            WaitColdStart => {
                let remaining_cold_start_profiles =
                    runtime_remaining_sync_probe_cold_start_profiles_for_route(
                        context.shared,
                        &loop_state.excluded_profiles,
                        RuntimeRouteKind::Responses,
                    )?;
                if remaining_cold_start_profiles > 0 && loop_state.claim_cold_start_probe_wait() {
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
                cold_start_pending = false;
            }
            DirectFallback => {
                let action = try_runtime_responses_direct_fallback(
                    context,
                    affinity_state,
                    loop_state,
                    RuntimeResponsesDirectCurrentFallbackReason::CandidateExhausted,
                    quota_last_chance_profile,
                )?;
                return Ok(action
                    .map(runtime_responses_direct_fallback_control)
                    .unwrap_or_else(|| runtime_responses_final_failure_control(loop_state)));
            }
            ReturnFinalFailure | ReturnWithoutRotation => {
                return Ok(runtime_responses_final_failure_control(loop_state));
            }
            Attempt => {
                unreachable!("invalid Mojo Responses candidate loop action")
            }
        }
    }
}
