use super::super::local_rewrite::RUNTIME_LOCAL_REWRITE_PROFILE;
use super::super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
};
use super::super::local_rewrite_application_data_plane::{
    runtime_gateway_application_provider_dispatch, runtime_gateway_route_uses_compact_dispatch,
    runtime_gateway_route_uses_models_dispatch,
};
use super::super::local_rewrite_copilot::runtime_copilot_model_catalog_from_provider;
use super::super::local_rewrite_gemini_compact::runtime_gemini_compact_response;
use super::super::local_rewrite_gemini_compact::runtime_local_compact_response_parts_with_reason;
use super::super::local_rewrite_kiro::{
    runtime_kiro_compact_response_parts, runtime_kiro_model_catalog_from_provider,
    runtime_kiro_models_buffered_response,
};
use super::super::local_rewrite_response::{
    respond_runtime_local_rewrite_proxy_request, runtime_local_rewrite_response_with_call_id,
};
use super::super::local_rewrite_upstream::send_runtime_local_rewrite_upstream_request;
use super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteAcceptedBinding, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_local_rewrite_binding_recorder,
    runtime_local_rewrite_previous_response_id, runtime_local_rewrite_request_bound_binding,
    runtime_local_rewrite_route_kind,
};
use super::super::provider_bridge::{RuntimeProviderRouteKind, runtime_provider_route_kind};
use super::super::provider_bridge::{
    runtime_provider_models_buffered_response, runtime_provider_request_ledger_message,
};
use super::{
    RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE, RuntimeLocalRewriteDispatchReadyRequest,
    RuntimeLocalRewritePipelineResult, runtime_local_rewrite_request_timeout_response,
};
use crate::runtime_proxy::{
    build_runtime_proxy_json_error_response, build_runtime_proxy_text_response,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
#[path = "local_rewrite_pipeline_dispatch/provider_precommit.rs"]
mod provider_precommit;
use prodex_provider_core::{ProviderId, RuntimeProviderBindingIdentity};
use provider_precommit::{
    runtime_local_rewrite_precommit_live_provider_response,
    runtime_local_rewrite_provider_fallback_class, runtime_local_rewrite_record_provider_health,
    runtime_local_rewrite_record_provider_metric,
};
use std::time::Instant;

pub(super) fn runtime_local_rewrite_dispatch_compact(
    request: RuntimeLocalRewriteDispatchReadyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
    if !runtime_gateway_route_uses_compact_dispatch(&request.captured.path_and_query) {
        return Ok(request);
    }
    let dispatch = match runtime_gateway_application_provider_dispatch(&request.admission, shared) {
        Ok(dispatch) => dispatch,
        Err(_) => {
            return Err(request
                .state
                .reject(build_runtime_proxy_json_error_response(
                    503,
                    "provider_unavailable",
                    "provider dispatch is unavailable",
                )));
        }
    };
    let selected_shared = dispatch.selected_shared(shared);
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

pub(super) fn runtime_local_rewrite_dispatch_builtin_models(
    request: RuntimeLocalRewriteDispatchReadyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
    if !runtime_gateway_route_uses_models_dispatch(&request.captured.path_and_query) {
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
    request: RuntimeLocalRewriteDispatchReadyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<()> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
    let dispatch = match runtime_gateway_application_provider_dispatch(&request.admission, shared) {
        Ok(dispatch) => dispatch,
        Err(_) => {
            return Err(request
                .state
                .reject(build_runtime_proxy_json_error_response(
                    503,
                    "provider_unavailable",
                    "provider dispatch is unavailable",
                )));
        }
    };
    let selected_shared = dispatch.selected_shared(shared);
    let selected_provider = dispatch.provider();
    let selected_binding_identity =
        runtime_local_rewrite_single_binding_identity(&selected_shared, selected_provider);
    if runtime_local_rewrite_validate_bound_provider(
        &selected_shared,
        &request.captured,
        selected_provider,
        selected_binding_identity.as_ref(),
    )
    .is_err()
    {
        return Err(request
            .state
            .reject(build_runtime_proxy_json_error_response(
                503,
                "bound_continuation_unavailable",
                "bound continuation provider is unavailable",
            )));
    }

    let route_kind = runtime_local_rewrite_route_kind(dispatch.endpoint());
    let started_at = Instant::now();
    let mut result = send_runtime_local_rewrite_upstream_request(
        request.state.request_id,
        &request.captured,
        &selected_shared,
        &dispatch,
    );
    if let Ok(response) = result.as_mut()
        && let Err(error) = runtime_local_rewrite_precommit_live_provider_response(
            response,
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
    {
        result = Err(error);
    }
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
        RUNTIME_LOCAL_REWRITE_PROFILE,
        route_kind,
        &result,
        fallback_class,
    );

    let Ok(mut response) = result else {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_upstream_error",
                [
                    runtime_proxy_log_field("request", request.state.request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("error", "upstream_request_failed"),
                ],
            ),
        );
        return Err(request
            .state
            .reject(runtime_local_rewrite_upstream_request_failed_response()));
    };
    if let Some(identity) = selected_binding_identity {
        runtime_local_rewrite_attach_accepted_binding(
            &mut response,
            &selected_shared,
            &request.captured,
            identity,
        );
    }
    respond_runtime_local_rewrite_proxy_request(
        request.state.request_id,
        request.state.request,
        response,
        &request.captured,
        &selected_shared,
        Default::default(),
    );
    Ok(())
}

fn runtime_local_rewrite_validate_bound_provider(
    shared: &RuntimeLocalRewriteProxyShared,
    request: &RuntimeProxyRequest,
    selected_provider: ProviderId,
    selected_identity: Option<&RuntimeProviderBindingIdentity>,
) -> Result<(), anyhow::Error> {
    let Some(binding) = runtime_local_rewrite_request_bound_binding(shared, request)? else {
        return Ok(());
    };
    if let Some(bound) = binding.binding_identity.as_ref() {
        if bound.provider() != selected_provider {
            anyhow::bail!("bound continuation provider is unavailable");
        }
        if selected_identity.is_some_and(|selected| selected != bound) {
            anyhow::bail!("bound continuation provider identity is unavailable");
        }
    }
    Ok(())
}

fn runtime_local_rewrite_single_binding_identity(
    shared: &RuntimeLocalRewriteProxyShared,
    provider: ProviderId,
) -> Option<RuntimeProviderBindingIdentity> {
    if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = shared.provider.as_ref() {
        return RuntimeProviderBindingIdentity::from_profile(
            provider,
            &auth.profile_name,
            &shared.upstream_base_url,
        );
    }
    RuntimeProviderBindingIdentity::from_profile(
        provider,
        RUNTIME_LOCAL_REWRITE_PROFILE,
        &shared.upstream_base_url,
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

fn runtime_local_rewrite_upstream_request_failed_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_text_response(502, RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE)
}
