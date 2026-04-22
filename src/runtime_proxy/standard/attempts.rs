use super::*;

pub(super) fn attempt_runtime_noncompact_standard_request(
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

pub(super) fn attempt_runtime_noncompact_standard_request_with_policy(
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
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
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
        let previous_response_not_found =
            extract_runtime_proxy_previous_response_message(&parts.body).is_some();
        let response = build_runtime_proxy_response_from_parts(
            runtime_proxy_translate_previous_response_http_parts(parts),
        );

        if previous_response_not_found {
            runtime_proxy_record_continuity_failure_reason(
                shared,
                "stale_continuation",
                "previous_response_not_found",
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http stale_continuation reason=previous_response_not_found route=standard profile={profile_name}"
                ),
            );
            return Ok(RuntimeStandardAttempt::StaleContinuation { response });
        }

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

pub(super) fn attempt_runtime_standard_request(
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
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
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
        let previous_response_not_found =
            extract_runtime_proxy_previous_response_message(&parts.body).is_some();
        let response = build_runtime_proxy_response_from_parts(
            runtime_proxy_translate_previous_response_http_parts(parts),
        );

        if previous_response_not_found {
            runtime_proxy_record_continuity_failure_reason(
                shared,
                "stale_continuation",
                "previous_response_not_found",
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http stale_continuation reason=previous_response_not_found route={} profile={profile_name}",
                    if compact_request {
                        "compact"
                    } else {
                        "standard"
                    }
                ),
            );
            return Ok(RuntimeStandardAttempt::StaleContinuation { response });
        }

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
