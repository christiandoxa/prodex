//! Standard-route non-compact upstream attempt orchestration.

use super::*;

pub(in crate::runtime_proxy::standard) fn attempt_runtime_noncompact_standard_request(
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

pub(in crate::runtime_proxy::standard) fn attempt_runtime_noncompact_standard_request_with_policy(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    enforce_local_precommit_quota_guard: bool,
) -> Result<RuntimeStandardAttempt> {
    let request_session_id = runtime_request_session_id(request);
    if enforce_local_precommit_quota_guard {
        match runtime_standard_precommit_quota_guard(
            request_id,
            shared,
            profile_name,
            request_session_id.is_none(),
        )? {
            RuntimeStandardPrecommitGuard::Continue => {}
            RuntimeStandardPrecommitGuard::RetryWithoutGuard => {
                return attempt_runtime_noncompact_standard_request_with_policy(
                    request_id,
                    request,
                    shared,
                    profile_name,
                    false,
                );
            }
            RuntimeStandardPrecommitGuard::Blocked(attempt) => return Ok(attempt),
        }
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "standard_http")?;
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    loop {
        let upstream_auth =
            runtime_profile_usage_auth(shared, profile_name).inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_auth_lookup",
                    err,
                );
            })?;
        let upstream_request = request.clone();
        let upstream_shared = shared.clone();
        let upstream_profile_name = profile_name.to_string();
        let response =
            await_runtime_proxy_async_task(shared, "standard_upstream_request", async move {
                send_runtime_proxy_upstream_request(
                    request_id,
                    &upstream_request,
                    &upstream_shared,
                    &upstream_profile_name,
                    None,
                    upstream_auth,
                )
                .await
            })
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_upstream_request",
                    err,
                );
            })?;
        if runtime_wham_usage_path(&request.path_and_query) {
            let status = response.status().as_u16();
            let parts = await_runtime_proxy_async_task(
                shared,
                "standard_buffer_usage_response",
                buffer_runtime_proxy_async_response_parts(response, Vec::new()),
            )
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
            if status == 401 {
                note_runtime_profile_auth_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    status,
                );
                return Ok(RuntimeStandardAttempt::AuthFailed {
                    profile_name: profile_name.to_string(),
                    response: build_runtime_proxy_response_from_parts(parts),
                });
            }
            let error_policy = runtime_proxy_crate::runtime_http_error_policy(
                status,
                &parts.body,
                runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit,
            );
            if error_policy.may_retry_or_rotate() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http standard_usage_retryable_failure profile={profile_name} status={status} rule={} message={}",
                        error_policy.rule.unwrap_or("-"),
                        error_policy.message.as_deref().unwrap_or("-"),
                    ),
                );
                return Ok(RuntimeStandardAttempt::RetryableFailure {
                    profile_name: profile_name.to_string(),
                    response: build_runtime_proxy_response_from_parts(parts),
                    overload: error_policy.action
                        == runtime_proxy_crate::RuntimeHttpErrorAction::RetryProfile,
                });
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
            if matches!(status, 401 | 403)
                || runtime_proxy_body_indicates_token_invalidated(&parts.body)
            {
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
            let response = forward_runtime_standard_success_response(shared, request, response)
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
        let parts = await_runtime_proxy_async_task(
            shared,
            "standard_buffer_response",
            buffer_runtime_proxy_async_response_parts(response, Vec::new()),
        )
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
        let error_policy = runtime_proxy_crate::runtime_http_error_policy(
            status,
            &parts.body,
            runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit,
        );
        let retryable_quota =
            error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile;
        let retryable_overload =
            error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RetryProfile;
        let token_invalidated = runtime_proxy_body_indicates_token_invalidated(&parts.body);
        if matches!(status, 402 | 403 | 429) && !retryable_quota {
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

        if status == 401 {
            note_runtime_profile_auth_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Standard,
                status,
            );
            return Ok(RuntimeStandardAttempt::AuthFailed {
                profile_name: profile_name.to_string(),
                response,
            });
        }

        if retryable_quota || retryable_overload {
            return Ok(RuntimeStandardAttempt::RetryableFailure {
                profile_name: profile_name.to_string(),
                response,
                overload: retryable_overload,
            });
        }

        if matches!(status, 401 | 403) || token_invalidated {
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

fn runtime_wham_usage_path(path_and_query: &str) -> bool {
    let normalized = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized.as_ref())
        .trim_end_matches('/')
        .ends_with("/wham/usage")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wham_usage_path_matches_codex_prodex_mounts_and_query() {
        assert!(runtime_wham_usage_path("/backend-api/wham/usage"));
        assert!(runtime_wham_usage_path(
            "/backend-api/wham/usage?client_version=0.142.5"
        ));
        assert!(runtime_wham_usage_path(
            "/backend-api/prodex/wham/usage?client_version=0.142.5"
        ));
        assert!(!runtime_wham_usage_path("/backend-api/codex/responses"));
    }
}
