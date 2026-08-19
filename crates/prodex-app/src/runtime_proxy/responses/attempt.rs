use super::*;

pub(crate) struct RuntimeResponsesContinuationTrace<'a> {
    pub(crate) request_id: u64,
    pub(crate) profile_name: &'a str,
    pub(crate) session_id: Option<&'a str>,
    pub(crate) thread_id: Option<&'a str>,
    pub(crate) turn_id: Option<&'a str>,
    pub(crate) turn_state: Option<&'a str>,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) returned_response_id: Option<&'a str>,
    pub(crate) rotation_generation: usize,
    pub(crate) retry_number: usize,
    pub(crate) compaction_generation: Option<u64>,
    pub(crate) stream_committed: bool,
}

pub(crate) fn log_runtime_responses_continuation_trace(
    shared: &RuntimeRotationProxyShared,
    trace: RuntimeResponsesContinuationTrace<'_>,
) {
    if !shared.runtime_config.response_chain_trace {
        return;
    }
    let RuntimeResponsesContinuationTrace {
        request_id,
        profile_name,
        session_id,
        thread_id,
        turn_id,
        turn_state,
        previous_response_id,
        returned_response_id,
        rotation_generation,
        retry_number,
        compaction_generation,
        stream_committed,
    } = trace;
    let (logical_provider, transport_provider_hash) =
        runtime_response_trace_provider_labels(shared, profile_name);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "responses_continuation",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field(
                    "profile_hash",
                    runtime_proxy_crate::runtime_proxy_identifier_hash(Some(profile_name)),
                ),
                runtime_proxy_log_field("logical_provider", logical_provider),
                runtime_proxy_log_field("transport_provider_hash", transport_provider_hash),
                runtime_proxy_log_field(
                    "session_id_hash",
                    runtime_proxy_crate::runtime_proxy_identifier_hash(session_id),
                ),
                runtime_proxy_log_field(
                    "thread_id_hash",
                    runtime_proxy_crate::runtime_proxy_identifier_hash(thread_id),
                ),
                runtime_proxy_log_field(
                    "turn_id_hash",
                    runtime_proxy_crate::runtime_proxy_identifier_hash(turn_id),
                ),
                runtime_proxy_log_field(
                    "turn_state_hash",
                    runtime_proxy_crate::runtime_proxy_identifier_hash(turn_state),
                ),
                runtime_proxy_log_field(
                    "previous_response_id_hash",
                    runtime_proxy_crate::runtime_proxy_identifier_hash(previous_response_id),
                ),
                runtime_proxy_log_field(
                    "response_id_hash",
                    runtime_proxy_crate::runtime_proxy_identifier_hash(returned_response_id),
                ),
                runtime_proxy_log_field("rotation_generation", rotation_generation.to_string()),
                runtime_proxy_log_field("transport_generation", "not_applicable"),
                runtime_proxy_log_field("retry_attempt", retry_number.to_string()),
                runtime_proxy_log_field(
                    "compaction_generation",
                    compaction_generation
                        .map(|generation| generation.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                ),
                runtime_proxy_log_field(
                    "full_context_request",
                    previous_response_id.is_none().to_string(),
                ),
                runtime_proxy_log_field(
                    "chain_reuse_reason",
                    if previous_response_id.is_some() {
                        "previous_response_present"
                    } else {
                        "full_context"
                    },
                ),
                runtime_proxy_log_field("stream_committed", stream_committed.to_string()),
            ],
        ),
    );
}

pub(crate) fn runtime_response_trace_provider_labels(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> (String, String) {
    let Ok(runtime) = shared.runtime.lock() else {
        return ("unknown".to_string(), "none".to_string());
    };
    let logical_provider = runtime
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.provider.label().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let transport_provider_hash = runtime_proxy_crate::runtime_proxy_identifier_hash(Some(
        runtime.upstream_base_url.as_str(),
    ));
    (logical_provider, transport_provider_hash)
}

pub(crate) struct RuntimeResponsesAttemptOptions<'a> {
    pub(crate) turn_state_override: Option<&'a str>,
    pub(crate) prompt_cache_key: Option<&'a str>,
    pub(crate) hard_affinity: bool,
    pub(crate) selection_attempt: usize,
}

pub(crate) fn attempt_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    options: RuntimeResponsesAttemptOptions<'_>,
) -> Result<RuntimeResponsesAttempt> {
    let RuntimeResponsesAttemptOptions {
        turn_state_override,
        prompt_cache_key,
        hard_affinity,
        selection_attempt,
    } = options;
    let request_session_id = runtime_request_session_id(request);
    let request_previous_response_id = runtime_request_previous_response_id(request);
    let request_prompt_cache_key = prompt_cache_key
        .map(str::to_string)
        .or_else(|| runtime_request_prompt_cache_key(request));
    let request_turn_state = runtime_request_turn_state(request);
    let request_turn_id = runtime_proxy_crate::runtime_request_turn_id(request);
    let request_thread_id = runtime_proxy_crate::runtime_request_thread_id(request);
    let request_compaction_generation =
        runtime_proxy_crate::runtime_request_compaction_generation(request);
    let codex_previous_response_id_regression =
        runtime_codex_previous_response_id_regression(request);
    let mut request_for_attempt = request.clone();
    let mut previous_response_id_for_attempt = request_previous_response_id.clone();
    let mut full_history_fallback_used = false;
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
    let Some(inflight_guard) = try_acquire_runtime_profile_inflight_guard(
        shared,
        profile_name,
        "responses_http",
        hard_affinity,
    )?
    else {
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "profile_inflight_saturated",
        });
    };

    let mut inflight_guard = Some(inflight_guard);
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    let mut retry_number = 0usize;
    loop {
        log_runtime_responses_continuation_trace(
            shared,
            RuntimeResponsesContinuationTrace {
                request_id,
                profile_name,
                session_id: request_session_id.as_deref(),
                thread_id: request_thread_id.as_deref(),
                turn_id: request_turn_id.as_deref(),
                turn_state: request_turn_state.as_deref(),
                previous_response_id: previous_response_id_for_attempt.as_deref(),
                returned_response_id: None,
                rotation_generation: selection_attempt,
                retry_number,
                compaction_generation: request_compaction_generation,
                stream_committed: false,
            },
        );
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
        let upstream_request = request_for_attempt.clone();
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
            let exact_invalid_previous_response_id =
                runtime_proxy_crate::runtime_proxy_body_is_invalid_previous_response_id(
                    &parts.body,
                );
            if status == 400
                && exact_invalid_previous_response_id
                && codex_previous_response_id_regression
                && !full_history_fallback_used
                && let Some(previous_response_id) = previous_response_id_for_attempt.as_deref()
                && runtime_response_bound_profile(
                    shared,
                    previous_response_id,
                    RuntimeRouteKind::Responses,
                )?
                .as_deref()
                    == Some(profile_name)
                && let Some(fallback_request) =
                    runtime_proxy_crate::runtime_request_full_history_without_previous_response_id(
                        &request_for_attempt,
                    )
            {
                clear_runtime_dead_response_bindings(
                    shared,
                    profile_name,
                    &[previous_response_id.to_string()],
                    "invalid_previous_response_id",
                )?;
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "responses_full_history_recovery",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field(
                                "profile_hash",
                                runtime_proxy_crate::runtime_proxy_identifier_hash(Some(
                                    profile_name,
                                )),
                            ),
                            runtime_proxy_log_field(
                                "previous_response_id_hash",
                                runtime_proxy_crate::runtime_proxy_identifier_hash(Some(
                                    previous_response_id,
                                )),
                            ),
                            runtime_proxy_log_field("attempt", "one"),
                            runtime_proxy_log_field("full_context_request", "true"),
                        ],
                    ),
                );
                request_for_attempt = fallback_request;
                previous_response_id_for_attempt = None;
                full_history_fallback_used = true;
                retry_number = retry_number.saturating_add(1);
                continue;
            }
            let Some(attempt) = handle_runtime_responses_non_success(
                request_id,
                shared,
                profile_name,
                status,
                parts,
                response_turn_state,
                &mut recovery_steps,
            )?
            else {
                retry_number = retry_number.saturating_add(1);
                continue;
            };
            return Ok(attempt);
        }
        let mut prepared = prepare_runtime_responses_success_attempt(
            RuntimeResponsesSuccessAttemptContext {
                request_id,
                shared: shared.clone(),
                profile_name: profile_name.to_string(),
                turn_state_override: turn_state_override.map(str::to_string),
                request_previous_response_id: previous_response_id_for_attempt.clone(),
                request_prompt_cache_key: request_prompt_cache_key.clone(),
                request_session_id: request_session_id.clone(),
                request_thread_id: request_thread_id.clone(),
                request_turn_id: request_turn_id.clone(),
                request_turn_state: request_turn_state.clone(),
                request_model_name: runtime_smart_context_model_name_from_body(&request.body),
                selection_attempt,
                retry_number,
                request_compaction_generation,
                inflight_guard: inflight_guard.take(),
            },
            response,
        );
        let exact_sse_invalid_previous_response_id = matches!(
            prepared.as_ref(),
            Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                invalid_previous_response_id: true,
                ..
            })
        );
        if exact_sse_invalid_previous_response_id
            && codex_previous_response_id_regression
            && !full_history_fallback_used
            && let Some(previous_response_id) = previous_response_id_for_attempt.as_deref()
            && runtime_response_bound_profile(
                shared,
                previous_response_id,
                RuntimeRouteKind::Responses,
            )?
            .as_deref()
                == Some(profile_name)
            && let Some(fallback_request) =
                runtime_proxy_crate::runtime_request_full_history_without_previous_response_id(
                    &request_for_attempt,
                )
        {
            let recovery_guard = match &mut prepared {
                Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                    response: RuntimeResponsesReply::Streaming(response),
                    ..
                }) => response._inflight_guard.take(),
                _ => None,
            };
            let Some(recovery_guard) = recovery_guard else {
                return Err(anyhow::anyhow!(
                    "responses SSE recovery lost its profile in-flight guard"
                ));
            };
            clear_runtime_dead_response_bindings(
                shared,
                profile_name,
                &[previous_response_id.to_string()],
                "invalid_previous_response_id",
            )?;
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "responses_full_history_recovery",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http_sse"),
                        runtime_proxy_log_field(
                            "profile_hash",
                            runtime_proxy_crate::runtime_proxy_identifier_hash(Some(profile_name)),
                        ),
                        runtime_proxy_log_field(
                            "previous_response_id_hash",
                            runtime_proxy_crate::runtime_proxy_identifier_hash(Some(
                                previous_response_id,
                            )),
                        ),
                        runtime_proxy_log_field("attempt", "one"),
                        runtime_proxy_log_field("full_context_request", "true"),
                    ],
                ),
            );
            drop(prepared);
            inflight_guard = Some(recovery_guard);
            request_for_attempt = fallback_request;
            previous_response_id_for_attempt = None;
            full_history_fallback_used = true;
            retry_number = retry_number.saturating_add(1);
            continue;
        }
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

fn handle_runtime_responses_non_success(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    status: u16,
    parts: RuntimeHeapTrimmedBufferedResponseParts,
    response_turn_state: Option<String>,
    recovery_steps: &mut RuntimeProfileUnauthorizedRecoverySteps,
) -> Result<Option<RuntimeResponsesAttempt>> {
    if status == 401
        && runtime_try_recover_profile_auth_from_unauthorized_steps(
            request_id,
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            recovery_steps,
        )
    {
        return Ok(None);
    }
    let error_policy = runtime_proxy_crate::runtime_http_error_policy(
        status,
        &parts.body,
        runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit,
    );
    let token_invalidated = runtime_proxy_body_indicates_token_invalidated(&parts.body);
    let invalid_previous_response_id =
        runtime_proxy_crate::runtime_proxy_body_is_invalid_previous_response_id(&parts.body);
    let retryable_previous =
        status == 400 && extract_runtime_proxy_previous_response_message(&parts.body).is_some();
    let response = RuntimeResponsesReply::Buffered(parts);
    if status == 401 {
        note_runtime_profile_auth_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            status,
        );
        return Ok(Some(RuntimeResponsesAttempt::AuthFailed {
            profile_name: profile_name.to_string(),
            response,
        }));
    }
    if error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile {
        return Ok(Some(RuntimeResponsesAttempt::QuotaBlocked {
            profile_name: profile_name.to_string(),
            response,
        }));
    }
    if error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RetryProfile {
        return Ok(Some(RuntimeResponsesAttempt::Overloaded {
            profile_name: profile_name.to_string(),
            response,
        }));
    }
    if retryable_previous {
        return Ok(Some(RuntimeResponsesAttempt::PreviousResponseNotFound {
            profile_name: profile_name.to_string(),
            response,
            turn_state: response_turn_state,
            invalid_previous_response_id,
        }));
    }
    if matches!(status, 401 | 403) || token_invalidated {
        note_runtime_profile_auth_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            status,
        );
    }
    Ok(Some(RuntimeResponsesAttempt::Success {
        profile_name: profile_name.to_string(),
        response,
    }))
}

struct RuntimeResponsesSuccessAttemptContext {
    request_id: u64,
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    turn_state_override: Option<String>,
    request_previous_response_id: Option<String>,
    request_prompt_cache_key: Option<String>,
    request_session_id: Option<String>,
    request_thread_id: Option<String>,
    request_turn_id: Option<String>,
    request_turn_state: Option<String>,
    request_model_name: Option<String>,
    selection_attempt: usize,
    retry_number: usize,
    request_compaction_generation: Option<u64>,
    inflight_guard: Option<RuntimeProfileInFlightGuard>,
}

fn prepare_runtime_responses_success_attempt(
    context: RuntimeResponsesSuccessAttemptContext,
    response: reqwest::Response,
) -> Result<RuntimeResponsesAttempt> {
    let RuntimeResponsesSuccessAttemptContext {
        request_id,
        shared,
        profile_name,
        turn_state_override,
        request_previous_response_id,
        request_prompt_cache_key,
        request_session_id,
        request_thread_id,
        request_turn_id,
        request_turn_state,
        request_model_name,
        selection_attempt,
        retry_number,
        request_compaction_generation,
        inflight_guard,
    } = context;
    let Some(inflight_guard) = inflight_guard else {
        runtime_proxy_log(
            &shared,
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
    let success_profile_name = profile_name.clone();
    await_runtime_proxy_async_task(&shared, "responses_prepare_success", async move {
        prepare_runtime_proxy_responses_success(
            RuntimeResponsesSuccessContext {
                request_id,
                request_model_name: request_model_name.as_deref(),
                request_previous_response_id: request_previous_response_id.as_deref(),
                request_prompt_cache_key: request_prompt_cache_key.as_deref(),
                request_session_id: request_session_id.as_deref(),
                request_thread_id: request_thread_id.as_deref(),
                request_turn_id: request_turn_id.as_deref(),
                request_turn_state: request_turn_state.as_deref(),
                turn_state_override: turn_state_override.as_deref(),
                selection_attempt,
                retry_number,
                request_compaction_generation,
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
            &shared,
            &profile_name,
            RuntimeRouteKind::Responses,
            "responses_prepare_success",
            err,
        );
    })
}
