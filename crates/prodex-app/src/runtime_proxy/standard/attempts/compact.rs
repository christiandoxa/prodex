//! Compact-route upstream attempt orchestration.

use super::*;

pub(in crate::runtime_proxy::standard) fn attempt_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    allow_quota_exhausted_send: bool,
) -> Result<RuntimeStandardAttempt> {
    let request_session_id = runtime_request_session_id(request);
    match runtime_compact_precommit_quota_guard(
        request_id,
        shared,
        profile_name,
        allow_quota_exhausted_send,
        request_session_id.is_none(),
    )? {
        RuntimeStandardPrecommitGuard::Continue => {}
        RuntimeStandardPrecommitGuard::RetryWithoutGuard => {
            return attempt_runtime_standard_request(
                request_id,
                request,
                shared,
                profile_name,
                allow_quota_exhausted_send,
            );
        }
        RuntimeStandardPrecommitGuard::Blocked(attempt) => return Ok(attempt),
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "compact_http")?;
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    loop {
        let upstream_auth =
            runtime_profile_usage_auth(shared, profile_name).inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_auth_lookup",
                    err,
                );
            })?;
        let upstream_request = request.clone();
        let upstream_shared = shared.clone();
        let upstream_profile_name = profile_name.to_string();
        let response =
            match await_runtime_proxy_async_task(shared, "compact_upstream_request", async move {
                send_runtime_proxy_upstream_request(
                    request_id,
                    &upstream_request,
                    &upstream_shared,
                    &upstream_profile_name,
                    None,
                    upstream_auth,
                )
                .await
            }) {
                Ok(response) => response,
                Err(err) => {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Compact,
                        "compact_upstream_request",
                        &err,
                    );
                    if is_runtime_proxy_transport_failure(&err) {
                        return Ok(RuntimeStandardAttempt::TransportFailed {
                            profile_name: profile_name.to_string(),
                            stage: "compact_upstream_request",
                        });
                    }
                    return Err(err);
                }
            };
        let compact_request = is_runtime_compact_path(&request.path_and_query);
        if !compact_request || response.status().is_success() {
            return forward_runtime_compact_success_attempt(
                request_id,
                shared,
                profile_name,
                request_session_id.as_deref(),
                response,
                compact_request,
            );
        }

        let status = response.status().as_u16();
        let parts = match await_runtime_proxy_async_task(
            shared,
            "compact_buffer_response",
            buffer_runtime_proxy_async_response_parts(response, Vec::new()),
        ) {
            Ok(parts) => parts,
            Err(err) => {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_buffer_response",
                    &err,
                );
                if is_runtime_proxy_transport_failure(&err) {
                    return Ok(RuntimeStandardAttempt::TransportFailed {
                        profile_name: profile_name.to_string(),
                        stage: "compact_buffer_response",
                    });
                }
                return Err(err);
            }
        };
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
        let retryable_quota = runtime_proxy_precommit_error_rotates_profile(status, &parts.body);
        let token_invalidated = runtime_proxy_body_indicates_token_invalidated(&parts.body);
        let retryable_overload =
            extract_runtime_proxy_overload_message(status, &parts.body).is_some();
        if matches!(status, 402 | 403 | 429) && !retryable_quota {
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

        if status == 401 {
            note_runtime_profile_auth_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
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
