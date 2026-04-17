use super::*;

pub(crate) fn proxy_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_session_id = runtime_request_session_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    if !is_runtime_compact_path(&request.path_and_query) {
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
                    format!(
                        "request={request_id} transport=http profile_inflight_saturated profile={current_profile} hard_limit={}",
                        runtime_proxy_profile_inflight_hard_limit(),
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
                    format!(
                        "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                        runtime_proxy_profile_inflight_hard_limit(),
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

    let current_profile = runtime_proxy_current_profile(shared)?;
    let mut compact_followup_profile = runtime_compact_route_followup_bound_profile(
        shared,
        request_turn_state.as_deref(),
        request_session_id.as_deref(),
    )?;
    if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
    let initial_compact_affinity_profile = compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.as_str())
        .or(session_profile.as_deref());
    let compact_owner_profile = compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.clone())
        .or(session_profile.clone())
        .unwrap_or_else(|| current_profile.clone());
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Compact);
    if runtime_proxy_should_shed_fresh_compact_request(
        pressure_mode,
        initial_compact_affinity_profile,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_pressure_shed reason=fresh_request pressure_mode={pressure_mode}"
            ),
        );
        return Ok(build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            "Fresh compact requests are temporarily deferred while the runtime proxy is under pressure. Retry the request.",
        ));
    }
    let mut excluded_profiles = BTreeSet::new();
    let mut conservative_overload_retried_profiles = BTreeSet::new();
    let mut last_failure: Option<(tiny_http::ResponseBox, bool)> = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;

    loop {
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            compact_followup_profile.is_some() || session_profile.is_some(),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if let Some(response) = runtime_proxy_final_retryable_http_failure_response(
                last_failure,
                saw_inflight_saturation,
                true,
            ) {
                return Ok(response);
            }
            if compact_followup_profile.is_some() || session_profile.is_some() {
                return Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                ));
            }
            return match attempt_runtime_standard_request(
                request_id,
                request,
                shared,
                &compact_owner_profile,
                runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                    route_kind: RuntimeRouteKind::Compact,
                    candidate_name: &compact_owner_profile,
                    strict_affinity_profile: compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile: None,
                    turn_state_profile: None,
                    session_profile: session_profile.as_deref(),
                    trusted_previous_response_affinity: false,
                }),
            )? {
                RuntimeStandardAttempt::Success {
                    profile_name,
                    response,
                } => {
                    commit_runtime_proxy_profile_selection_with_notice(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                    )?;
                    Ok(response)
                }
                RuntimeStandardAttempt::RetryableFailure { response, .. } => Ok(response),
                RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
                    Ok(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    ))
                }
            };
        }
        selection_attempts = selection_attempts.saturating_add(1);

        let Some(candidate_name) = select_runtime_response_candidate_for_route_with_selection(
            shared,
            RuntimeResponseCandidateSelection {
                excluded_profiles: &excluded_profiles,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: None,
                turn_state_profile: None,
                session_profile: session_profile.as_deref(),
                discover_previous_response_owner: false,
                previous_response_id: None,
                route_kind: RuntimeRouteKind::Compact,
            },
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_candidate_exhausted last_failure={}",
                    if last_failure.is_some() {
                        "http"
                    } else {
                        "none"
                    }
                ),
            );
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Compact,
                )?;
            if remaining_cold_start_profiles > 0
                && compact_followup_profile.is_none()
                && session_profile.is_none()
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http candidate_exhausted_continue route=compact remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Compact);
                continue;
            }
            if let Some(response) = runtime_proxy_final_retryable_http_failure_response(
                last_failure,
                saw_inflight_saturation,
                true,
            ) {
                return Ok(response);
            }
            if compact_followup_profile.is_some() || session_profile.is_some() {
                return Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                ));
            }
            return match attempt_runtime_standard_request(
                request_id,
                request,
                shared,
                &compact_owner_profile,
                runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                    route_kind: RuntimeRouteKind::Compact,
                    candidate_name: &compact_owner_profile,
                    strict_affinity_profile: compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile: None,
                    turn_state_profile: None,
                    session_profile: session_profile.as_deref(),
                    trusted_previous_response_affinity: false,
                }),
            )? {
                RuntimeStandardAttempt::Success {
                    profile_name,
                    response,
                } => {
                    commit_runtime_proxy_profile_selection_with_notice(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                    )?;
                    Ok(response)
                }
                RuntimeStandardAttempt::RetryableFailure { response, .. } => Ok(response),
                RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
                    Ok(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    ))
                }
            };
        };

        if excluded_profiles.contains(&candidate_name) {
            continue;
        }

        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_candidate={} excluded_count={}",
                candidate_name,
                excluded_profiles.len()
            ),
        );
        let session_affinity_candidate = compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == &candidate_name)
            || session_profile.as_deref() == Some(candidate_name.as_str());
        if runtime_profile_inflight_hard_limited_for_context(
            shared,
            &candidate_name,
            "compact_http",
        )? && !session_affinity_candidate
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
            runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                route_kind: RuntimeRouteKind::Compact,
                candidate_name: &candidate_name,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: None,
                turn_state_profile: None,
                session_profile: session_profile.as_deref(),
                trusted_previous_response_affinity: false,
            }),
        )? {
            RuntimeStandardAttempt::Success {
                profile_name,
                response,
            } => {
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Compact,
                )?;
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_committed profile={profile_name}"
                    ),
                );
                return Ok(response);
            }
            RuntimeStandardAttempt::RetryableFailure {
                profile_name,
                response,
                overload,
            } => {
                let mut released_affinity = false;
                let mut released_compact_lineage = false;
                if !overload {
                    released_affinity = release_runtime_quota_blocked_affinity(
                        shared,
                        &profile_name,
                        None,
                        None,
                        request_session_id.as_deref(),
                    )?;
                    released_compact_lineage = release_runtime_compact_lineage(
                        shared,
                        &profile_name,
                        request_session_id.as_deref(),
                        None,
                        "quota_blocked",
                    )?;
                    if session_profile.as_deref() == Some(profile_name.as_str()) {
                        session_profile = None;
                    }
                    if compact_followup_profile
                        .as_ref()
                        .is_some_and(|(owner, _)| owner == &profile_name)
                    {
                        compact_followup_profile = None;
                    }
                }
                let should_retry_same_profile = overload
                    && !conservative_overload_retried_profiles.contains(&profile_name)
                    && (compact_followup_profile
                        .as_ref()
                        .is_some_and(|(owner, _)| owner == &profile_name)
                        || session_profile.as_deref() == Some(profile_name.as_str())
                        || current_profile == profile_name);
                if should_retry_same_profile {
                    conservative_overload_retried_profiles.insert(profile_name.clone());
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http compact_overload_conservative_retry profile={profile_name} delay_ms={RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS} reason=non_blocking_retry"
                        ),
                    );
                    last_failure = Some((response, false));
                    continue;
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_retryable_failure profile={profile_name}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !overload
                    && runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                        route_kind: RuntimeRouteKind::Compact,
                        candidate_name: &profile_name,
                        strict_affinity_profile: compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile: None,
                        turn_state_profile: None,
                        session_profile: session_profile.as_deref(),
                        trusted_previous_response_affinity: false,
                    })
                {
                    return Ok(response);
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=compact"
                        ),
                    );
                }
                if released_compact_lineage {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http compact_lineage_released profile={profile_name} reason=quota_blocked"
                        ),
                    );
                }
                if overload {
                    let _ = bump_runtime_profile_health_score(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                        RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                        "compact_overload",
                    );
                    let _ = bump_runtime_profile_bad_pairing_score(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                        "compact_overload",
                    );
                }
                if !overload
                    && !runtime_has_route_eligible_quota_fallback(
                        shared,
                        &profile_name,
                        &BTreeSet::new(),
                        RuntimeRouteKind::Compact,
                    )?
                {
                    return Ok(response);
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((response, !overload));
            }
            RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=compact reason=quota_exhausted_before_send"
                    ),
                );
                excluded_profiles.insert(profile_name);
            }
        }
    }
}

pub(crate) fn attempt_runtime_noncompact_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<RuntimeStandardAttempt> {
    attempt_runtime_noncompact_standard_request_with_policy(
        request_id,
        request,
        shared,
        profile_name,
        true,
    )
}

pub(crate) fn attempt_runtime_noncompact_standard_request_with_policy(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    enforce_local_precommit_quota_guard: bool,
) -> Result<RuntimeStandardAttempt> {
    let request_session_id = runtime_request_session_id(request);
    if enforce_local_precommit_quota_guard {
        let (quota_summary, quota_source) = runtime_profile_quota_summary_for_route(
            shared,
            profile_name,
            RuntimeRouteKind::Standard,
        )?;
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=standard quota_source={} {}",
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            return Ok(RuntimeStandardAttempt::LocalSelectionBlocked {
                profile_name: profile_name.to_string(),
            });
        }
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "standard_http")?;
    let mut recovery_steps = [
        RuntimeProfileUnauthorizedRecoveryStep::Reload,
        RuntimeProfileUnauthorizedRecoveryStep::Refresh,
    ]
    .into_iter();
    loop {
        let response =
            send_runtime_proxy_upstream_request(request_id, request, shared, profile_name, None)
                .inspect_err(|err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Standard,
                        "standard_upstream_request",
                        err,
                    );
                })?;
        if request.path_and_query.ends_with("/backend-api/wham/usage") {
            let status = response.status().as_u16();
            let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
                .inspect_err(|err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Standard,
                        "standard_buffer_usage_response",
                        err,
                    );
                })?;
            if status == 401
                && runtime_try_recover_profile_auth_from_unauthorized_steps(
                    request_id,
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    &mut recovery_steps,
                )
            {
                continue;
            }
            if let Ok(usage) = serde_json::from_slice::<UsageResponse>(&parts.body) {
                update_runtime_profile_probe_cache_with_usage(shared, profile_name, usage)?;
            }
            remember_runtime_session_id(
                shared,
                profile_name,
                request_session_id.as_deref(),
                RuntimeRouteKind::Standard,
            )?;
            if matches!(status, 401 | 403) {
                note_runtime_profile_auth_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    status,
                );
            }
            return Ok(RuntimeStandardAttempt::Success {
                profile_name: profile_name.to_string(),
                response: build_runtime_proxy_response_from_parts(parts),
            });
        }
        if response.status().is_success() {
            remember_runtime_session_id(
                shared,
                profile_name,
                request_session_id.as_deref(),
                RuntimeRouteKind::Standard,
            )?;
            let response = forward_runtime_proxy_response(shared, response, Vec::new())
                .inspect_err(|err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Standard,
                        "standard_forward_response",
                        err,
                    );
                })?;
            return Ok(RuntimeStandardAttempt::Success {
                profile_name: profile_name.to_string(),
                response,
            });
        }

        let status = response.status().as_u16();
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_buffer_response",
                    err,
                );
            })?;
        if status == 401
            && runtime_try_recover_profile_auth_from_unauthorized_steps(
                request_id,
                shared,
                profile_name,
                RuntimeRouteKind::Standard,
                &mut recovery_steps,
            )
        {
            continue;
        }
        let retryable_quota = matches!(status, 403 | 429)
            && extract_runtime_proxy_quota_message(&parts.body).is_some();
        if matches!(status, 403 | 429) && !retryable_quota {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http standard_quota_unclassified profile={profile_name} status={status} body_snippet={}",
                    runtime_proxy_body_snippet(&parts.body, 240),
                ),
            );
        }
        let response = build_runtime_proxy_response_from_parts(parts);

        if retryable_quota {
            return Ok(RuntimeStandardAttempt::RetryableFailure {
                profile_name: profile_name.to_string(),
                response,
                overload: false,
            });
        }

        if matches!(status, 401 | 403) {
            note_runtime_profile_auth_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Standard,
                status,
            );
        }

        remember_runtime_session_id(
            shared,
            profile_name,
            request_session_id.as_deref(),
            RuntimeRouteKind::Standard,
        )?;
        return Ok(RuntimeStandardAttempt::Success {
            profile_name: profile_name.to_string(),
            response,
        });
    }
}

pub(crate) fn attempt_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    allow_quota_exhausted_send: bool,
) -> Result<RuntimeStandardAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Compact)?;
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
        && !allow_quota_exhausted_send
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=compact quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeStandardAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
        });
    } else if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_pre_send_allow_quota_exhausted profile={profile_name} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "compact_http")?;
    let mut recovery_steps = [
        RuntimeProfileUnauthorizedRecoveryStep::Reload,
        RuntimeProfileUnauthorizedRecoveryStep::Refresh,
    ]
    .into_iter();
    loop {
        let response =
            send_runtime_proxy_upstream_request(request_id, request, shared, profile_name, None)
                .inspect_err(|err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Compact,
                        "compact_upstream_request",
                        err,
                    );
                })?;
        let compact_request = is_runtime_compact_path(&request.path_and_query);
        if !compact_request || response.status().is_success() {
            let response_turn_state = compact_request
                .then(|| runtime_proxy_header_value(response.headers(), "x-codex-turn-state"))
                .flatten();
            let response = if compact_request {
                forward_runtime_proxy_response_with_limit(
                    shared,
                    response,
                    Vec::new(),
                    RUNTIME_PROXY_COMPACT_BUFFERED_RESPONSE_MAX_BYTES,
                )
            } else {
                forward_runtime_proxy_response(shared, response, Vec::new())
            }
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_forward_response",
                    err,
                );
            })?;
            remember_runtime_session_id(
                shared,
                profile_name,
                request_session_id.as_deref(),
                if compact_request {
                    RuntimeRouteKind::Compact
                } else {
                    RuntimeRouteKind::Standard
                },
            )?;
            if compact_request {
                remember_runtime_compact_lineage(
                    shared,
                    profile_name,
                    request_session_id.as_deref(),
                    response_turn_state.as_deref(),
                    RuntimeRouteKind::Compact,
                )?;
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_committed_owner profile={profile_name} session={} turn_state={}",
                        request_session_id.as_deref().unwrap_or("-"),
                        response_turn_state.as_deref().unwrap_or("-"),
                    ),
                );
            }
            return Ok(RuntimeStandardAttempt::Success {
                profile_name: profile_name.to_string(),
                response,
            });
        }

        let status = response.status().as_u16();
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_buffer_response",
                    err,
                );
            })?;
        if status == 401
            && runtime_try_recover_profile_auth_from_unauthorized_steps(
                request_id,
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
                &mut recovery_steps,
            )
        {
            continue;
        }
        let retryable_quota = matches!(status, 403 | 429)
            && extract_runtime_proxy_quota_message(&parts.body).is_some();
        let retryable_overload =
            extract_runtime_proxy_overload_message(status, &parts.body).is_some();
        if matches!(status, 403 | 429) && !retryable_quota {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_quota_unclassified profile={profile_name} status={status} body_snippet={}",
                    runtime_proxy_body_snippet(&parts.body, 240),
                ),
            );
        }
        let response = build_runtime_proxy_response_from_parts(parts);

        if retryable_quota || retryable_overload {
            return Ok(RuntimeStandardAttempt::RetryableFailure {
                profile_name: profile_name.to_string(),
                response,
                overload: retryable_overload,
            });
        }

        if matches!(status, 401 | 403) {
            note_runtime_profile_auth_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
                status,
            );
        }

        return Ok(RuntimeStandardAttempt::Success {
            profile_name: profile_name.to_string(),
            response,
        });
    }
}
