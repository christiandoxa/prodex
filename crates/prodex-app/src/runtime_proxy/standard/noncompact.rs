use super::*;

pub(super) fn proxy_runtime_noncompact_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let current_profile = runtime_proxy_current_profile(shared)?;
    if is_runtime_realtime_call_path(&request.path_and_query) {
        return proxy_runtime_noncompact_realtime_request(
            request_id,
            request,
            shared,
            &current_profile,
        );
    }
    proxy_runtime_noncompact_standard_request(request_id, request, shared, current_profile)
}

fn proxy_runtime_noncompact_realtime_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    current_profile: &str,
) -> Result<tiny_http::ResponseBox> {
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
    runtime_selection_trace_log_direct(
        shared,
        request_id,
        RuntimeSelectionTraceDirect {
            requested_model: request_model_name.as_deref(),
            route_kind: RuntimeRouteKind::Standard,
            candidate_key: current_profile,
            class: runtime_proxy_crate::RuntimeRouteCandidateClass::Current,
            affinity_kind: Some(runtime_proxy_crate::RuntimeRouteAffinityKind::Strict),
            hard_affinity: true,
        },
    );
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http realtime_call_owner_pinned profile={current_profile} reason=sideband_auth_uses_current_profile"
        ),
    );
    match attempt_runtime_noncompact_standard_request_with_policy(
        request_id,
        request,
        shared,
        current_profile,
        false,
        true,
    )? {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => {
            commit_runtime_proxy_profile_selection_with_notice(
                shared,
                &profile_name,
                RuntimeRouteKind::Standard,
            )?;
            Ok(response)
        }
        RuntimeStandardAttempt::StaleContinuation { response }
        | RuntimeStandardAttempt::RetryableFailure { response, .. }
        | RuntimeStandardAttempt::ProfileUnavailable { response, .. }
        | RuntimeStandardAttempt::AuthFailed { response, .. } => Ok(response),
        RuntimeStandardAttempt::LocalSelectionBlocked { .. }
        | RuntimeStandardAttempt::TransportFailed { .. } => Ok(build_runtime_proxy_text_response(
            503,
            runtime_proxy_local_selection_failure_message(),
        )),
        RuntimeStandardAttempt::ProfileInflightSaturated { .. } => Ok(
            build_runtime_proxy_text_response(503, runtime_proxy_local_selection_failure_message()),
        ),
    }
}

fn proxy_runtime_noncompact_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    current_profile: String,
) -> Result<tiny_http::ResponseBox> {
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
    let request_session_id = runtime_request_session_id(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    let preferred_profile = session_profile
        .clone()
        .unwrap_or_else(|| current_profile.clone());
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Standard);
    let mut loop_state = RuntimePrecommitLoopState::<tiny_http::ResponseBox>::new();
    let (quota_summary, quota_source) = runtime_profile_quota_summary_for_route(
        shared,
        &preferred_profile,
        RuntimeRouteKind::Standard,
    )?;
    let preferred_is_session = session_profile.as_deref() == Some(preferred_profile.as_str());
    let preferred_profile_usable = if preferred_is_session {
        runtime_quota_summary_allows_soft_affinity(
            quota_summary,
            quota_source,
            RuntimeRouteKind::Standard,
        )
    } else {
        quota_summary.route_band != RuntimeQuotaPressureBand::Exhausted
    };
    if !preferred_profile_usable {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http {} profile={} reason={} quota_source={} {}",
                if preferred_is_session {
                    format!(
                        "selection_skip_affinity route={} affinity=session",
                        runtime_route_kind_label(RuntimeRouteKind::Standard)
                    )
                } else {
                    format!(
                        "selection_skip_current route={}",
                        runtime_route_kind_label(RuntimeRouteKind::Standard)
                    )
                },
                preferred_profile,
                if preferred_is_session {
                    runtime_quota_soft_affinity_rejection_reason(
                        quota_summary,
                        quota_source,
                        RuntimeRouteKind::Standard,
                    )
                } else {
                    runtime_quota_pressure_band_reason(quota_summary.route_band)
                },
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        loop_state
            .excluded_profiles
            .insert(preferred_profile.clone());
    }

    run_runtime_noncompact_standard_loop(RuntimeNoncompactStandardLoopContext {
        request_id,
        request,
        shared,
        request_model_name: request_model_name.as_deref(),
        request_session_id: request_session_id.as_deref(),
        preferred_profile: &preferred_profile,
        preferred_is_session,
        session_profile: &mut session_profile,
        pressure_mode,
        loop_state: &mut loop_state,
    })
}

struct RuntimeNoncompactStandardLoopContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    request_model_name: Option<&'a str>,
    request_session_id: Option<&'a str>,
    preferred_profile: &'a str,
    preferred_is_session: bool,
    session_profile: &'a mut Option<String>,
    pressure_mode: bool,
    loop_state: &'a mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
}

fn run_runtime_noncompact_standard_loop(
    context: RuntimeNoncompactStandardLoopContext<'_>,
) -> Result<tiny_http::ResponseBox> {
    let RuntimeNoncompactStandardLoopContext {
        request_id,
        request,
        shared,
        request_model_name,
        request_session_id,
        preferred_profile,
        preferred_is_session,
        session_profile,
        pressure_mode,
        loop_state,
    } = context;
    loop {
        if loop_state.budget_exhausted(shared, session_profile.is_some(), pressure_mode)? {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http standard_precommit_budget_exhausted attempts={} elapsed_ms={} pressure_mode={pressure_mode}",
                    loop_state.selection_attempts,
                    loop_state.selection_started_at.elapsed().as_millis()
                ),
            );
            if session_profile.is_none()
                && loop_state.maybe_wait_for_transient_recovery(
                    request_id,
                    shared,
                    RuntimeRouteKind::Standard,
                )?
            {
                continue;
            }
            return Ok(runtime_proxy_final_retryable_http_failure_response(
                loop_state.last_failure.take(),
                loop_state.saw_inflight_saturation,
                false,
            )
            .unwrap_or_else(|| {
                build_runtime_proxy_text_response(
                    503,
                    runtime_proxy_local_selection_failure_message(),
                )
            }));
        }

        let action = runtime_noncompact_next_action(
            request_id,
            shared,
            request_model_name,
            preferred_profile,
            preferred_is_session,
            session_profile.is_some(),
            &mut *loop_state,
        )?;
        let candidate_name = match action {
            RuntimePrecommitLoopAction::Continue => continue,
            RuntimePrecommitLoopAction::Attempt(candidate_name) => candidate_name,
            RuntimePrecommitLoopAction::Return(response) => return Ok(response),
        };
        loop_state.record_attempt();

        if runtime_noncompact_candidate_saturated(
            request_id,
            request,
            shared,
            &candidate_name,
            &mut *loop_state,
            session_profile.is_some(),
            session_profile.as_deref(),
        )? {
            continue;
        }

        let attempt = attempt_runtime_noncompact_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
            session_profile.as_deref() == Some(candidate_name.as_str()),
        )?;
        if let Some(response) = handle_runtime_noncompact_attempt(
            request_id,
            shared,
            request_session_id,
            !preferred_is_session,
            &mut *session_profile,
            &mut *loop_state,
            attempt,
        )? {
            return Ok(response);
        }
    }
}

fn runtime_noncompact_next_action(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request_model_name: Option<&str>,
    preferred_profile: &str,
    preferred_is_session: bool,
    session_present: bool,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
) -> Result<RuntimePrecommitLoopAction<String, tiny_http::ResponseBox>> {
    if loop_state.excluded_profiles.is_empty() {
        runtime_selection_trace_log_direct(
            shared,
            request_id,
            RuntimeSelectionTraceDirect {
                requested_model: request_model_name,
                route_kind: RuntimeRouteKind::Standard,
                candidate_key: preferred_profile,
                class: if preferred_is_session {
                    runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity
                } else {
                    runtime_proxy_crate::RuntimeRouteCandidateClass::Current
                },
                affinity_kind: preferred_is_session
                    .then_some(runtime_proxy_crate::RuntimeRouteAffinityKind::Session),
                hard_affinity: preferred_is_session,
            },
        );
        return Ok(RuntimePrecommitLoopAction::Attempt(
            preferred_profile.to_string(),
        ));
    }
    if let Some(candidate_name) = select_runtime_response_candidate_for_route_with_request(
        shared,
        RuntimeResponseCandidateSelection::fresh(
            &loop_state.excluded_profiles,
            RuntimeRouteKind::Standard,
        ),
        Some(request_id),
        request_model_name,
    )? {
        return Ok(RuntimePrecommitLoopAction::Attempt(candidate_name));
    }
    let remaining_cold_start_profiles = runtime_remaining_sync_probe_cold_start_profiles_for_route(
        shared,
        &loop_state.excluded_profiles,
        RuntimeRouteKind::Standard,
    )?;
    if remaining_cold_start_profiles > 0 && !session_present {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http candidate_exhausted_continue route=standard remaining_cold_start_profiles={remaining_cold_start_profiles}"
            ),
        );
        runtime_proxy_probe_refresh_pause(shared, RuntimeRouteKind::Standard);
        return Ok(RuntimePrecommitLoopAction::Continue);
    }
    if !session_present
        && loop_state.maybe_wait_for_transient_recovery(
            request_id,
            shared,
            RuntimeRouteKind::Standard,
        )?
    {
        return Ok(RuntimePrecommitLoopAction::Continue);
    }
    Ok(RuntimePrecommitLoopAction::Return(
        runtime_proxy_final_retryable_http_failure_response(
            loop_state.last_failure.take(),
            loop_state.saw_inflight_saturation,
            false,
        )
        .unwrap_or_else(|| {
            build_runtime_proxy_text_response(503, runtime_proxy_local_selection_failure_message())
        }),
    ))
}

fn runtime_noncompact_candidate_saturated(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    candidate_name: &str,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    continuation: bool,
    wait_affinity_owner: Option<&str>,
) -> Result<bool> {
    let hard_affinity = wait_affinity_owner == Some(candidate_name);
    if hard_affinity
        || !runtime_profile_inflight_hard_limited_for_context(
            shared,
            candidate_name,
            "standard_http",
        )?
    {
        return Ok(false);
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_inflight_saturated",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("profile", candidate_name),
                runtime_proxy_log_field(
                    "hard_limit",
                    shared
                        .runtime_config
                        .tuning
                        .profile_inflight_hard_limit
                        .to_string(),
                ),
            ],
        ),
    );
    loop_state.record_inflight_saturation();
    if runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
        request_id,
        request,
        shared,
        excluded_profiles: &loop_state.excluded_profiles,
        route_kind: RuntimeRouteKind::Standard,
        selection_started_at: loop_state.selection_started_at,
        continuation,
        wait_affinity_owner,
        selected_profile: Some(candidate_name),
    })? {
        return Ok(true);
    }
    loop_state
        .excluded_profiles
        .insert(candidate_name.to_string());
    Ok(true)
}

fn handle_runtime_noncompact_attempt(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    promote_committed_profile: bool,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    attempt: RuntimeStandardAttempt,
) -> Result<Option<tiny_http::ResponseBox>> {
    match attempt {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => {
            let _ = commit_runtime_proxy_profile_selection_with_policy(
                shared,
                &profile_name,
                RuntimeRouteKind::Standard,
                promote_committed_profile,
            )?;
            Ok(Some(response))
        }
        RuntimeStandardAttempt::StaleContinuation { response } => Ok(Some(response)),
        RuntimeStandardAttempt::RetryableFailure {
            profile_name,
            response,
            overload,
        } => handle_runtime_noncompact_retryable(RuntimeNoncompactRetryableContext {
            request_id,
            shared,
            request_session_id,
            session_profile,
            loop_state,
            profile_name,
            response,
            overload,
        }),
        RuntimeStandardAttempt::ProfileUnavailable {
            profile_name,
            response,
        } => handle_runtime_noncompact_profile_unavailable(
            request_id,
            shared,
            session_profile,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeStandardAttempt::AuthFailed {
            profile_name,
            response,
        } => handle_runtime_noncompact_auth_failed(
            request_id,
            shared,
            request_session_id,
            session_profile,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http local_selection_blocked profile={profile_name} route=standard reason=quota_exhausted_before_send"
                ),
            );
            clear_noncompact_session_profile(session_profile, &profile_name);
            loop_state.excluded_profiles.insert(profile_name);
            Ok(None)
        }
        RuntimeStandardAttempt::ProfileInflightSaturated { profile_name } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http local_selection_blocked profile={profile_name} route=standard reason=profile_inflight_saturated"
                ),
            );
            loop_state.record_inflight_saturation();
            clear_noncompact_session_profile(session_profile, &profile_name);
            loop_state.excluded_profiles.insert(profile_name);
            Ok(None)
        }
        RuntimeStandardAttempt::TransportFailed {
            profile_name,
            stage,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http standard_transport_failure profile={profile_name} stage={stage}"
                ),
            );
            if session_profile.as_deref() == Some(profile_name.as_str()) {
                return Ok(Some(build_runtime_proxy_text_response(
                    503,
                    runtime_proxy_local_selection_failure_message(),
                )));
            }
            loop_state.record_transport_failure_at(stage);
            loop_state.excluded_profiles.insert(profile_name);
            Ok(None)
        }
    }
}

struct RuntimeNoncompactRetryableContext<'a> {
    request_id: u64,
    shared: &'a RuntimeRotationProxyShared,
    request_session_id: Option<&'a str>,
    session_profile: &'a mut Option<String>,
    loop_state: &'a mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
    response: tiny_http::ResponseBox,
    overload: bool,
}

fn handle_runtime_noncompact_profile_unavailable(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
    response: tiny_http::ResponseBox,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_profile_unavailable profile={profile_name}"
        ),
    );
    if session_profile.as_deref() == Some(profile_name.as_str()) {
        return Ok(Some(response));
    }
    mark_runtime_profile_retry_backoff(shared, &profile_name)?;
    clear_noncompact_session_profile(session_profile, &profile_name);
    loop_state.excluded_profiles.insert(profile_name);
    loop_state.last_failure = Some((response, false));
    Ok(None)
}

fn handle_runtime_noncompact_retryable(
    context: RuntimeNoncompactRetryableContext<'_>,
) -> Result<Option<tiny_http::ResponseBox>> {
    let RuntimeNoncompactRetryableContext {
        request_id,
        shared,
        request_session_id,
        session_profile,
        loop_state,
        profile_name,
        response,
        overload,
    } = context;
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_retryable_failure profile={profile_name} reason={}",
            if overload { "overload" } else { "quota" }
        ),
    );
    if overload {
        loop_state.record_overload_failure();
    }
    mark_runtime_profile_retry_backoff(shared, &profile_name)?;
    let released_affinity = if overload {
        let _ = bump_runtime_profile_health_score(
            shared,
            &profile_name,
            RuntimeRouteKind::Standard,
            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
            "standard_overload",
        );
        let _ = bump_runtime_profile_bad_pairing_score(
            shared,
            &profile_name,
            RuntimeRouteKind::Standard,
            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
            "standard_overload",
        );
        false
    } else {
        release_runtime_quota_blocked_affinity(
            shared,
            &profile_name,
            None,
            None,
            request_session_id,
        )?
    };
    clear_noncompact_session_profile(session_profile, &profile_name);
    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=standard"
            ),
        );
    }
    if !overload
        && !runtime_has_route_eligible_quota_fallback(
            shared,
            &profile_name,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )?
    {
        return Ok(Some(response));
    }
    loop_state.excluded_profiles.insert(profile_name);
    loop_state.last_failure = Some((response, !overload));
    Ok(None)
}

fn handle_runtime_noncompact_auth_failed(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
    response: tiny_http::ResponseBox,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!("request={request_id} transport=http standard_auth_failed profile={profile_name}"),
    );
    let released_affinity = release_runtime_auth_failed_affinity(
        shared,
        &profile_name,
        None,
        None,
        request_session_id,
    )?;
    clear_noncompact_session_profile(session_profile, &profile_name);
    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http auth_failed_affinity_released profile={profile_name} route=standard"
            ),
        );
    }
    loop_state.excluded_profiles.insert(profile_name);
    loop_state.last_failure = Some((response, true));
    Ok(None)
}

fn clear_noncompact_session_profile(session_profile: &mut Option<String>, profile_name: &str) {
    if session_profile.as_deref() == Some(profile_name) {
        *session_profile = None;
    }
}
