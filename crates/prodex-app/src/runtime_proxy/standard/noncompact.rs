use super::*;

pub(super) fn proxy_runtime_noncompact_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
    let request_session_id = runtime_request_session_id(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    let current_profile = runtime_proxy_current_profile(shared)?;
    if is_runtime_realtime_call_path(&request.path_and_query) {
        runtime_selection_trace_log_direct(
            shared,
            request_id,
            RuntimeSelectionTraceDirect {
                requested_model: request_model_name.as_deref(),
                route_kind: RuntimeRouteKind::Standard,
                candidate_key: &current_profile,
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
        if runtime_profile_inflight_hard_limited_for_context(
            shared,
            &current_profile,
            "standard_http",
        )? {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "profile_inflight_saturated",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", current_profile.as_str()),
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
            return Ok(build_runtime_proxy_text_response(
                503,
                "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
            ));
        }

        return match attempt_runtime_noncompact_standard_request_with_policy(
            request_id,
            request,
            shared,
            &current_profile,
            false,
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
            RuntimeStandardAttempt::StaleContinuation { response } => Ok(response),
            RuntimeStandardAttempt::RetryableFailure { response, .. } => Ok(response),
            RuntimeStandardAttempt::AuthFailed { response, .. } => Ok(response),
            RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
                Ok(build_runtime_proxy_text_response(
                    503,
                    runtime_proxy_local_selection_failure_message(),
                ))
            }
            RuntimeStandardAttempt::TransportFailed { .. } => {
                Ok(build_runtime_proxy_text_response(
                    503,
                    runtime_proxy_local_selection_failure_message(),
                ))
            }
        };
    }
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
            return Ok(runtime_proxy_final_retryable_http_failure_response(
                loop_state.last_failure,
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

        let action = if loop_state.excluded_profiles.is_empty() {
            runtime_selection_trace_log_direct(
                shared,
                request_id,
                RuntimeSelectionTraceDirect {
                    requested_model: request_model_name.as_deref(),
                    route_kind: RuntimeRouteKind::Standard,
                    candidate_key: &preferred_profile,
                    class: if preferred_is_session {
                        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity
                    } else {
                        runtime_proxy_crate::RuntimeRouteCandidateClass::Current
                    },
                    affinity_kind: preferred_is_session
                        .then_some(runtime_proxy_crate::RuntimeRouteAffinityKind::Session),
                    hard_affinity: false,
                },
            );
            RuntimePrecommitLoopAction::Attempt(preferred_profile.clone())
        } else if let Some(candidate_name) =
            select_runtime_response_candidate_for_route_with_request(
                shared,
                RuntimeResponseCandidateSelection::fresh(
                    &loop_state.excluded_profiles,
                    RuntimeRouteKind::Standard,
                ),
                Some(request_id),
                request_model_name.as_deref(),
            )?
        {
            RuntimePrecommitLoopAction::Attempt(candidate_name)
        } else {
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &loop_state.excluded_profiles,
                    RuntimeRouteKind::Standard,
                )?;
            if remaining_cold_start_profiles > 0 && session_profile.is_none() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http candidate_exhausted_continue route=standard remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Standard);
                RuntimePrecommitLoopAction::Continue
            } else {
                RuntimePrecommitLoopAction::Return(
                    runtime_proxy_final_retryable_http_failure_response(
                        loop_state.last_failure.take(),
                        loop_state.saw_inflight_saturation,
                        false,
                    )
                    .unwrap_or_else(|| {
                        build_runtime_proxy_text_response(
                            503,
                            runtime_proxy_local_selection_failure_message(),
                        )
                    }),
                )
            }
        };
        let candidate_name = match action {
            RuntimePrecommitLoopAction::Continue => continue,
            RuntimePrecommitLoopAction::Attempt(candidate_name) => candidate_name,
            RuntimePrecommitLoopAction::Return(response) => return Ok(response),
        };
        loop_state.record_attempt();

        if runtime_profile_inflight_hard_limited_for_context(
            shared,
            &candidate_name,
            "standard_http",
        )? {
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
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait {
                    request_id,
                    request,
                    shared,
                    excluded_profiles: &loop_state.excluded_profiles,
                    route_kind: RuntimeRouteKind::Standard,
                    selection_started_at: loop_state.selection_started_at,
                    continuation: session_profile.is_some(),
                    wait_affinity_owner: session_profile.as_deref(),
                },
            )? {
                continue;
            }
            loop_state.excluded_profiles.insert(candidate_name);
            continue;
        }

        match attempt_runtime_noncompact_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
        )? {
            RuntimeStandardAttempt::Success {
                profile_name: _,
                response,
            } => return Ok(response),
            RuntimeStandardAttempt::StaleContinuation { response } => return Ok(response),
            RuntimeStandardAttempt::RetryableFailure {
                profile_name,
                response,
                overload,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http standard_retryable_failure profile={profile_name} reason={}",
                        if overload { "overload" } else { "quota" }
                    ),
                );
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
                        request_session_id.as_deref(),
                    )?
                };
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
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
                    return Ok(response);
                }
                loop_state.excluded_profiles.insert(profile_name);
                loop_state.last_failure = Some((response, !overload));
            }
            RuntimeStandardAttempt::AuthFailed {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http standard_auth_failed profile={profile_name}"
                    ),
                );
                let released_affinity = release_runtime_auth_failed_affinity(
                    shared,
                    &profile_name,
                    None,
                    None,
                    request_session_id.as_deref(),
                )?;
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
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
            }
            RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=standard reason=quota_exhausted_before_send"
                    ),
                );
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                loop_state.excluded_profiles.insert(profile_name);
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
                    return Ok(build_runtime_proxy_text_response(
                        503,
                        runtime_proxy_local_selection_failure_message(),
                    ));
                }
                loop_state.excluded_profiles.insert(profile_name);
            }
        }
    }
}
