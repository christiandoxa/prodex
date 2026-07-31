use super::super::local_rewrite::RUNTIME_LOCAL_REWRITE_PROFILE;
use super::super::local_rewrite_application_data_plane::RuntimeGatewayApplicationProviderDispatch;
use super::super::local_rewrite_gemini_compact::runtime_gemini_local_compact_response_parts;
use super::super::local_rewrite_upstream::runtime_local_rewrite_route_kind;
use super::{
    RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE, RuntimeLocalRewriteDispatchReadyRequest,
    RuntimeLocalRewritePipelineResult, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, RuntimeProviderBridgeKind, RuntimeProxyRequest,
    build_runtime_proxy_json_error_response, build_runtime_proxy_text_response, path_without_query,
    respond_runtime_local_rewrite_proxy_request, runtime_copilot_model_catalog_from_provider,
    runtime_gateway_application_provider_dispatch,
    runtime_gateway_application_provider_dispatch_attempt,
    runtime_gateway_application_provider_retry_precommit, runtime_gemini_compact_response,
    runtime_kiro_compact_response_parts, runtime_kiro_model_catalog_from_provider,
    runtime_kiro_models_buffered_response, runtime_local_rewrite_request_timeout_response,
    runtime_local_rewrite_response_with_call_id, runtime_provider_error_class,
    runtime_provider_models_buffered_response, runtime_provider_request_ledger_message,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
    send_runtime_local_rewrite_upstream_request,
};
use crate::runtime_proxy::{
    RuntimeHeapTrimmedBufferedResponseParts, build_runtime_proxy_response_from_parts,
    bump_runtime_profile_health_score, commit_runtime_proxy_profile_selection_with_policy,
    note_runtime_profile_transport_failure, runtime_proxy_local_overload_pressure_active,
};
use crate::{RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY, RuntimeRouteKind};
use prodex_provider_core::ProviderErrorClass;
use prodex_provider_spi::ProviderRetryCause;
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
    let result = send_runtime_local_rewrite_upstream_request(
        request.state.request_id,
        &request.captured,
        &selected_shared,
        &provider_dispatch,
    );
    runtime_local_rewrite_record_provider_metric(
        selected_shared.provider.bridge_kind(),
        &result,
        started_at.elapsed(),
    );
    runtime_local_rewrite_record_provider_health(
        shared,
        profile_name,
        route_kind,
        selected_shared.provider.bridge_kind(),
        &result,
    );
    match result {
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

fn runtime_local_rewrite_record_provider_health(
    shared: &RuntimeLocalRewriteProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    provider: RuntimeProviderBridgeKind,
    result: &anyhow::Result<RuntimeLocalRewriteUpstreamResult>,
) {
    match result {
        Err(error) => note_runtime_profile_transport_failure(
            &shared.runtime_shared,
            profile_name,
            route_kind,
            "governed_provider_dispatch",
            error,
        ),
        Ok(response) if (200..400).contains(&response.status()) => {
            let _ = commit_runtime_proxy_profile_selection_with_policy(
                &shared.runtime_shared,
                profile_name,
                route_kind,
                false,
            );
        }
        Ok(response)
            if response.status() == 503
                || runtime_local_rewrite_buffered_provider_fallback_class(response, provider)
                    == Some(ProviderErrorClass::Transient) =>
        {
            let _ = bump_runtime_profile_health_score(
                &shared.runtime_shared,
                profile_name,
                route_kind,
                RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                "governed_provider_overload",
            );
        }
        Ok(_) => {}
    }
}

fn runtime_local_rewrite_record_provider_metric(
    provider: RuntimeProviderBridgeKind,
    result: &anyhow::Result<RuntimeLocalRewriteUpstreamResult>,
    duration: std::time::Duration,
) {
    let provider = match provider {
        RuntimeProviderBridgeKind::OpenAiResponses => prodex_observability::ProviderKind::OpenAi,
        RuntimeProviderBridgeKind::Anthropic => prodex_observability::ProviderKind::Anthropic,
        RuntimeProviderBridgeKind::Gemini => prodex_observability::ProviderKind::Gemini,
        RuntimeProviderBridgeKind::Copilot
        | RuntimeProviderBridgeKind::DeepSeek
        | RuntimeProviderBridgeKind::Kiro => prodex_observability::ProviderKind::Other,
    };
    let result = match result {
        Err(_) => prodex_observability::ProviderResultClass::TransportError,
        Ok(response) => runtime_local_rewrite_provider_result_class(response.status()),
    };
    crate::record_runtime_provider_metric(
        provider,
        result,
        duration.as_millis().try_into().unwrap_or(u64::MAX),
    );
}

fn runtime_local_rewrite_provider_result_class(
    status: u16,
) -> prodex_observability::ProviderResultClass {
    match status {
        200..=399 => prodex_observability::ProviderResultClass::Success,
        429 => prodex_observability::ProviderResultClass::RateLimited,
        503 => prodex_observability::ProviderResultClass::Overloaded,
        _ => prodex_observability::ProviderResultClass::ProviderError,
    }
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
    let probe_state = runtime_gateway_operational_probe_state(shared, probe);
    let (probe_metric, result_metric) =
        runtime_gateway_operational_probe_metrics(probe, &probe_state);
    crate::record_runtime_health_probe_metric(probe_metric, result_metric);
    let body = serde_json::json!({
        "object": "gateway.health",
        "probe": probe,
        "status": probe_state.status,
        "ready": probe_state.ready,
        "local_overload": probe_state.overloaded,
        "draining": probe_state.draining,
        "credentials_stale": probe_state.credentials_stale,
        "governance_audit_available": probe_state.governance_audit_available,
        "governance_policy_available": probe_state.governance_policy_available,
        "policy_version": shared.gateway_policy_version,
        "active_requests": shared.runtime_shared.active_request_count.load(Ordering::SeqCst),
        "active_request_limit": shared.runtime_shared.active_request_limit,
    })
    .to_string();
    Some(build_runtime_proxy_response_from_parts(
        RuntimeHeapTrimmedBufferedResponseParts {
            status: if probe_state.ready { 200 } else { 503 },
            headers: vec![("content-type".to_string(), b"application/json".to_vec())],
            body: if method == "HEAD" {
                Vec::new().into()
            } else {
                body.into_bytes().into()
            },
        },
    ))
}

struct RuntimeGatewayOperationalProbeState {
    status: &'static str,
    ready: bool,
    overloaded: bool,
    draining: bool,
    credentials_stale: bool,
    governance_audit_available: bool,
    governance_policy_available: bool,
}

fn runtime_gateway_operational_probe_state(
    shared: &RuntimeLocalRewriteProxyShared,
    probe: &str,
) -> RuntimeGatewayOperationalProbeState {
    let overloaded = runtime_proxy_local_overload_pressure_active(&shared.runtime_shared);
    let draining = shared.gateway_draining.load(Ordering::SeqCst);
    let credentials_stale = shared.gateway_credentials.refresh_is_stale();
    let governance_policy_available = runtime_gateway_mandatory_governance_available(shared);
    let governance_audit_available =
        super::super::local_rewrite_governance_audit::runtime_governance_audit_is_available(shared);
    let ready = probe != "readyz"
        || (!overloaded
            && !draining
            && !credentials_stale
            && governance_policy_available
            && governance_audit_available);
    let status = if ready {
        "ok"
    } else if draining {
        "draining"
    } else if credentials_stale {
        "credentials_stale"
    } else if !governance_audit_available {
        "governance_audit_unavailable"
    } else if !governance_policy_available {
        "governance_policy_unavailable"
    } else {
        "overloaded"
    };
    RuntimeGatewayOperationalProbeState {
        status,
        ready,
        overloaded,
        draining,
        credentials_stale,
        governance_audit_available,
        governance_policy_available,
    }
}

fn runtime_gateway_operational_probe_metrics(
    probe: &str,
    state: &RuntimeGatewayOperationalProbeState,
) -> (
    prodex_observability::HealthProbeKind,
    prodex_observability::HealthProbeResult,
) {
    let probe_metric = match probe {
        "livez" => prodex_observability::HealthProbeKind::Live,
        "readyz" => prodex_observability::HealthProbeKind::Ready,
        _ => prodex_observability::HealthProbeKind::Startup,
    };
    let result_metric = if state.draining {
        prodex_observability::HealthProbeResult::Draining
    } else if state.overloaded
        || state.credentials_stale
        || !state.governance_policy_available
        || !state.governance_audit_available
    {
        prodex_observability::HealthProbeResult::Degraded
    } else {
        prodex_observability::HealthProbeResult::Passing
    };
    (probe_metric, result_metric)
}

fn runtime_gateway_mandatory_governance_available(shared: &RuntimeLocalRewriteProxyShared) -> bool {
    if !shared
        .runtime_shared
        .runtime_config
        .governance
        .mode
        .is_enforcing()
    {
        return true;
    }
    let Some(authority) = shared.governance_authority.as_ref() else {
        return false;
    };
    let Ok(tenant_ids) = authority.tenant_ids() else {
        return false;
    };
    shared.governance.policies_are_servable(
        &tenant_ids,
        super::super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis(),
    )
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
