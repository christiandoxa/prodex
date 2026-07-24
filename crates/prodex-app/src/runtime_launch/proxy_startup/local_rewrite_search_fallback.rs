use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_model_memory::runtime_local_rewrite_model_scope;
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_label,
};
use super::provider_tools::{
    runtime_provider_chat_request_body_without_web_search_options,
    runtime_provider_error_rejects_web_search_options,
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, runtime_proxy_log};
use anyhow::Result;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(super) enum RuntimeLocalRewritePreparedSendResult {
    Live(reqwest::blocking::Response),
    Error {
        status: u16,
        parts: RuntimeHeapTrimmedBufferedResponseParts,
        class: RuntimeProviderErrorClass,
    },
}

pub(super) struct RuntimeLocalRewriteSearchFallbackRequest<'a, F> {
    pub(super) request_id: u64,
    pub(super) request: &'a RuntimeProxyRequest,
    pub(super) shared: &'a RuntimeLocalRewriteProxyShared,
    pub(super) upstream_url: &'a str,
    pub(super) body: Vec<u8>,
    pub(super) provider_kind: RuntimeProviderBridgeKind,
    pub(super) auth_label: &'a str,
    pub(super) model: &'a str,
    pub(super) auth_factory: F,
}

pub(super) fn send_runtime_local_rewrite_prepared_request_with_chat_search_fallback<'a, F>(
    options: RuntimeLocalRewriteSearchFallbackRequest<'a, F>,
) -> Result<RuntimeLocalRewritePreparedSendResult>
where
    F: Fn() -> RuntimeLocalRewritePreparedAuth<'a>,
{
    let RuntimeLocalRewriteSearchFallbackRequest {
        request_id,
        request,
        shared,
        upstream_url,
        body,
        provider_kind,
        auth_label,
        model,
        auth_factory,
    } = options;
    let fallback_body = runtime_provider_chat_request_body_without_web_search_options(&body);
    let response = send_runtime_local_rewrite_prepared_request(
        request_id,
        request,
        shared,
        upstream_url,
        body,
        auth_factory(),
    )?;
    let mut status = response.status().as_u16();
    if status < 400 {
        runtime_local_rewrite_remember_accepted_model(shared, provider_kind, request, model);
        return Ok(RuntimeLocalRewritePreparedSendResult::Live(response));
    }

    let mut parts = runtime_local_rewrite_buffered_response_from_response(response)?;
    let mut class = runtime_provider_error_class(provider_kind, status, &parts.body);
    if status == 400
        && runtime_provider_error_rejects_web_search_options(&parts.body)
        && let Some(fallback_body) = fallback_body
    {
        runtime_local_rewrite_log_chat_search_fallback(
            request_id,
            shared,
            provider_kind,
            auth_label,
            model,
            status,
        );
        let fallback_response = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            upstream_url,
            fallback_body,
            auth_factory(),
        )?;
        status = fallback_response.status().as_u16();
        if status < 400 {
            runtime_local_rewrite_remember_accepted_model(shared, provider_kind, request, model);
            return Ok(RuntimeLocalRewritePreparedSendResult::Live(
                fallback_response,
            ));
        }
        parts = runtime_local_rewrite_buffered_response_from_response(fallback_response)?;
        class = runtime_provider_error_class(provider_kind, status, &parts.body);
    }

    Ok(RuntimeLocalRewritePreparedSendResult::Error {
        status,
        parts,
        class,
    })
}

pub(super) fn runtime_local_rewrite_remember_accepted_model(
    shared: &RuntimeLocalRewriteProxyShared,
    provider_kind: RuntimeProviderBridgeKind,
    request: &RuntimeProxyRequest,
    model: &str,
) {
    if !shared.gateway_request_constraints.enabled {
        return;
    }
    if let Some(scope) = runtime_local_rewrite_model_scope(provider_kind, request, &request.body)
        && let Ok(mut memory) = shared.model_memory.lock()
    {
        memory.remember_selected_model(&scope, model);
    }
}

fn runtime_local_rewrite_log_chat_search_fallback(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    provider_kind: RuntimeProviderBridgeKind,
    auth_label: &str,
    model: &str,
    status: u16,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_web_search_options_fallback",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", runtime_provider_label(provider_kind)),
                runtime_proxy_log_field("auth", auth_label),
                runtime_proxy_log_field("model", model),
                runtime_proxy_log_field("status", status.to_string()),
            ],
        ),
    );
}
