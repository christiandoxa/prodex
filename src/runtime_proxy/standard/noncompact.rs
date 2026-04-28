use super::*;

pub(super) fn proxy_runtime_noncompact_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_session_id = runtime_request_session_id(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    let current_profile = runtime_proxy_current_profile(shared)?;
    if is_runtime_realtime_call_path(&request.path_and_query) {
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
                            runtime_proxy_profile_inflight_hard_limit().to_string(),
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
            RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
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
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure: Option<(tiny_http::ResponseBox, bool)> = None;
    let mut saw_inflight_saturation = false;
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
        excluded_profiles.insert(preferred_profile.clone());
    }

    loop {
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            session_profile.is_some(),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http standard_precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            return Ok(runtime_proxy_final_retryable_http_failure_response(
                last_failure,
                saw_inflight_saturation,
                false,
            )
            .unwrap_or_else(|| {
                build_runtime_proxy_text_response(
                    503,
                    runtime_proxy_local_selection_failure_message(),
                )
            }));
        }

        let candidate_name = if excluded_profiles.is_empty() {
            preferred_profile.clone()
        } else if let Some(candidate_name) =
            select_runtime_response_candidate_for_route_with_selection(
                shared,
                RuntimeResponseCandidateSelection {
                    excluded_profiles: &excluded_profiles,
                    strict_affinity_profile: None,
                    pinned_profile: None,
                    turn_state_profile: None,
                    session_profile: None,
                    discover_previous_response_owner: false,
                    previous_response_id: None,
                    route_kind: RuntimeRouteKind::Standard,
                },
            )?
        {
            candidate_name
        } else {
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
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
                continue;
            }
            return Ok(runtime_proxy_final_retryable_http_failure_response(
                last_failure,
                saw_inflight_saturation,
                false,
            )
            .unwrap_or_else(|| {
                build_runtime_proxy_text_response(
                    503,
                    runtime_proxy_local_selection_failure_message(),
                )
            }));
        };
        selection_attempts = selection_attempts.saturating_add(1);

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
                            runtime_proxy_profile_inflight_hard_limit().to_string(),
                        ),
                    ],
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
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
                overload: _,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http standard_retryable_failure profile={profile_name}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                let released_affinity = release_runtime_quota_blocked_affinity(
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
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=standard"
                        ),
                    );
                }
                if !runtime_has_route_eligible_quota_fallback(
                    shared,
                    &profile_name,
                    &BTreeSet::new(),
                    RuntimeRouteKind::Standard,
                )? {
                    return Ok(response);
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((response, true));
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
                excluded_profiles.insert(profile_name);
            }
        }
    }
}
