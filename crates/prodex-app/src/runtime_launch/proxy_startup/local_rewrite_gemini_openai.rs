use super::super::chat_compatible_rewrite::{
    RuntimeDeepSeekRewriteOptions, runtime_provider_chat_compatible_request_body,
};
use super::super::deepseek_rewrite::RuntimeDeepSeekPendingRequest;
use super::super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    runtime_local_rewrite_model_selection,
};
use super::super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_gemini_openai_compatible_upstream_url,
    runtime_local_rewrite_api_key_attempts,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_log_request_conformance,
    runtime_provider_model_fallback_chain, runtime_provider_request_body_with_model,
    runtime_provider_request_conformance_result, runtime_provider_should_retry_with_next_model,
    runtime_provider_should_rotate_auth_after_response,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Result, bail};
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(super) fn send_runtime_gemini_openai_compatible_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_keys: &[String],
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
        bail!("Gemini API-key pool is empty");
    }
    let attempt_count = api_key_attempts.len();
    let model_selection = runtime_local_rewrite_model_selection(
        shared,
        RuntimeProviderBridgeKind::Gemini,
        request,
        &body,
        GEMINI_DEFAULT_MODEL,
    );
    let model_chain = runtime_provider_model_fallback_chain(
        RuntimeProviderBridgeKind::Gemini,
        &model_selection.model,
    );
    let upstream_url = runtime_gemini_openai_compatible_upstream_url(&shared.upstream_base_url);
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_openai_compatible",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("endpoint", upstream_url.as_str()),
                runtime_proxy_log_field("auth", "api-key"),
                runtime_proxy_log_field("attempts", attempt_count.to_string()),
            ],
        ),
    );

    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            if let Some(result) = runtime_provider_request_conformance_result(
                RuntimeProviderBridgeKind::Gemini,
                request,
                &model_body,
            ) {
                runtime_provider_log_request_conformance(
                    &shared.runtime_shared,
                    request_id,
                    RuntimeProviderBridgeKind::Gemini,
                    &result,
                );
            }
            let translated = runtime_provider_chat_compatible_request_body(
                &model_body,
                &shared.deepseek_conversations,
                RuntimeProviderBridgeKind::Gemini,
                GEMINI_DEFAULT_MODEL,
                true,
                RuntimeDeepSeekRewriteOptions::default(),
            )?;
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
                        provider_kind: RuntimeProviderBridgeKind::Gemini,
                        auth_label: api_key_label.as_str(),
                        model,
                        auth_factory: || RuntimeLocalRewritePreparedAuth::GeminiOpenAi { api_key },
                    },
                )?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }
                RuntimeLocalRewritePreparedSendResult::Error {
                    status,
                    parts,
                    class,
                } => (status, parts, class),
            };
            if model_index + 1 < model_chain.len()
                && runtime_provider_should_retry_with_next_model(class)
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("provider", "gemini-openai"),
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
            if api_key_index + 1 < attempt_count
                && runtime_provider_should_rotate_auth_after_response(class)
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_auth_rotate",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("provider", "gemini-openai"),
                            runtime_proxy_log_field("auth", api_key_label.as_str()),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                break;
            }
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
    }
    bail!("no Gemini OpenAI-compatible attempts were available")
}
