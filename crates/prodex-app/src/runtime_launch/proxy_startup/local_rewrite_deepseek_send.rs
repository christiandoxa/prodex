use super::super::deepseek_rewrite::{
    RuntimeDeepSeekPendingRequest, RuntimeDeepSeekRewriteOptions,
    runtime_deepseek_chat_request_body_with_options,
};
use super::super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    runtime_local_rewrite_model_selection,
};
use super::super::local_rewrite_application_data_plane::runtime_gateway_application_provider_retry_precommit;
use super::super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_deepseek_upstream_url,
    runtime_local_rewrite_api_key_attempts, send_runtime_local_rewrite_prepared_request,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_label, runtime_provider_log_request_conformance,
    runtime_provider_model_fallback_chain, runtime_provider_request_body_with_model,
    runtime_provider_request_conformance_result,
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, runtime_proxy_log};
use anyhow::Result;
use prodex_provider_core::{
    ProviderEndpoint,
    deepseek_provider_core_request_body as core_deepseek_provider_core_request_body,
    deepseek_provider_core_simple_request,
};
use prodex_provider_spi::ProviderRetryCause;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(in super::super) fn send_runtime_deepseek_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_keys: &[String],
    endpoint: ProviderEndpoint,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let api_key_attempts = if shared.provider_credential.is_some() {
        vec![("projected".to_string(), None)]
    } else {
        runtime_local_rewrite_api_key_attempts(shared, api_keys)
            .into_iter()
            .map(|(label, api_key)| (label, Some(api_key)))
            .collect()
    };
    if api_key_attempts.is_empty() {
        anyhow::bail!("DeepSeek provider has no API keys configured");
    }
    let api_key_attempt_count = api_key_attempts.len();
    if endpoint == ProviderEndpoint::Responses {
        send_runtime_deepseek_responses_request(
            request_id,
            request,
            shared,
            body,
            api_key_attempts,
            api_key_attempt_count,
        )
    } else {
        send_runtime_deepseek_passthrough_request(
            request_id,
            request,
            shared,
            body,
            api_key_attempts,
            api_key_attempt_count,
        )
    }
}

fn send_runtime_deepseek_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_key_attempts: Vec<(String, Option<&str>)>,
    api_key_attempt_count: usize,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let model_selection = runtime_local_rewrite_model_selection(
        shared,
        RuntimeProviderBridgeKind::DeepSeek,
        request,
        &body,
        prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL,
    );
    let model_chain = runtime_provider_model_fallback_chain(
        RuntimeProviderBridgeKind::DeepSeek,
        &model_selection.model,
    );
    let (strict_tools, beta_base_url, web_search_mode) = match &shared.provider {
        super::super::local_rewrite_options::RuntimeLocalRewriteProviderOptions::DeepSeek {
            strict_tools,
            beta_base_url,
            web_search_mode,
            ..
        } => (*strict_tools, beta_base_url.as_str(), *web_search_mode),
        _ => (
            false,
            shared.upstream_base_url.as_str(),
            super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode::Auto,
        ),
    };
    let upstream_base_url = if strict_tools {
        beta_base_url
    } else {
        &shared.upstream_base_url
    };
    let upstream_url = runtime_deepseek_upstream_url(
        upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );
    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            let conformance = runtime_provider_request_conformance_result(
                RuntimeProviderBridgeKind::DeepSeek,
                request,
                &model_body,
            );
            if let Some(result) = conformance.as_ref() {
                runtime_provider_log_request_conformance(
                    &shared.runtime_shared,
                    request_id,
                    RuntimeProviderBridgeKind::DeepSeek,
                    result,
                );
            }
            let mut translated = runtime_deepseek_chat_request_body_with_options(
                &model_body,
                &shared.deepseek_conversations,
                RuntimeDeepSeekRewriteOptions {
                    strict_tools,
                    web_search_mode,
                },
            )?;
            if deepseek_provider_core_simple_request(&model_body, |previous_response_id| {
                shared
                    .deepseek_conversations
                    .lock()
                    .ok()
                    .is_some_and(|store| store.contains_key(previous_response_id))
            }) && let Some(body) = conformance
                .as_ref()
                .and_then(core_deepseek_provider_core_request_body)
            {
                translated.body = body;
            }
            if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                pending.insert(
                    request_id,
                    RuntimeDeepSeekPendingRequest {
                        messages: translated.messages,
                        response_metadata: translated.response_metadata,
                    },
                );
            }
            let send_result =
                send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                    RuntimeLocalRewriteSearchFallbackRequest {
                        request_id,
                        request,
                        shared,
                        upstream_url: &upstream_url,
                        body: translated.body,
                        provider_kind: RuntimeProviderBridgeKind::DeepSeek,
                        auth_label: &api_key_label,
                        model,
                        auth_factory: || RuntimeLocalRewritePreparedAuth::DeepSeek { api_key },
                    },
                )?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(runtime_deepseek_live_result(response));
                }
                RuntimeLocalRewritePreparedSendResult::Error {
                    status,
                    parts,
                    class,
                } => (status, parts, class),
            };
            if model_index + 1 < model_chain.len()
                && runtime_gateway_application_provider_retry_precommit(
                    ProviderRetryCause::NextModel,
                    class,
                    model_index,
                    model_chain.len(),
                )
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field(
                                "provider",
                                runtime_provider_label(RuntimeProviderBridgeKind::DeepSeek),
                            ),
                            runtime_proxy_log_field("auth", api_key_label.as_str()),
                            runtime_proxy_log_field("from_model", model.as_str()),
                            runtime_proxy_log_field(
                                "to_model",
                                model_chain[model_index + 1].as_str(),
                            ),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                continue;
            }
            if runtime_gateway_application_provider_retry_precommit(
                ProviderRetryCause::RotateCredential,
                class,
                api_key_index,
                api_key_attempt_count,
            ) {
                runtime_deepseek_log_auth_rotate(shared, request_id, &api_key_label, status, class);
                break;
            }
            return Ok(runtime_deepseek_buffered_result(parts));
        }
        if api_key_index + 1 < api_key_attempt_count {
            continue;
        }
    }
    anyhow::bail!("no DeepSeek model attempts were available");
}

fn send_runtime_deepseek_passthrough_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_key_attempts: Vec<(String, Option<&str>)>,
    api_key_attempt_count: usize,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let upstream_url = runtime_deepseek_upstream_url(
        &shared.upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );
    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        let response = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            &upstream_url,
            body.clone(),
            RuntimeLocalRewritePreparedAuth::DeepSeek { api_key },
        )?;
        let status = response.status().as_u16();
        if status >= 400 {
            let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
            let class = runtime_provider_error_class(
                RuntimeProviderBridgeKind::DeepSeek,
                status,
                &parts.body,
            );
            if runtime_gateway_application_provider_retry_precommit(
                ProviderRetryCause::RotateCredential,
                class,
                api_key_index,
                api_key_attempt_count,
            ) {
                runtime_deepseek_log_auth_rotate(shared, request_id, &api_key_label, status, class);
                continue;
            }
            return Ok(runtime_deepseek_buffered_result(parts));
        }
        return Ok(runtime_deepseek_live_result(response));
    }
    anyhow::bail!("no DeepSeek API key attempts were available")
}

fn runtime_deepseek_log_auth_rotate(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    api_key_label: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_auth_rotate",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::DeepSeek),
                ),
                runtime_proxy_log_field("auth", api_key_label),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("class", format!("{class:?}")),
            ],
        ),
    );
}

fn runtime_deepseek_buffered_result(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
        gemini_context: None,
        copilot_context: None,
    }
}

fn runtime_deepseek_live_result(
    response: reqwest::blocking::Response,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Live(RuntimeLocalRewriteLiveResponse::new(
            response,
        )),
        gemini_context: None,
        copilot_context: None,
    }
}
