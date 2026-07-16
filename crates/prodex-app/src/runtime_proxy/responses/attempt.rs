use super::*;

pub(crate) fn attempt_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
    prompt_cache_key: Option<&str>,
) -> Result<RuntimeResponsesAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let request_previous_response_id = runtime_request_previous_response_id(request);
    let request_prompt_cache_key = prompt_cache_key
        .map(str::to_string)
        .or_else(|| runtime_request_prompt_cache_key(request));
    let request_turn_state = runtime_request_turn_state(request);
    let quota_gate = runtime_precommit_quota_gate(RuntimePrecommitQuotaGateRequest {
        shared,
        profile_name,
        route_kind: RuntimeRouteKind::Responses,
        has_continuation_context: request_previous_response_id.is_some()
            || request_session_id.is_some()
            || request_turn_state.is_some(),
        reprobe_context: "responses_precommit_reprobe",
    })?;
    if let RuntimePrecommitQuotaGateDecision::Block {
        reason,
        summary,
        source,
    } = quota_gate
    {
        let reason_label = reason.as_str();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason_label} quota_source={} {}",
                source.map(runtime_quota_source_label).unwrap_or("unknown"),
                runtime_quota_summary_log_fields(summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: reason_label,
        });
    }
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "responses_http")?;

    let mut inflight_guard = Some(inflight_guard);
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    loop {
        let upstream_auth =
            runtime_profile_usage_auth(shared, profile_name).inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    "responses_auth_lookup",
                    err,
                );
            })?;
        let upstream_request = request.clone();
        let upstream_shared = shared.clone();
        let upstream_profile_name = profile_name.to_string();
        let upstream_turn_state_override = turn_state_override.map(str::to_string);
        let response =
            await_runtime_proxy_async_task(shared, "responses_upstream_request", async move {
                send_runtime_proxy_upstream_responses_request(
                    request_id,
                    &upstream_request,
                    &upstream_shared,
                    &upstream_profile_name,
                    upstream_turn_state_override.as_deref(),
                    upstream_auth,
                )
                .await
            })
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
            let parts = await_runtime_proxy_async_task(
                shared,
                "responses_buffer_response",
                buffer_runtime_proxy_async_response_parts(response, Vec::new()),
            )
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
            let error_policy = runtime_proxy_crate::runtime_http_error_policy(
                status,
                &parts.body,
                runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit,
            );
            let token_invalidated = runtime_proxy_body_indicates_token_invalidated(&parts.body);
            let retryable_previous = status == 400
                && extract_runtime_proxy_previous_response_message(&parts.body).is_some();
            let response = RuntimeResponsesReply::Buffered(parts);

            if status == 401 {
                note_runtime_profile_auth_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    status,
                );
                return Ok(RuntimeResponsesAttempt::AuthFailed {
                    profile_name: profile_name.to_string(),
                    response,
                });
            }
            if error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile {
                return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                    profile_name: profile_name.to_string(),
                    response,
                });
            }
            if error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RetryProfile {
                return Ok(RuntimeResponsesAttempt::Overloaded {
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
            if matches!(status, 401 | 403) || token_invalidated {
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
        let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
        let Some(inflight_guard) = inflight_guard.take() else {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "responses_inflight_guard_missing",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", profile_name),
                    ],
                ),
            );
            return Err(anyhow::anyhow!(
                "responses inflight guard missing before success forwarding"
            ));
        };
        let success_shared = shared.clone();
        let success_profile_name = profile_name.to_string();
        let success_request_model_name = request_model_name;
        let success_previous_response_id = request_previous_response_id.clone();
        let success_prompt_cache_key = request_prompt_cache_key.clone();
        let success_session_id = request_session_id.clone();
        let success_turn_state = request_turn_state.clone();
        let success_turn_state_override = turn_state_override.map(str::to_string);
        let prepared =
            await_runtime_proxy_async_task(shared, "responses_prepare_success", async move {
                prepare_runtime_proxy_responses_success(
                    RuntimeResponsesSuccessContext {
                        request_id,
                        request_model_name: success_request_model_name.as_deref(),
                        request_previous_response_id: success_previous_response_id.as_deref(),
                        request_prompt_cache_key: success_prompt_cache_key.as_deref(),
                        request_session_id: success_session_id.as_deref(),
                        request_turn_state: success_turn_state.as_deref(),
                        turn_state_override: success_turn_state_override.as_deref(),
                        shared: &success_shared,
                        profile_name: &success_profile_name,
                        inflight_guard,
                    },
                    response,
                )
                .await
            })
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    "responses_prepare_success",
                    err,
                );
            });
        if let Ok(RuntimeResponsesAttempt::Success { profile_name, .. }) = &prepared {
            remember_runtime_prompt_cache_profile(
                shared,
                profile_name,
                request_prompt_cache_key.as_deref(),
                RuntimeRouteKind::Responses,
            );
        }
        return prepared;
    }
}
