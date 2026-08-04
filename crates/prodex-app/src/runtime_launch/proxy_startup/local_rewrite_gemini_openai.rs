use super::super::chat_compatible_rewrite::{
    RuntimeDeepSeekRewriteOptions, runtime_provider_chat_compatible_request_body,
};
use super::super::deepseek_rewrite::RuntimeDeepSeekPendingRequest;
use super::super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    runtime_local_rewrite_model_selection,
};
use super::super::local_rewrite_application_data_plane::runtime_gateway_application_provider_retry_precommit;
use super::super::local_rewrite_gemini_quota::runtime_gemini_429_is_structured;
use super::super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_gemini_openai_compatible_upstream_url,
    runtime_local_rewrite_api_key_attempts,
};
use super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteBindingContext, runtime_local_rewrite_attach_accepted_binding,
    runtime_local_rewrite_binding_context, runtime_local_rewrite_raw_binding_identity,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_log_request_conformance,
    runtime_provider_model_fallback_chain, runtime_provider_request_body_with_model,
    runtime_provider_request_conformance_result,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Result, bail};
use prodex_provider_core::PRODEX_GEMINI_DEFAULT_MODEL as GEMINI_DEFAULT_MODEL;
use prodex_provider_core::{ProviderId, RuntimeProviderBindingIdentity};
use prodex_provider_spi::ProviderRetryCause;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(super) fn send_runtime_gemini_openai_compatible_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_keys: &[String],
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let binding = runtime_local_rewrite_binding_context(shared, request)?;
    let binding_endpoint = shared.upstream_base_url.clone();
    let mut api_key_attempts = if shared.provider_credential.is_some() {
        vec![("projected".to_string(), None)]
    } else {
        runtime_local_rewrite_api_key_attempts(shared, api_keys)
            .into_iter()
            .map(|(label, api_key)| (label, Some(api_key)))
            .collect()
    };
    api_key_attempts.retain(|(_, api_key)| {
        runtime_gemini_openai_binding_identity(shared, *api_key, &binding_endpoint)
            .as_ref()
            .is_some_and(|identity| binding.candidate_allowed(Some(identity)))
    });
    if api_key_attempts.is_empty() {
        if binding.bound.is_some() {
            bail!("Gemini continuation binding is unavailable or unauthorized");
        }
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
    let conversations = shared.deepseek_conversations_for_request(request);
    runtime_gemini_openai_attempts(
        &RuntimeGeminiOpenAiAttemptContext {
            request_id,
            request,
            shared,
            conversations: &conversations,
            base_body: &model_selection.body,
            model_chain: &model_chain,
            upstream_url: &upstream_url,
            attempt_count,
            binding: &binding,
            binding_endpoint: &binding_endpoint,
        },
        api_key_attempts,
    )
}

fn runtime_gemini_openai_binding_identity(
    shared: &RuntimeLocalRewriteProxyShared,
    api_key: Option<&str>,
    endpoint: &str,
) -> Option<RuntimeProviderBindingIdentity> {
    runtime_local_rewrite_raw_binding_identity(
        shared,
        ProviderId::Gemini,
        api_key,
        endpoint,
        api_key.is_none().then_some(RUNTIME_LOCAL_REWRITE_PROFILE),
    )
}

struct RuntimeGeminiOpenAiAttemptContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeLocalRewriteProxyShared,
    conversations: &'a super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore,
    base_body: &'a [u8],
    model_chain: &'a [String],
    upstream_url: &'a str,
    attempt_count: usize,
    binding: &'a RuntimeLocalRewriteBindingContext,
    binding_endpoint: &'a str,
}

fn runtime_gemini_openai_attempts(
    context: &RuntimeGeminiOpenAiAttemptContext<'_>,
    api_key_attempts: Vec<(String, Option<&str>)>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        for (model_index, model) in context.model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(context.base_body, model);
            let (send_result, pending_request) = send_gemini_openai_model_request(
                context,
                &model_body,
                api_key_label.as_str(),
                api_key,
                model,
            )?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    let Some(binding_identity) = runtime_gemini_openai_binding_identity(
                        context.shared,
                        api_key,
                        context.binding_endpoint,
                    ) else {
                        bail!("Gemini accepted binding identity is unavailable");
                    };
                    let mut live_response = RuntimeLocalRewriteLiveResponse::new(response);
                    runtime_local_rewrite_attach_accepted_binding(
                        context.shared,
                        &mut live_response,
                        context.binding,
                        binding_identity,
                    );
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            live_response.with_chat_compatible_request(pending_request),
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
            if status == 429 && !runtime_gemini_429_is_structured(&parts.body) {
                return Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                    gemini_context: None,
                    copilot_context: None,
                });
            }
            if model_index + 1 < context.model_chain.len()
                && runtime_gateway_application_provider_retry_precommit(
                    ProviderRetryCause::NextModel,
                    class,
                    model_index,
                    context.model_chain.len(),
                )
            {
                runtime_proxy_log(
                    &context.shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", context.request_id.to_string()),
                            runtime_proxy_log_field("provider", "gemini-openai"),
                            runtime_proxy_log_field("auth", api_key_label.as_str()),
                            runtime_proxy_log_field("from_model", model.as_str()),
                            runtime_proxy_log_field(
                                "to_model",
                                context.model_chain[model_index + 1].as_str(),
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
                context.attempt_count,
            ) {
                runtime_proxy_log(
                    &context.shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_auth_rotate",
                        [
                            runtime_proxy_log_field("request", context.request_id.to_string()),
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

fn send_gemini_openai_model_request(
    context: &RuntimeGeminiOpenAiAttemptContext<'_>,
    model_body: &[u8],
    api_key_label: &str,
    api_key: Option<&str>,
    model: &str,
) -> Result<(
    RuntimeLocalRewritePreparedSendResult,
    RuntimeDeepSeekPendingRequest,
)> {
    if let Some(result) = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        context.request,
        model_body,
    ) {
        runtime_provider_log_request_conformance(
            &context.shared.runtime_shared,
            context.request_id,
            RuntimeProviderBridgeKind::Gemini,
            &result,
        );
    }
    let translated = runtime_provider_chat_compatible_request_body(
        model_body,
        context.conversations,
        RuntimeProviderBridgeKind::Gemini,
        GEMINI_DEFAULT_MODEL,
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )?;
    let pending_request = RuntimeDeepSeekPendingRequest {
        messages: translated.messages,
        response_metadata: translated.response_metadata,
    };
    let send_result = send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
        RuntimeLocalRewriteSearchFallbackRequest {
            request_id: context.request_id,
            request: context.request,
            shared: context.shared,
            upstream_url: context.upstream_url,
            body: translated.body,
            provider_kind: RuntimeProviderBridgeKind::Gemini,
            auth_label: api_key_label,
            model,
            auth_factory: || RuntimeLocalRewritePreparedAuth::GeminiOpenAi { api_key },
        },
    )?;
    Ok((send_result, pending_request))
}
