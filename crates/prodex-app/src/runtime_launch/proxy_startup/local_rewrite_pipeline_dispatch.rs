use super::super::local_rewrite::RUNTIME_LOCAL_REWRITE_PROFILE;
use super::super::local_rewrite_application_data_plane::RuntimeGatewayApplicationProviderDispatch;
use super::super::local_rewrite_gemini_compact::runtime_gemini_local_compact_response_parts;
use super::super::local_rewrite_upstream::runtime_local_rewrite_route_kind;
use super::super::provider_bridge::{RuntimeProviderRouteKind, runtime_provider_route_kind};
#[path = "local_rewrite_pipeline_dispatch/operational_probe.rs"]
mod operational_probe;
#[path = "local_rewrite_pipeline_dispatch/provider_precommit.rs"]
mod provider_precommit;
use super::{
    RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE, RuntimeLocalRewriteDispatchReadyRequest,
    RuntimeLocalRewritePipelineResult, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResult, RuntimeProxyRequest,
    build_runtime_proxy_json_error_response, build_runtime_proxy_text_response, path_without_query,
    respond_runtime_local_rewrite_proxy_request, runtime_copilot_model_catalog_from_provider,
    runtime_gateway_application_provider_dispatch,
    runtime_gateway_application_provider_dispatch_attempt,
    runtime_gateway_application_provider_retry_precommit, runtime_gemini_compact_response,
    runtime_kiro_compact_response_parts, runtime_kiro_model_catalog_from_provider,
    runtime_kiro_models_buffered_response, runtime_local_rewrite_request_timeout_response,
    runtime_local_rewrite_response_with_call_id, runtime_provider_models_buffered_response,
    runtime_provider_request_ledger_message, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, send_runtime_local_rewrite_upstream_request,
};
use crate::runtime_proxy::{
    RuntimeHeapTrimmedBufferedResponseParts, build_runtime_proxy_response_from_parts,
    runtime_proxy_local_overload_pressure_active,
};
pub(super) use operational_probe::runtime_gateway_operational_probe_response;
use prodex_provider_core::ProviderErrorClass;
use prodex_provider_spi::ProviderRetryCause;
use provider_precommit::{
    runtime_local_rewrite_precommit_live_provider_response,
    runtime_local_rewrite_provider_fallback_class, runtime_local_rewrite_record_provider_health,
    runtime_local_rewrite_record_provider_metric,
};
use std::sync::atomic::Ordering;
use std::time::Instant;

pub(super) fn runtime_local_rewrite_dispatch_compact<'target>(
    request: RuntimeLocalRewriteDispatchReadyRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
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
    if let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } =
        selected_shared.provider.as_ref()
    {
        let response = runtime_gemini_compact_response(
            request.state.request_id,
            &request.captured,
            &selected_shared,
            auth,
        );
        return Err(request.state.respond(response));
    }
    if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = selected_shared.provider.as_ref() {
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
        selected_shared.provider.as_ref(),
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
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
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
    if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = shared.provider.as_ref()
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
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
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
            spend_termination: Default::default(),
        };
    let candidate_count = request
        .application_admission
        .routing()
        .map_or(1, |routing| 1 + routing.fallbacks.len());
    let (selected_response, last_error) =
        runtime_local_rewrite_try_provider_candidates(&mut request, shared, candidate_count);
    if request.state.deadline_expired() {
        if let Some(guard) = request.state.guards.route_load.as_mut() {
            guard.mark_error();
        }
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
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

enum RuntimeLocalRewriteProviderAttempt {
    Success(
        Box<(
            RuntimeLocalRewriteUpstreamResult,
            RuntimeLocalRewriteProxyShared,
        )>,
    ),
    Retry(anyhow::Error),
    Stop(anyhow::Error),
}

fn runtime_local_rewrite_try_provider_candidates(
    request: &mut RuntimeLocalRewriteDispatchReadyRequest<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
    candidate_count: usize,
) -> (
    Option<(
        RuntimeLocalRewriteUpstreamResult,
        RuntimeLocalRewriteProxyShared,
    )>,
    Option<anyhow::Error>,
) {
    let (mut primary_dispatch, mut last_error) =
        match runtime_gateway_application_provider_dispatch(&request.application_admission, shared)
        {
            Ok(dispatch) => (Some(dispatch), None),
            Err(error) => (None, Some(anyhow::anyhow!(error))),
        };
    for attempt_index in 0..candidate_count {
        if request.state.deadline_expired() {
            break;
        }
        if attempt_index == 0 && primary_dispatch.is_none() {
            continue;
        }
        match runtime_local_rewrite_provider_attempt(
            request,
            shared,
            attempt_index,
            candidate_count,
            &mut primary_dispatch,
        ) {
            RuntimeLocalRewriteProviderAttempt::Success(result) => {
                let (response, selected_shared) = *result;
                if let Some(guard) = request.state.guards.route_load.as_mut() {
                    guard.mark_status(response.status());
                }
                return (Some((response, selected_shared)), last_error);
            }
            RuntimeLocalRewriteProviderAttempt::Retry(error) => last_error = Some(error),
            RuntimeLocalRewriteProviderAttempt::Stop(error) => {
                last_error = Some(error);
                break;
            }
        }
    }
    (None, last_error)
}

fn runtime_local_rewrite_provider_attempt(
    request: &RuntimeLocalRewriteDispatchReadyRequest<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
    attempt_index: usize,
    candidate_count: usize,
    primary_dispatch: &mut Option<RuntimeGatewayApplicationProviderDispatch<'_>>,
) -> RuntimeLocalRewriteProviderAttempt {
    let provider_dispatch = if attempt_index == 0 {
        let Some(dispatch) = primary_dispatch.take() else {
            return RuntimeLocalRewriteProviderAttempt::Retry(anyhow::anyhow!(
                "provider dispatch unavailable"
            ));
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
                return RuntimeLocalRewriteProviderAttempt::Retry(anyhow::anyhow!(error));
            }
        }
    };
    let selected_provider = provider_dispatch.provider();
    let profile_name = if selected_provider == shared.provider.bridge_kind().provider_id() {
        RUNTIME_LOCAL_REWRITE_PROFILE
    } else {
        selected_provider.label()
    };
    let route_kind = runtime_local_rewrite_route_kind(provider_dispatch.endpoint());
    let selected_shared = provider_dispatch.selected_shared(shared);
    let started_at = Instant::now();
    let result = match send_runtime_local_rewrite_upstream_request(
        request.state.request_id,
        &request.captured,
        &selected_shared,
        &provider_dispatch,
    ) {
        Ok(mut response) => runtime_local_rewrite_precommit_live_provider_response(
            &mut response,
            selected_shared.provider.bridge_kind(),
            matches!(
                runtime_provider_route_kind(&request.captured.path_and_query),
                Some(RuntimeProviderRouteKind::Responses)
            ),
            selected_shared
                .runtime_shared
                .runtime_config
                .tuning
                .sse_lookahead_timeout_ms,
            selected_shared
                .runtime_shared
                .runtime_config
                .tuning
                .stream_idle_timeout_ms,
            &selected_shared.runtime_shared.async_runtime,
            &selected_shared.provider_sse_prefetch_slots,
        )
        .map(|()| response),
        Err(error) => Err(error),
    };
    let fallback_class = result.as_ref().ok().and_then(|response| {
        runtime_local_rewrite_provider_fallback_class(
            response,
            selected_shared.provider.bridge_kind(),
        )
    });
    #[cfg(test)]
    if let Err(error) = &result {
        eprintln!("provider attempt error: {error:#}");
    }
    runtime_local_rewrite_record_provider_metric(
        selected_shared.provider.bridge_kind(),
        &result,
        fallback_class,
        started_at.elapsed(),
    );
    runtime_local_rewrite_record_provider_health(
        shared,
        profile_name,
        route_kind,
        &result,
        fallback_class,
    );
    match result {
        Ok(_response)
            if fallback_class.is_some_and(|class| {
                runtime_gateway_application_provider_retry_precommit(
                    ProviderRetryCause::NextProvider,
                    class,
                    attempt_index,
                    candidate_count,
                )
            }) =>
        {
            RuntimeLocalRewriteProviderAttempt::Retry(anyhow::anyhow!(
                "provider precommit fallback"
            ))
        }
        Ok(response) => {
            RuntimeLocalRewriteProviderAttempt::Success(Box::new((response, selected_shared)))
        }
        Err(error)
            if runtime_gateway_application_provider_retry_precommit(
                ProviderRetryCause::NextProvider,
                ProviderErrorClass::Transient,
                attempt_index,
                candidate_count,
            ) =>
        {
            RuntimeLocalRewriteProviderAttempt::Retry(error)
        }
        Err(error) => RuntimeLocalRewriteProviderAttempt::Stop(error),
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
    use super::super::super::local_rewrite_upstream::{
        RuntimeLocalRewriteLiveBody, RuntimeLocalRewriteLiveResponse,
        RuntimeLocalRewriteUpstreamResponse,
    };
    use super::super::super::provider_bridge::RuntimeProviderBridgeKind;
    use super::*;
    use prodex_provider_core::ProviderErrorClass;
    use std::sync::Arc;

    fn test_async_runtime() -> Arc<tokio::runtime::Runtime> {
        Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("SSE test runtime should build"),
        )
    }

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
            runtime_local_rewrite_provider_fallback_class(
                &buffered(429, b"too many requests"),
                RuntimeProviderBridgeKind::OpenAiResponses,
            ),
            None,
        );
        assert_eq!(
            runtime_local_rewrite_provider_fallback_class(
                &buffered(429, br#"{"error":{"code":"rate_limit_exceeded"}}"#),
                RuntimeProviderBridgeKind::OpenAiResponses,
            ),
            Some(ProviderErrorClass::RateLimit),
        );
        assert_eq!(
            runtime_local_rewrite_provider_fallback_class(
                &buffered(503, b"temporarily unavailable"),
                RuntimeProviderBridgeKind::OpenAiResponses,
            ),
            Some(ProviderErrorClass::Transient),
        );
    }

    fn live_sse(body: &'static str) -> RuntimeLocalRewriteUpstreamResult {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("SSE test server should bind");
        let address = server
            .server_addr()
            .to_ip()
            .expect("SSE test server should expose an IP address");
        let sender = std::thread::spawn(move || {
            let request = server
                .recv()
                .expect("SSE test server should receive a request");
            request
                .respond(tiny_http::Response::new(
                    tiny_http::StatusCode(200),
                    vec![
                        tiny_http::Header::from_bytes("content-type", "text/event-stream")
                            .expect("SSE content type header"),
                    ],
                    Box::new(std::io::Cursor::new(body.as_bytes().to_vec())),
                    None,
                    None,
                ))
                .expect("SSE test server should respond");
        });
        let response = reqwest::blocking::Client::new()
            .get(format!("http://{address}"))
            .send()
            .expect("SSE test client should receive a response");
        sender.join().expect("SSE test server should finish");
        RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Live(
                RuntimeLocalRewriteLiveResponse::new(response),
            ),
            gemini_context: None,
            copilot_context: None,
        }
    }

    #[test]
    fn provider_sse_first_retryable_event_is_precommit_classified_for_chat_compatible_candidates() {
        // This guard intentionally covers only chat-compatible /v1/responses adapters.
        // Native provider protocols, including Anthropic Messages, stay outside this
        // precommit fallback contract until they have an explicit equivalent.
        let async_runtime = test_async_runtime();
        for (provider, body, expected) in [
            (
                RuntimeProviderBridgeKind::DeepSeek,
                concat!(r#"data: {"error":{"code":"insufficient_quota"}}"#, "\n\n"),
                ProviderErrorClass::Quota,
            ),
            (
                RuntimeProviderBridgeKind::Anthropic,
                concat!(r#"data: {"error":{"code":"rate_limit_exceeded"}}"#, "\n\n"),
                ProviderErrorClass::RateLimit,
            ),
            (
                RuntimeProviderBridgeKind::Gemini,
                concat!(r#"data: {"error":{"code":"server_is_overloaded"}}"#, "\n\n"),
                ProviderErrorClass::Transient,
            ),
        ] {
            let mut response = live_sse(body);
            runtime_local_rewrite_precommit_live_provider_response(
                &mut response,
                provider,
                true,
                crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
                crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
                &async_runtime,
                &Arc::new(tokio::sync::Semaphore::new(1)),
            )
            .expect("SSE precommit lookahead should succeed");
            assert_eq!(
                runtime_local_rewrite_provider_fallback_class(&response, provider),
                Some(expected)
            );
            let RuntimeLocalRewriteUpstreamResponse::Live(mut live) = response.response else {
                panic!("SSE test response should remain live");
            };
            let mut tail = Vec::new();
            let mut reader = live
                .body
                .take()
                .expect("SSE test body should remain available")
                .into_reader();
            std::io::Read::read_to_end(&mut reader, &mut tail)
                .expect("SSE test response should remain readable");
            let mut reconstructed = live.prefix;
            reconstructed.extend(tail);
            assert_eq!(reconstructed, body.as_bytes());
        }

        let body = concat!(
            r#"data: {"type":"response.output_text.delta","delta":"overloaded"}"#,
            "\n\n"
        );
        let mut response = live_sse(body);
        runtime_local_rewrite_precommit_live_provider_response(
            &mut response,
            RuntimeProviderBridgeKind::DeepSeek,
            true,
            crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
            crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
            &async_runtime,
            &Arc::new(tokio::sync::Semaphore::new(1)),
        )
        .expect("ordinary SSE lookahead should succeed");
        assert_eq!(
            runtime_local_rewrite_provider_fallback_class(
                &response,
                RuntimeProviderBridgeKind::DeepSeek,
            ),
            None
        );

        let mut response = live_sse(body);
        runtime_local_rewrite_precommit_live_provider_response(
            &mut response,
            RuntimeProviderBridgeKind::DeepSeek,
            false,
            crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
            crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
            &async_runtime,
            &Arc::new(tokio::sync::Semaphore::new(1)),
        )
        .expect("non-Responses SSE should remain untouched");
        let RuntimeLocalRewriteUpstreamResponse::Live(live) = response.response else {
            panic!("SSE test response should remain live");
        };
        assert!(live.prefix.is_empty());
    }

    #[test]
    fn provider_sse_prefetch_saturation_preserves_the_original_live_body() {
        let async_runtime = test_async_runtime();
        let mut response = live_sse("data: {}\n\n");

        runtime_local_rewrite_precommit_live_provider_response(
            &mut response,
            RuntimeProviderBridgeKind::DeepSeek,
            true,
            crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
            crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
            &async_runtime,
            &Arc::new(tokio::sync::Semaphore::new(0)),
        )
        .expect("saturated lookahead should pass through");

        let RuntimeLocalRewriteUpstreamResponse::Live(live) = response.response else {
            panic!("SSE test response should remain live");
        };
        assert!(live.prefix.is_empty());
        assert!(matches!(
            live.body,
            Some(RuntimeLocalRewriteLiveBody::Response(_))
        ));
    }
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
