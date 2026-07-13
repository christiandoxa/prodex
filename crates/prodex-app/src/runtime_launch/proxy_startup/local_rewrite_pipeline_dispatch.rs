use super::{
    RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE, RuntimeLocalRewriteDispatchReadyRequest,
    RuntimeLocalRewritePipelineExit, RuntimeLocalRewritePipelineResult,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared, RuntimeProxyRequest,
    build_runtime_proxy_json_error_response, build_runtime_proxy_text_response, path_without_query,
    respond_runtime_gemini_compact_request, respond_runtime_local_rewrite_proxy_request,
    runtime_copilot_model_catalog_from_provider, runtime_gateway_application_provider_dispatch,
    runtime_kiro_compact_response_parts, runtime_kiro_model_catalog_from_provider,
    runtime_kiro_models_buffered_response, runtime_local_rewrite_response_with_call_id,
    runtime_provider_models_buffered_response, runtime_provider_request_ledger_message,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
    send_runtime_local_rewrite_upstream_request,
};
use crate::runtime_proxy::{
    RuntimeHeapTrimmedBufferedResponseParts, build_runtime_proxy_response_from_parts,
    runtime_proxy_local_overload_pressure_active,
};
use std::sync::atomic::Ordering;

pub(super) fn runtime_local_rewrite_dispatch_compact<'target>(
    request: RuntimeLocalRewriteDispatchReadyRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    if !path_without_query(&request.captured.path_and_query).ends_with("/responses/compact") {
        return Ok(request);
    }
    if runtime_gateway_application_provider_dispatch(&request.application_admission, shared)
        .is_err()
    {
        return Err(request
            .state
            .reject(build_runtime_proxy_json_error_response(
                503,
                "governed_provider_unavailable",
                "governed provider dispatch is unavailable",
            )));
    }
    if let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } = &shared.provider {
        respond_runtime_gemini_compact_request(
            request.state.request_id,
            request.state.request,
            &request.captured,
            shared,
            auth,
        );
        return Err(RuntimeLocalRewritePipelineExit::Handled);
    }
    if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = &shared.provider {
        let parts = runtime_kiro_compact_response_parts(
            request.state.request_id,
            &request.captured.body,
            &shared.runtime_shared.async_runtime,
            auth,
        );
        let response =
            runtime_local_rewrite_response_with_call_id(parts, request.state.request_id, shared);
        return Err(request.state.respond(response));
    }
    if matches!(
        shared.provider,
        RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) {
        return Ok(request);
    }
    let response = build_runtime_proxy_text_response(
        501,
        &runtime_local_rewrite_remote_compact_unsupported_message(&shared.provider),
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
    request: RuntimeLocalRewriteDispatchReadyRequest<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<()> {
    runtime_local_rewrite_log_governance_decision(&request, shared);
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
    let response_obligations = request.application_admission.response_obligations();
    let response = match send_runtime_local_rewrite_upstream_request(
        request.state.request_id,
        &request.captured,
        shared,
        &provider_dispatch,
    ) {
        Ok(response) => response,
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_upstream_error",
                    [
                        runtime_proxy_log_field("request", request.state.request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field(
                            "error",
                            runtime_local_rewrite_error_log_value(&err),
                        ),
                    ],
                ),
            );
            return Err(request
                .state
                .reject(runtime_local_rewrite_upstream_request_failed_response()));
        }
    };
    respond_runtime_local_rewrite_proxy_request(
        request.state.request_id,
        request.state.request,
        response,
        &request.captured,
        shared,
        response_obligations,
    );
    Ok(())
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

    #[test]
    fn upstream_error_log_value_is_content_free() {
        let error =
            anyhow::anyhow!("Bearer secret-sentinel for user@example.com in raw provider response");

        assert_eq!(
            runtime_local_rewrite_error_log_value(&error),
            "upstream_request_failed"
        );
    }
}

pub(in super::super) fn runtime_local_rewrite_remote_compact_unsupported_message(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> String {
    let provider_name = match provider {
        RuntimeLocalRewriteProviderOptions::ProjectedCredential { provider, .. } => {
            return runtime_local_rewrite_remote_compact_unsupported_message(provider);
        }
        RuntimeLocalRewriteProviderOptions::Anthropic { .. } => "Anthropic",
        RuntimeLocalRewriteProviderOptions::Copilot { .. } => "GitHub Copilot",
        RuntimeLocalRewriteProviderOptions::OpenAiResponses { .. } => "OpenAI",
        RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly { .. } => "Prodex local embeddings",
        RuntimeLocalRewriteProviderOptions::Gemini { .. } => "Gemini",
        RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => "DeepSeek",
        RuntimeLocalRewriteProviderOptions::Kiro { .. } => "Kiro",
    };
    format!("{provider_name} provider does not support Codex remote compact yet")
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
