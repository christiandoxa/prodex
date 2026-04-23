use super::*;

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
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Responses)
        })
        .transpose()?
        .flatten();
    let mut trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
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
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut bound_session_profile = None;
    let mut compact_followup_profile = None;
    let mut compact_session_profile = None;
    let mut session_profile = None;
    let mut pinned_profile = None;
    macro_rules! recompute_route_affinity {
        ($reason:expr) => {
            refresh_and_log_runtime_response_route_affinity(
                shared,
                request_id,
                None,
                $reason,
                previous_response_id.as_deref(),
                bound_profile.as_deref(),
                turn_state_profile.as_deref(),
                request_turn_state.as_deref(),
                request_session_id.as_deref(),
                explicit_request_session_id.as_ref(),
                None,
                &mut bound_session_profile,
                &mut compact_followup_profile,
                &mut compact_session_profile,
                &mut session_profile,
                &mut pinned_profile,
            )
        };
    }
    recompute_route_affinity!("initial")?;
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure: Option<(RuntimeUpstreamFailureResponse, bool)> = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;
    macro_rules! runtime_candidate_affinity {
        ($route_kind:expr, $candidate_name:expr, $strict_affinity_profile:expr $(,)?) => {
            RuntimeCandidateAffinity {
                route_kind: $route_kind,
                candidate_name: $candidate_name,
                strict_affinity_profile: $strict_affinity_profile,
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                trusted_previous_response_affinity,
            }
        };
    }
    macro_rules! runtime_responses_previous_response_not_found_context {
        ($profile_name:expr, $turn_state:expr, $via:expr, $policy:expr $(,)?) => {
            RuntimePreviousResponseNotFoundContext {
                shared,
                log_context: RuntimePreviousResponseLogContext {
                    request_id,
                    transport: "http",
                    route: "responses",
                    websocket_session: None,
                    via: $via,
                },
                route: RuntimePreviousResponseNotFoundRoute::Responses,
                route_kind: RuntimeRouteKind::Responses,
                profile_name: $profile_name,
                turn_state: $turn_state,
                previous_response_id: previous_response_id.as_deref(),
                request_turn_state: request_turn_state.as_deref(),
                request_session_id: request_session_id.as_deref(),
                request_requires_previous_response_affinity,
                trusted_previous_response_affinity,
                previous_response_fresh_fallback_used,
                fresh_fallback_shape: previous_response_fresh_fallback_shape,
                policy: $policy,
            }
        };
    }
    macro_rules! runtime_responses_previous_response_not_found_state {
        ($trusted_previous_response_affinity:expr $(,)?) => {
            RuntimePreviousResponseNotFoundState {
                saw_previous_response_not_found: &mut saw_previous_response_not_found,
                previous_response_retry_candidate: &mut previous_response_retry_candidate,
                previous_response_retry_index: &mut previous_response_retry_index,
                candidate_turn_state_retry_profile: &mut candidate_turn_state_retry_profile,
                candidate_turn_state_retry_value: &mut candidate_turn_state_retry_value,
                bound_profile: &mut bound_profile,
                session_profile: &mut session_profile,
                pinned_profile: &mut pinned_profile,
                turn_state_profile: &mut turn_state_profile,
                compact_followup_profile: Some(&mut compact_followup_profile),
                excluded_profiles: &mut excluded_profiles,
                trusted_previous_response_affinity: $trusted_previous_response_affinity,
            }
        };
    }

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Responses);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
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
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
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
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                            previous_response_fresh_fallback_shape,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
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
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        match handle_runtime_previous_response_not_found(
                            runtime_responses_previous_response_not_found_context!(
                                &profile_name,
                                turn_state,
                                Some("direct_current_profile_fallback"),
                                RuntimePreviousResponseNotFoundPolicy::responses(false),
                            ),
                            runtime_responses_previous_response_not_found_state!(None),
                        )? {
                            RuntimePreviousResponseNotFoundAction::RetryOwner
                            | RuntimePreviousResponseNotFoundAction::Rotate => {
                                last_failure =
                                    Some((RuntimeUpstreamFailureResponse::Http(response), false));
                                continue;
                            }
                            RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                                unreachable!(
                                    "responses previous_response policy cannot return this action"
                                )
                            }
                        }
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        match runtime_responses_local_selection_action(
                            runtime_quota_blocked_affinity_is_releasable(
                                runtime_candidate_affinity!(
                                    RuntimeRouteKind::Responses,
                                    &profile_name,
                                    compact_followup_profile
                                        .as_ref()
                                        .map(|(profile_name, _)| profile_name.as_str()),
                                ),
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
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
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
            RuntimeResponseCandidateSelection {
                excluded_profiles: &excluded_profiles,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                discover_previous_response_owner: previous_response_id.is_some(),
                previous_response_id: previous_response_id.as_deref(),
                route_kind: RuntimeRouteKind::Responses,
            },
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
                    runtime_proxy_has_continuation_priority(
                        previous_response_id.as_deref(),
                        pinned_profile.as_deref(),
                        request_turn_state.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                    ),
                    runtime_wait_affinity_owner(
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                        trusted_previous_response_affinity,
                    ),
                ),
            )? {
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
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
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                            previous_response_fresh_fallback_shape,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
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
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        match handle_runtime_previous_response_not_found(
                            runtime_responses_previous_response_not_found_context!(
                                &profile_name,
                                turn_state,
                                Some("direct_current_profile_fallback"),
                                RuntimePreviousResponseNotFoundPolicy::responses(false),
                            ),
                            runtime_responses_previous_response_not_found_state!(None),
                        )? {
                            RuntimePreviousResponseNotFoundAction::RetryOwner
                            | RuntimePreviousResponseNotFoundAction::Rotate => {
                                last_failure =
                                    Some((RuntimeUpstreamFailureResponse::Http(response), false));
                                continue;
                            }
                            RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                                unreachable!(
                                    "responses previous_response policy cannot return this action"
                                )
                            }
                        }
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        match runtime_responses_local_selection_action(
                            runtime_quota_blocked_affinity_is_releasable(
                                runtime_candidate_affinity!(
                                    RuntimeRouteKind::Responses,
                                    &profile_name,
                                    compact_followup_profile
                                        .as_ref()
                                        .map(|(profile_name, _)| profile_name.as_str()),
                                ),
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
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
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
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "responses_http",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
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
                    runtime_proxy_has_continuation_priority(
                        previous_response_id.as_deref(),
                        pinned_profile.as_deref(),
                        request_turn_state.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                    ),
                    runtime_wait_affinity_owner(
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                        trusted_previous_response_affinity,
                    ),
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
        )? {
            RuntimeResponsesAttempt::Success {
                profile_name,
                response,
            } => {
                if saw_previous_response_not_found {
                    remember_runtime_successful_previous_response_owner(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                        RuntimeRouteKind::Responses,
                    )?;
                }
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
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Responses,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
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
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
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
                    runtime_quota_blocked_affinity_is_releasable(
                        runtime_candidate_affinity!(
                            RuntimeRouteKind::Responses,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                        ),
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
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
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
                    runtime_responses_previous_response_not_found_context!(
                        &profile_name,
                        turn_state,
                        None,
                        RuntimePreviousResponseNotFoundPolicy::responses(true),
                    ),
                    runtime_responses_previous_response_not_found_state!(Some(
                        &mut trusted_previous_response_affinity
                    )),
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

enum RuntimeResponsesLocalSelectionAction {
    ReturnServiceUnavailable,
    Rotate,
}

fn runtime_responses_local_selection_action(
    releasable: bool,
) -> RuntimeResponsesLocalSelectionAction {
    if releasable {
        RuntimeResponsesLocalSelectionAction::Rotate
    } else {
        RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable
    }
}

fn runtime_responses_local_selection_failure_reply() -> RuntimeResponsesReply {
    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
        503,
        "service_unavailable",
        runtime_proxy_local_selection_failure_message(),
    ))
}

pub(crate) fn attempt_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeResponsesAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let request_previous_response_id = runtime_request_previous_response_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Responses)?;
    if (request_previous_response_id.is_some()
        || request_session_id.is_some()
        || request_turn_state.is_some())
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_quota_precommit_guard_reason(initial_quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                initial_quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(initial_quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let has_alternative_quota_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        profile_name,
        &BTreeSet::new(),
        RuntimeRouteKind::Responses,
    )?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        RuntimeRouteKind::Responses,
        "responses_precommit_reprobe",
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        RuntimeRouteKind::Responses,
    ) && has_alternative_quota_profile
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason=quota_windows_unavailable_after_reprobe quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "quota_windows_unavailable_after_reprobe",
        });
    }
    if let Some(reason) =
        runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "responses_http")?;
    let mut inflight_guard = Some(inflight_guard);
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    loop {
        let response = send_runtime_proxy_upstream_responses_request(
            request_id,
            request,
            shared,
            profile_name,
            turn_state_override,
        )
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Responses,
                "responses_upstream_request",
                err,
            );
        })?;
        let response_turn_state =
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
                .inspect_err(|err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Responses,
                        "responses_buffer_response",
                        err,
                    );
                })?;
            if status == 401
                && runtime_try_recover_profile_auth_from_unauthorized_steps(
                    request_id,
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    &mut recovery_steps,
                )
            {
                continue;
            }
            let retryable_quota = matches!(status, 403 | 429)
                && extract_runtime_proxy_quota_message(&parts.body).is_some();
            let retryable_previous = status == 400
                && extract_runtime_proxy_previous_response_message(&parts.body).is_some();
            let response = RuntimeResponsesReply::Buffered(parts);

            if retryable_quota {
                return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                    profile_name: profile_name.to_string(),
                    response,
                });
            }
            if retryable_previous {
                return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                    profile_name: profile_name.to_string(),
                    response,
                    turn_state: response_turn_state,
                });
            }
            if matches!(status, 401 | 403) {
                note_runtime_profile_auth_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    status,
                );
            }

            return Ok(RuntimeResponsesAttempt::Success {
                profile_name: profile_name.to_string(),
                response,
            });
        }
        let prepared = prepare_runtime_proxy_responses_success(
            RuntimeResponsesSuccessContext {
                request_id,
                request_previous_response_id: runtime_request_previous_response_id(request)
                    .as_deref(),
                request_session_id: request_session_id.as_deref(),
                request_turn_state: runtime_request_turn_state(request).as_deref(),
                turn_state_override,
                shared,
                profile_name,
                inflight_guard: inflight_guard
                    .take()
                    .expect("responses inflight guard should be present"),
            },
            response,
        )
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Responses,
                "responses_prepare_success",
                err,
            );
        });
        return prepared;
    }
}
