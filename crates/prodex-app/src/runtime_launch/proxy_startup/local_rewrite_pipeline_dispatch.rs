use super::super::local_rewrite_gemini_compact::runtime_gemini_local_compact_response_parts;
use super::{
    RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE, RuntimeLocalRewriteDispatchReadyRequest,
    RuntimeLocalRewritePipelineExit, RuntimeLocalRewritePipelineResult,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    RuntimeProviderBridgeKind, RuntimeProxyRequest, build_runtime_proxy_json_error_response,
    build_runtime_proxy_text_response, path_without_query, respond_runtime_gemini_compact_request,
    respond_runtime_local_rewrite_proxy_request, runtime_copilot_model_catalog_from_provider,
    runtime_gateway_application_provider_dispatch,
    runtime_gateway_application_provider_dispatch_attempt,
    runtime_gateway_application_provider_retry_precommit, runtime_kiro_compact_response_parts,
    runtime_kiro_model_catalog_from_provider, runtime_kiro_models_buffered_response,
    runtime_local_rewrite_response_with_call_id, runtime_provider_error_class,
    runtime_provider_models_buffered_response, runtime_provider_request_ledger_message,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
    send_runtime_local_rewrite_upstream_request,
};
use crate::runtime_proxy::{
    RuntimeHeapTrimmedBufferedResponseParts, build_runtime_proxy_response_from_parts,
    runtime_proxy_local_overload_pressure_active,
};
use prodex_provider_core::ProviderErrorClass;
use prodex_provider_spi::ProviderRetryCause;
use std::sync::atomic::Ordering;

pub(super) fn runtime_local_rewrite_dispatch_compact<'target>(
    request: RuntimeLocalRewriteDispatchReadyRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    if !path_without_query(&request.captured.path_and_query).ends_with("/responses/compact") {
        return Ok(request);
    }
    let provider_dispatch =
        match runtime_gateway_application_provider_dispatch(&request.application_admission, shared)
        {
            Ok(dispatch) => dispatch,
            Err(_) => {
                return Err(request
                    .state
                    .reject(build_runtime_proxy_json_error_response(
                        503,
                        "governed_provider_unavailable",
                        "governed provider dispatch is unavailable",
                    )));
            }
        };
    let selected_shared = provider_dispatch.selected_shared(shared);
    if let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } = &selected_shared.provider {
        respond_runtime_gemini_compact_request(
            request.state.request_id,
            request.state.request,
            &request.captured,
            &selected_shared,
            auth,
        );
        return Err(RuntimeLocalRewritePipelineExit::Handled);
    }
    if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = &selected_shared.provider {
        let parts = runtime_kiro_compact_response_parts(
            request.state.request_id,
            &request.captured.body,
            &selected_shared.runtime_shared.async_runtime,
            auth,
        );
        let response = runtime_local_rewrite_response_with_call_id(
            parts,
            request.state.request_id,
            &selected_shared,
        );
        return Err(request.state.respond(response));
    }
    if matches!(
        selected_shared.provider,
        RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) {
        return Ok(request);
    }
    let response = runtime_local_rewrite_response_with_call_id(
        runtime_gemini_local_compact_response_parts(&request.captured.body),
        request.state.request_id,
        &selected_shared,
    );
    Err(request.state.respond(response))
}

pub(super) fn runtime_local_rewrite_dispatch_builtin_models<'target>(
    request: RuntimeLocalRewriteDispatchReadyRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    let Some(response) = runtime_local_rewrite_builtin_models_response(
        request.state.request_id,
        &request.captured,
        shared,
    ) else {
        return Ok(request);
    };
    Err(request.state.respond(response))
}

fn runtime_local_rewrite_builtin_models_response(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<tiny_http::ResponseBox> {
    if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = &shared.provider
        && let Some(parts) =
            runtime_kiro_models_buffered_response(auth, &request.method, &request.path_and_query)
    {
        runtime_local_rewrite_log_builtin_response(request_id, request, parts.status, shared);
        return Some(runtime_local_rewrite_response_with_call_id(
            parts, request_id, shared,
        ));
    }
    let mut catalog = runtime_copilot_model_catalog_from_provider(&shared.provider);
    if catalog.is_empty() {
        catalog = runtime_kiro_model_catalog_from_provider(&shared.provider);
    }
    let parts = runtime_provider_models_buffered_response(
        shared.provider.bridge_kind(),
        (!catalog.is_empty()).then_some(catalog.as_slice()),
        &request.method,
        &request.path_and_query,
    )?;
    runtime_local_rewrite_log_builtin_response(request_id, request, parts.status, shared);
    Some(runtime_local_rewrite_response_with_call_id(
        parts, request_id, shared,
    ))
}

fn runtime_local_rewrite_log_builtin_response(
    request_id: u64,
    request: &RuntimeProxyRequest,
    status: u16,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_provider_request_ledger_message(
            request_id,
            shared.provider.bridge_kind(),
            &request.path_and_query,
            None,
            status,
            0,
            request.body.len(),
        ),
    );
}

pub(super) fn runtime_local_rewrite_dispatch_provider(
    mut request: RuntimeLocalRewriteDispatchReadyRequest<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<()> {
    runtime_local_rewrite_log_governance_decision(&request, shared);
    let response_governance =
        super::super::local_rewrite_response::RuntimeGatewayResponseGovernance {
            obligations: request.application_admission.response_obligations(),
            audit_context: request.application_admission.audit_context().map(
                |(tenant, principal)| {
                    super::super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext::new(
                        tenant, principal,
                    )
                },
            ),
        };
    let candidate_count = request
        .application_admission
        .routing()
        .map_or(1, |routing| 1 + routing.fallbacks.len());
    let provider_dispatch =
        runtime_gateway_application_provider_dispatch(&request.application_admission, shared);
    let (mut primary_dispatch, mut last_error) = match provider_dispatch {
        Ok(dispatch) => (Some(dispatch), None),
        Err(error) => (None, Some(anyhow::anyhow!(error))),
    };
    let mut selected_response = None;
    for attempt_index in 0..candidate_count {
        let provider_dispatch = if attempt_index == 0 {
            let Some(dispatch) = primary_dispatch.take() else {
                continue;
            };
            dispatch
        } else {
            match runtime_gateway_application_provider_dispatch_attempt(
                &request.application_admission,
                shared,
                attempt_index,
            ) {
                Ok(dispatch) => dispatch,
                Err(error) => {
                    last_error = Some(anyhow::anyhow!(error));
                    continue;
                }
            }
        };
        let selected_shared = provider_dispatch.selected_shared(shared);
        match send_runtime_local_rewrite_upstream_request(
            request.state.request_id,
            &request.captured,
            &selected_shared,
            &provider_dispatch,
        ) {
            Ok(response)
                if runtime_local_rewrite_buffered_provider_fallback_class(
                    &response,
                    selected_shared.provider.bridge_kind(),
                )
                .is_some_and(|class| {
                    runtime_gateway_application_provider_retry_precommit(
                        ProviderRetryCause::NextProvider,
                        class,
                        attempt_index,
                        candidate_count,
                    )
                }) =>
            {
                last_error = Some(anyhow::anyhow!("provider precommit fallback"));
            }
            Ok(response) => {
                if let Some(guard) = request.state.guards.route_load.as_mut() {
                    guard.mark_status(response.status());
                }
                selected_response = Some((response, selected_shared));
                break;
            }
            Err(error)
                if runtime_gateway_application_provider_retry_precommit(
                    ProviderRetryCause::NextProvider,
                    ProviderErrorClass::Transient,
                    attempt_index,
                    candidate_count,
                ) =>
            {
                last_error = Some(error);
            }
            Err(error) => {
                last_error = Some(error);
                break;
            }
        }
    }
    let Some((response, selected_shared)) = selected_response else {
        if let Some(guard) = request.state.guards.route_load.as_mut() {
            guard.mark_error();
        }
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_upstream_error",
                [
                    runtime_proxy_log_field("request", request.state.request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field(
                        "error",
                        last_error
                            .as_ref()
                            .map(runtime_local_rewrite_error_log_value)
                            .unwrap_or_else(|| "upstream_request_failed".to_string()),
                    ),
                ],
            ),
        );
        return Err(request
            .state
            .reject(runtime_local_rewrite_upstream_request_failed_response()));
    };
    respond_runtime_local_rewrite_proxy_request(
        request.state.request_id,
        request.state.request,
        response,
        &request.captured,
        &selected_shared,
        response_governance,
    );
    Ok(())
}

fn runtime_local_rewrite_buffered_provider_fallback_class(
    response: &RuntimeLocalRewriteUpstreamResult,
    provider: RuntimeProviderBridgeKind,
) -> Option<ProviderErrorClass> {
    let RuntimeLocalRewriteUpstreamResponse::Buffered(parts) = &response.response else {
        return None;
    };
    if parts.status < 400 {
        return None;
    }
    let class = runtime_provider_error_class(provider, parts.status, &parts.body);
    match class {
        ProviderErrorClass::Quota | ProviderErrorClass::Transient => Some(class),
        ProviderErrorClass::RateLimit
            if std::str::from_utf8(&parts.body).is_ok_and(|body| {
                let body = body.to_ascii_lowercase();
                body.contains("rate_limit_exceeded") || body.contains("rate_limit_exceeded_error")
            }) =>
        {
            Some(class)
        }
        _ => None,
    }
}

fn runtime_local_rewrite_log_governance_decision(
    request: &RuntimeLocalRewriteDispatchReadyRequest<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let Some(governance) = request.application_admission.governance() else {
        return;
    };
    let routing = request.application_admission.routing();
    let effect = match governance.policy.effect {
        prodex_domain::PolicyEffect::Allow => "allow",
        prodex_domain::PolicyEffect::Deny => "deny",
        prodex_domain::PolicyEffect::RequireApproval => "require_approval",
    };
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_governance_decision",
            [
                runtime_proxy_log_field("request", request.state.request_id.to_string()),
                runtime_proxy_log_field(
                    "classification",
                    governance.classification.classification().as_str(),
                ),
                runtime_proxy_log_field("coverage", governance.classification.coverage().as_str()),
                runtime_proxy_log_field("effect", effect),
                runtime_proxy_log_field(
                    "policy_revision",
                    governance.policy.policy_revision.to_string(),
                ),
                runtime_proxy_log_field(
                    "obligation_count",
                    governance.policy.obligations.len().to_string(),
                ),
                runtime_proxy_log_field(
                    "provider",
                    routing
                        .map(|routing| routing.primary.provider.label())
                        .unwrap_or("legacy-observe"),
                ),
                runtime_proxy_log_field(
                    "registry_revision",
                    routing
                        .map(|routing| routing.registry_revision.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                ),
                runtime_proxy_log_field(
                    "score_revision",
                    routing
                        .map(|routing| routing.score_revision.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                ),
            ],
        ),
    );
}

fn runtime_local_rewrite_upstream_request_failed_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_text_response(502, RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE)
}

fn runtime_local_rewrite_error_log_value(_err: &anyhow::Error) -> String {
    "upstream_request_failed".to_string()
}

#[cfg(test)]
mod error_log_tests {
    use super::*;

    fn buffered(status: u16, body: &[u8]) -> RuntimeLocalRewriteUpstreamResult {
        RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                RuntimeHeapTrimmedBufferedResponseParts {
                    status,
                    headers: Vec::new(),
                    body: body.to_vec().into(),
                },
            ),
            gemini_context: None,
            copilot_context: None,
        }
    }

    #[test]
    fn upstream_error_log_value_is_content_free() {
        let error =
            anyhow::anyhow!("Bearer secret-sentinel for user@example.com in raw provider response");

        assert_eq!(
            runtime_local_rewrite_error_log_value(&error),
            "upstream_request_failed"
        );
    }

    #[test]
    fn provider_fallback_requires_explicit_rate_limit_or_retryable_precommit_error() {
        assert_eq!(
            runtime_local_rewrite_buffered_provider_fallback_class(
                &buffered(429, b"too many requests"),
                RuntimeProviderBridgeKind::OpenAiResponses,
            ),
            None,
        );
        assert_eq!(
            runtime_local_rewrite_buffered_provider_fallback_class(
                &buffered(429, br#"{"error":{"code":"rate_limit_exceeded"}}"#),
                RuntimeProviderBridgeKind::OpenAiResponses,
            ),
            Some(ProviderErrorClass::RateLimit),
        );
        assert_eq!(
            runtime_local_rewrite_buffered_provider_fallback_class(
                &buffered(503, b"temporarily unavailable"),
                RuntimeProviderBridgeKind::OpenAiResponses,
            ),
            Some(ProviderErrorClass::Transient),
        );
    }
}

pub(super) fn runtime_gateway_operational_probe_response(
    method: &str,
    request_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let probe = match request_path.split('?').next().unwrap_or(request_path) {
        "/livez" => "livez",
        "/readyz" => "readyz",
        "/startupz" => "startupz",
        _ => return None,
    };
    if method != "GET" && method != "HEAD" {
        return Some(runtime_gateway_probe_method_rejection(probe));
    }
    let overloaded = runtime_proxy_local_overload_pressure_active(&shared.runtime_shared);
    let draining = shared.gateway_draining.load(Ordering::SeqCst);
    let ready = probe != "readyz" || (!overloaded && !draining);
    let state = if ready {
        "ok"
    } else if draining {
        "draining"
    } else {
        "overloaded"
    };
    let body = serde_json::json!({
        "object": "gateway.health",
        "probe": probe,
        "status": state,
        "ready": ready,
        "local_overload": overloaded,
        "draining": draining,
        "policy_version": shared.gateway_policy_version,
        "active_requests": shared.runtime_shared.active_request_count.load(Ordering::SeqCst),
        "active_request_limit": shared.runtime_shared.active_request_limit,
    })
    .to_string();
    Some(build_runtime_proxy_response_from_parts(
        RuntimeHeapTrimmedBufferedResponseParts {
            status: if ready { 200 } else { 503 },
            headers: vec![("content-type".to_string(), b"application/json".to_vec())],
            body: if method == "HEAD" {
                Vec::new().into()
            } else {
                body.into_bytes().into()
            },
        },
    ))
}

fn runtime_gateway_probe_method_rejection(probe: &str) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 405,
        headers: vec![
            ("content-type".to_string(), b"application/json".to_vec()),
            ("allow".to_string(), b"GET, HEAD".to_vec()),
        ],
        body: serde_json::json!({
            "object": "gateway.health",
            "probe": probe,
            "status": "method_not_allowed"
        })
        .to_string()
        .into_bytes()
        .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::runtime_local_rewrite_error_log_value;

    #[test]
    fn local_rewrite_error_log_value_redacts_secret_like_chain() {
        let err = anyhow::anyhow!(
            "upstream failed\nAuthorization: Bearer local-rewrite-token\napi_key=local-rewrite-key"
        )
        .context("local rewrite upstream failed");
        let message = runtime_local_rewrite_error_log_value(&err);

        assert_eq!(message, "upstream_request_failed");
    }
}
