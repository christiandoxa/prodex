use super::super::local_rewrite::RUNTIME_LOCAL_REWRITE_PROFILE;
use super::super::local_rewrite_application_data_plane::RuntimeGatewayApplicationProviderDispatch;
use super::super::local_rewrite_application_data_plane::{
    runtime_gateway_route_uses_compact_dispatch, runtime_gateway_route_uses_models_dispatch,
};
use super::super::local_rewrite_gemini_compact::runtime_local_compact_response_parts_with_reason;
use super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteAcceptedBinding, RuntimeLocalRewriteUpstreamResponse,
    runtime_local_rewrite_binding_recorder, runtime_local_rewrite_continuation_is_bound,
    runtime_local_rewrite_previous_response_id, runtime_local_rewrite_request_bound_binding,
    runtime_local_rewrite_route_kind,
};
use super::super::provider_bridge::{RuntimeProviderRouteKind, runtime_provider_route_kind};
#[path = "local_rewrite_pipeline_dispatch/operational_probe.rs"]
mod operational_probe;
#[path = "local_rewrite_pipeline_dispatch/planning.rs"]
mod planning;
#[path = "local_rewrite_pipeline_dispatch/provider_precommit.rs"]
mod provider_precommit;
#[path = "local_rewrite_pipeline_dispatch/quota.rs"]
pub(super) mod quota;
use super::{
    RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE, RuntimeLocalRewriteDispatchReadyRequest,
    RuntimeLocalRewritePipelineResult, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResult, RuntimeProxyRequest,
    build_runtime_proxy_json_error_response, build_runtime_proxy_text_response,
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
use planning::{
    RuntimeLocalRewriteAttemptResult, RuntimeLocalRewriteBindingDecision,
    RuntimeLocalRewriteCandidateAttempt, runtime_local_rewrite_attempt_result,
    runtime_local_rewrite_binding_decision, runtime_local_rewrite_candidate_attempt,
    runtime_local_rewrite_candidate_count,
};
use prodex_provider_core::{ProviderErrorClass, RuntimeProviderBindingIdentity};
use prodex_provider_spi::{ProviderRetryCause, runtime_provider_binding_identity_from_secret_ref};
#[cfg(test)]
use provider_precommit::runtime_local_rewrite_provider_result_class;
use provider_precommit::{
    runtime_local_rewrite_precommit_live_provider_response,
    runtime_local_rewrite_provider_fallback_class, runtime_local_rewrite_record_provider_health,
    runtime_local_rewrite_record_provider_metric,
};
use std::{sync::atomic::Ordering, time::Instant};
pub(super) fn runtime_local_rewrite_dispatch_compact<'target>(
    request: RuntimeLocalRewriteDispatchReadyRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
    if !runtime_gateway_route_uses_compact_dispatch(request.state.context.route()) {
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
    let selected_provider = provider_dispatch.provider();
    let selected_binding_identity =
        runtime_local_rewrite_single_binding_identity(&selected_shared, selected_provider);
    if selected_binding_identity.as_ref().is_some_and(|identity| {
        runtime_local_rewrite_validate_bound_provider(
            &selected_shared,
            &request.captured,
            selected_provider,
            Some(identity),
        )
        .is_err()
    }) {
        return Err(request
            .state
            .reject(build_runtime_proxy_json_error_response(
                503,
                "bound_continuation_unavailable",
                "bound continuation provider is unavailable",
            )));
    }
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
        runtime_local_compact_response_parts_with_reason(
            &request.captured.body,
            selected_shared.provider.bridge_kind().provider_id().label(),
            "local-policy",
        ),
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
    if !runtime_gateway_route_uses_models_dispatch(request.state.context.route()) {
        return Ok(request);
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
    let mut catalog = match runtime_copilot_model_catalog_from_provider(&shared.provider) {
        Ok(catalog) => catalog,
        Err(error) => {
            runtime_local_rewrite_log_builtin_response(request_id, request, 503, shared);
            return Some(build_runtime_proxy_json_error_response(
                503,
                "model_catalog_limit_exceeded",
                &error.to_string(),
            ));
        }
    };
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
    let hard_continuation =
        runtime_local_rewrite_continuation_is_bound(shared, &request.captured).unwrap_or(true);
    let fallback_count = request
        .application_admission
        .routing()
        .map_or(0, |routing| routing.fallbacks.len());
    let (selected_response, last_error) = runtime_local_rewrite_try_provider_candidates(
        &mut request,
        shared,
        hard_continuation,
        fallback_count,
    );
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
    hard_continuation: bool,
    fallback_count: usize,
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
    let candidate_count = runtime_local_rewrite_candidate_count(hard_continuation, fallback_count);
    for attempt_index in 0..candidate_count {
        if request.state.deadline_expired() {
            break;
        }
        let attempt = match runtime_local_rewrite_candidate_attempt(
            hard_continuation,
            fallback_count,
            attempt_index,
            primary_dispatch.is_some(),
        ) {
            RuntimeLocalRewriteCandidateAttempt::Stop => break,
            RuntimeLocalRewriteCandidateAttempt::Skip => continue,
            attempt @ (RuntimeLocalRewriteCandidateAttempt::Primary
            | RuntimeLocalRewriteCandidateAttempt::Fallback) => attempt,
        };
        match runtime_local_rewrite_provider_attempt(
            request,
            shared,
            attempt_index,
            candidate_count,
            attempt,
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
    attempt: RuntimeLocalRewriteCandidateAttempt,
    primary_dispatch: &mut Option<RuntimeGatewayApplicationProviderDispatch<'_>>,
) -> RuntimeLocalRewriteProviderAttempt {
    let provider_dispatch = match attempt {
        RuntimeLocalRewriteCandidateAttempt::Primary => primary_dispatch
            .take()
            .expect("primary dispatch plan requires an available dispatch"),
        RuntimeLocalRewriteCandidateAttempt::Fallback => {
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
        }
        RuntimeLocalRewriteCandidateAttempt::Stop | RuntimeLocalRewriteCandidateAttempt::Skip => {
            unreachable!("non-dispatch attempt must not reach provider transport")
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
    let selected_binding_identity =
        runtime_local_rewrite_single_binding_identity(&selected_shared, selected_provider);
    if let Some(identity) = selected_binding_identity.as_ref()
        && let Err(error) = runtime_local_rewrite_validate_bound_provider(
            &selected_shared,
            &request.captured,
            selected_provider,
            Some(identity),
        )
    {
        return RuntimeLocalRewriteProviderAttempt::Stop(error);
    }
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
    let retry_allowed = fallback_class.is_some_and(|class| {
        runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::NextProvider,
            class,
            attempt_index,
            candidate_count,
        )
    }) || result.is_err()
        && runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::NextProvider,
            ProviderErrorClass::Transient,
            attempt_index,
            candidate_count,
        );
    match runtime_local_rewrite_attempt_result(
        result.is_ok(),
        fallback_class.is_some(),
        retry_allowed,
    ) {
        RuntimeLocalRewriteAttemptResult::Retry => RuntimeLocalRewriteProviderAttempt::Retry(
            anyhow::anyhow!("provider precommit fallback"),
        ),
        RuntimeLocalRewriteAttemptResult::Success => {
            let mut response = result.expect("successful application attempt has a response");
            if let Some(binding_identity) = selected_binding_identity {
                runtime_local_rewrite_attach_accepted_binding(
                    &mut response,
                    &selected_shared,
                    &request.captured,
                    binding_identity,
                );
            }
            RuntimeLocalRewriteProviderAttempt::Success(Box::new((response, selected_shared)))
        }
        RuntimeLocalRewriteAttemptResult::Stop => {
            let Err(error) = result else {
                unreachable!("stopped application attempt must have an error")
            };
            RuntimeLocalRewriteProviderAttempt::Stop(error)
        }
    }
}

fn runtime_local_rewrite_validate_bound_provider(
    shared: &RuntimeLocalRewriteProxyShared,
    request: &RuntimeProxyRequest,
    selected_provider: prodex_provider_core::ProviderId,
    selected_identity: Option<&RuntimeProviderBindingIdentity>,
) -> Result<(), anyhow::Error> {
    let Some(binding) = runtime_local_rewrite_request_bound_binding(shared, request)? else {
        return Ok(());
    };
    runtime_local_rewrite_validate_resolved_bound_provider(
        binding.binding_identity.as_ref(),
        selected_provider,
        selected_identity,
    )
}

fn runtime_local_rewrite_validate_resolved_bound_provider(
    bound_identity: Option<&RuntimeProviderBindingIdentity>,
    selected_provider: prodex_provider_core::ProviderId,
    selected_identity: Option<&RuntimeProviderBindingIdentity>,
) -> Result<(), anyhow::Error> {
    let decision = runtime_local_rewrite_binding_decision(
        true,
        bound_identity.is_some(),
        bound_identity.is_some_and(|identity| identity.provider() == selected_provider),
        selected_identity.is_some(),
        bound_identity
            .zip(selected_identity)
            .is_some_and(|(bound, selected)| bound == selected),
        selected_provider,
    );
    match decision {
        RuntimeLocalRewriteBindingDecision::Valid => Ok(()),
        RuntimeLocalRewriteBindingDecision::MissingIdentity => Err(anyhow::anyhow!(
            "bound continuation has no exact provider identity"
        )),
        RuntimeLocalRewriteBindingDecision::ProviderMismatch => Err(anyhow::anyhow!(
            "bound continuation provider is unavailable or unauthorized"
        )),
        RuntimeLocalRewriteBindingDecision::IdentityMismatch => Err(anyhow::anyhow!(
            "bound continuation provider identity is unavailable or unauthorized"
        )),
        RuntimeLocalRewriteBindingDecision::SelectedIdentityRequired => Err(anyhow::anyhow!(
            "bound continuation provider identity is unavailable"
        )),
    }
}

fn runtime_local_rewrite_single_binding_identity(
    shared: &RuntimeLocalRewriteProxyShared,
    provider: prodex_provider_core::ProviderId,
) -> Option<RuntimeProviderBindingIdentity> {
    if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = shared.provider.as_ref() {
        return shared
            .provider_credential
            .as_ref()
            .and_then(|credential| {
                runtime_provider_binding_identity_from_secret_ref(
                    provider,
                    credential.reference(),
                    &shared.upstream_base_url,
                    Some(&auth.profile_name),
                )
            })
            .or_else(|| {
                RuntimeProviderBindingIdentity::from_profile(
                    provider,
                    &auth.profile_name,
                    &shared.upstream_base_url,
                )
            });
    }
    runtime_provider_binding_identity_from_secret_ref(
        provider,
        shared.provider_credential.as_ref()?.reference(),
        &shared.upstream_base_url,
        Some(RUNTIME_LOCAL_REWRITE_PROFILE),
    )
}

fn runtime_local_rewrite_attach_accepted_binding(
    response: &mut RuntimeLocalRewriteUpstreamResult,
    shared: &RuntimeLocalRewriteProxyShared,
    request: &RuntimeProxyRequest,
    identity: RuntimeProviderBindingIdentity,
) {
    let recorder = runtime_local_rewrite_binding_recorder(shared, identity.clone());
    let accepted = RuntimeLocalRewriteAcceptedBinding {
        identity,
        previous_response_id: runtime_local_rewrite_previous_response_id(&request.body),
        turn_state: runtime_proxy_crate::runtime_request_turn_state(request),
        session_id: runtime_proxy_crate::runtime_request_session_id(request),
    };
    match &mut response.response {
        RuntimeLocalRewriteUpstreamResponse::Live(live) => {
            live.accepted_binding_recorder.get_or_insert(recorder);
            live.accepted_binding.get_or_insert(accepted);
        }
        RuntimeLocalRewriteUpstreamResponse::Streaming(streaming) => {
            streaming.accepted_binding_recorder.get_or_insert(recorder);
            streaming.accepted_binding.get_or_insert(accepted);
        }
        RuntimeLocalRewriteUpstreamResponse::Buffered(_) => {}
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
#[path = "local_rewrite_pipeline_dispatch/error_log_tests.rs"]
mod error_log_tests;

#[cfg(test)]
#[path = "local_rewrite_pipeline_dispatch/tests.rs"]
mod tests;
