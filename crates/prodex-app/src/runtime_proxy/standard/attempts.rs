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
            if runtime_auto_redeem_usage_limit_reset_credit(
                shared,
                profile_name,
                RuntimeRouteKind::Standard,
                "standard_precommit",
                request_session_id.is_none(),
            )? == RuntimeAutoRedeemResetCreditOutcome::Redeemed
            {
                let (redeemed_summary, _) = runtime_profile_quota_summary_for_route(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                )?;
                if redeemed_summary.route_band != RuntimeQuotaPressureBand::Exhausted {
                    return attempt_runtime_noncompact_standard_request_with_policy(
                        request_id,
                        request,
                        shared,
                        profile_name,
                        false,
                    );
                }
            }
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
        if request.path_and_query.ends_with("/backend-api/wham/usage") {
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
            let retryable_quota = error_policy.action
                == runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile
                && matches!(
                    error_policy.class,
                    runtime_proxy_crate::RuntimeHttpErrorClass::Quota
                        | runtime_proxy_crate::RuntimeHttpErrorClass::ProfileUnavailable
                );
            if retryable_quota {
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
                    overload: false,
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
        let retryable_quota = runtime_proxy_precommit_error_rotates_profile(status, &parts.body);
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

        if retryable_quota {
            return Ok(RuntimeStandardAttempt::RetryableFailure {
                profile_name: profile_name.to_string(),
                response,
                overload: false,
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

fn forward_runtime_standard_success_response(
    shared: &RuntimeRotationProxyShared,
    request: &RuntimeProxyRequest,
    response: reqwest::Response,
) -> Result<tiny_http::ResponseBox> {
    if !runtime_openai_models_metadata_path(&request.path_and_query) {
        return await_runtime_proxy_async_task(
            shared,
            "standard_forward_response",
            forward_runtime_proxy_response(response, Vec::new()),
        );
    }

    let parts = await_runtime_proxy_async_task(
        shared,
        "standard_buffer_models_response",
        buffer_runtime_proxy_async_response_parts(response, Vec::new()),
    )?;
    Ok(build_runtime_proxy_response_from_parts(
        runtime_patch_openai_spark_models_response(parts),
    ))
}

fn runtime_openai_models_metadata_path(path_and_query: &str) -> bool {
    let normalized = runtime_proxy_normalize_openai_path(path_and_query);
    let path = path_without_query(normalized.as_ref()).trim_end_matches('/');
    path == "/models" || path == "/v1/models" || path.ends_with("/codex/models")
}

fn runtime_patch_openai_spark_models_response(
    mut parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&parts.body) else {
        return parts;
    };
    let Some(models) = value
        .get_mut("models")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return parts;
    };

    if let Some(model) = models
        .iter_mut()
        .find(|model| runtime_model_catalog_entry_matches_slug(model, "gpt-5.3-codex-spark"))
    {
        runtime_patch_openai_spark_model_entry(model);
    } else if let Some(mut model) = ["gpt-5.3-codex", "gpt-5.4", "gpt-5.5"]
        .iter()
        .find_map(|slug| {
            models
                .iter()
                .find(|model| runtime_model_catalog_entry_matches_slug(model, slug))
                .cloned()
        })
        .or_else(|| models.first().cloned())
    {
        runtime_patch_openai_spark_model_entry(&mut model);
        models.push(model);
    }

    if let Ok(body) = serde_json::to_vec(&value) {
        parts.body = body.into();
    }
    parts
}

fn runtime_model_catalog_entry_matches_slug(model: &serde_json::Value, slug: &str) -> bool {
    model
        .get("slug")
        .or_else(|| model.get("id"))
        .or_else(|| model.get("model"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case(slug))
}

fn runtime_patch_openai_spark_model_entry(model: &mut serde_json::Value) {
    let Some(object) = model.as_object_mut() else {
        return;
    };
    object.insert(
        "slug".to_string(),
        serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
    );
    if object.contains_key("id") {
        object.insert(
            "id".to_string(),
            serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
        );
    }
    if object.contains_key("model") {
        object.insert(
            "model".to_string(),
            serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
        );
    }
    object.insert(
        "display_name".to_string(),
        serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
    );
    object.insert("context_window".to_string(), serde_json::json!(128_000));
    object.insert("max_context_window".to_string(), serde_json::json!(128_000));
    object.insert(
        "auto_compact_token_limit".to_string(),
        serde_json::json!(115_200),
    );
    object.insert(
        "effective_context_window_percent".to_string(),
        serde_json::json!(95),
    );
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
        if runtime_auto_redeem_usage_limit_reset_credit(
            shared,
            profile_name,
            RuntimeRouteKind::Compact,
            "compact_precommit",
            request_session_id.is_none(),
        )? == RuntimeAutoRedeemResetCreditOutcome::Redeemed
        {
            let (redeemed_summary, _) = runtime_profile_quota_summary_for_route(
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
            )?;
            if redeemed_summary.route_band != RuntimeQuotaPressureBand::Exhausted {
                return attempt_runtime_standard_request(
                    request_id,
                    request,
                    shared,
                    profile_name,
                    allow_quota_exhausted_send,
                );
            }
        }
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
            let response_turn_state = compact_request
                .then(|| runtime_proxy_header_value(response.headers(), "x-codex-turn-state"))
                .flatten();
            let response_result = if compact_request {
                await_runtime_proxy_async_task(
                    shared,
                    "compact_forward_response",
                    forward_runtime_proxy_response_with_limit(
                        response,
                        Vec::new(),
                        RUNTIME_PROXY_COMPACT_BUFFERED_RESPONSE_MAX_BYTES,
                    ),
                )
            } else {
                await_runtime_proxy_async_task(
                    shared,
                    "compact_forward_response",
                    forward_runtime_proxy_response(response, Vec::new()),
                )
            };
            let response = match response_result {
                Ok(response) => response,
                Err(err) => {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Compact,
                        "compact_forward_response",
                        &err,
                    );
                    if is_runtime_proxy_transport_failure(&err) {
                        return Ok(RuntimeStandardAttempt::TransportFailed {
                            profile_name: profile_name.to_string(),
                            stage: "compact_forward_response",
                        });
                    }
                    return Err(err);
                }
            };
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response_parts(body: serde_json::Value) -> RuntimeHeapTrimmedBufferedResponseParts {
        RuntimeHeapTrimmedBufferedResponseParts {
            status: 200,
            headers: Vec::new(),
            body: serde_json::to_vec(&body).unwrap().into(),
        }
    }

    #[test]
    fn openai_models_response_adds_spark_context_from_codex_metadata() {
        let parts = runtime_patch_openai_spark_models_response(response_parts(json!({
            "models": [{
                "slug": "gpt-5.3-codex",
                "display_name": "gpt-5.3-codex",
                "context_window": 272000,
                "max_context_window": 272000,
                "auto_compact_token_limit": null,
                "effective_context_window_percent": 95
            }]
        })));
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let spark = value["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["slug"] == "gpt-5.3-codex-spark")
            .unwrap();

        assert_eq!(spark["context_window"], 128_000);
        assert_eq!(spark["max_context_window"], 128_000);
        assert_eq!(spark["auto_compact_token_limit"], 115_200);
    }

    #[test]
    fn openai_models_response_adds_spark_when_codex_metadata_is_absent() {
        let parts = runtime_patch_openai_spark_models_response(response_parts(json!({
            "models": [{
                "slug": "gpt-5.4",
                "display_name": "gpt-5.4",
                "context_window": 272000,
                "max_context_window": 1000000,
                "auto_compact_token_limit": null,
                "effective_context_window_percent": 95
            }]
        })));
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let spark = value["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["slug"] == "gpt-5.3-codex-spark")
            .unwrap();

        assert_eq!(spark["display_name"], "gpt-5.3-codex-spark");
        assert_eq!(spark["context_window"], 128_000);
        assert_eq!(spark["max_context_window"], 128_000);
        assert_eq!(spark["auto_compact_token_limit"], 115_200);
    }

    #[test]
    fn openai_models_response_corrects_existing_spark_context() {
        let parts = runtime_patch_openai_spark_models_response(response_parts(json!({
            "models": [{
                "slug": "gpt-5.3-codex-spark",
                "display_name": "spark",
                "context_window": 272000,
                "max_context_window": 272000,
                "auto_compact_token_limit": 258400,
                "effective_context_window_percent": 95
            }]
        })));
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let spark = &value["models"][0];

        assert_eq!(spark["display_name"], "gpt-5.3-codex-spark");
        assert_eq!(spark["context_window"], 128_000);
        assert_eq!(spark["auto_compact_token_limit"], 115_200);
    }

    #[test]
    fn openai_models_metadata_path_matches_codex_and_openai_routes() {
        assert!(runtime_openai_models_metadata_path(
            "/backend-api/codex/models?client_version=0.124.0"
        ));
        assert!(runtime_openai_models_metadata_path(
            "/backend-api/prodex/models?client_version=0.142.5"
        ));
        assert!(runtime_openai_models_metadata_path(
            "/v1/models?client_version=0.124.0"
        ));
        assert!(!runtime_openai_models_metadata_path("/v1/responses"));
    }
}
