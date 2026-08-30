use super::super::deepseek_rewrite::{
    RuntimeDeepSeekPendingRequest, RuntimeDeepSeekRewriteOptions,
    runtime_deepseek_chat_request_body_with_options,
};
use super::super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared,
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
    RuntimeLocalRewritePreparedAuth, runtime_deepseek_anthropic_messages_upstream_url,
    runtime_deepseek_upstream_url, runtime_local_rewrite_api_key_attempts,
    send_runtime_local_rewrite_prepared_request,
};
use super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteBindingContext, RuntimeLocalRewriteLiveResponse,
    RuntimeLocalRewriteNativeFirstEvent, runtime_local_rewrite_attach_accepted_binding,
    runtime_local_rewrite_binding_context, runtime_local_rewrite_precommit_native_first_event,
    runtime_local_rewrite_raw_binding_identity,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_label, runtime_provider_log_request_conformance,
    runtime_provider_model_fallback_chain, runtime_provider_request_body_with_model,
    runtime_provider_request_conformance_result,
};
use super::super::provider_tools::runtime_provider_chat_request_body_without_web_search_options;
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, runtime_proxy_log};
use anyhow::Result;
use prodex_provider_core::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
    RuntimeProviderBindingIdentity, deepseek_provider_core_first_event_retry_allowed,
    deepseek_provider_core_request_body as core_deepseek_provider_core_request_body,
    deepseek_provider_core_simple_request, provider_core_rewritten_body,
    translate_openai_chat_request_to_anthropic_messages,
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
    let binding = runtime_local_rewrite_binding_context(shared, request)?;
    let binding_endpoint = runtime_deepseek_binding_endpoint(shared, endpoint);
    let mut api_key_attempts = if shared.provider_credential.is_some() {
        vec![("projected".to_string(), None)]
    } else {
        runtime_local_rewrite_api_key_attempts(shared, api_keys)
            .into_iter()
            .map(|(label, api_key)| (label, Some(api_key)))
            .collect()
    };
    api_key_attempts.retain(|(_, api_key)| {
        runtime_deepseek_binding_identity(shared, *api_key, &binding_endpoint)
            .as_ref()
            .is_some_and(|identity| binding.candidate_allowed(Some(identity)))
    });
    if api_key_attempts.is_empty() {
        if binding.bound.is_some() {
            anyhow::bail!("DeepSeek continuation binding is unavailable or unauthorized");
        }
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
            binding,
            binding_endpoint,
        )
    } else {
        send_runtime_deepseek_passthrough_request(
            request_id,
            request,
            shared,
            body,
            api_key_attempts,
            api_key_attempt_count,
            binding,
            binding_endpoint,
            endpoint == ProviderEndpoint::Messages,
        )
    }
}

fn runtime_deepseek_binding_endpoint(
    shared: &RuntimeLocalRewriteProxyShared,
    endpoint: ProviderEndpoint,
) -> String {
    match shared.provider.as_ref() {
        super::super::local_rewrite_options::RuntimeLocalRewriteProviderOptions::DeepSeek {
            strict_tools: true,
            beta_base_url,
            ..
        } if endpoint == ProviderEndpoint::Responses => beta_base_url.clone(),
        _ => shared.upstream_base_url.clone(),
    }
}

fn runtime_deepseek_binding_identity(
    shared: &RuntimeLocalRewriteProxyShared,
    api_key: Option<&str>,
    endpoint: &str,
) -> Option<RuntimeProviderBindingIdentity> {
    runtime_local_rewrite_raw_binding_identity(
        shared,
        ProviderId::DeepSeek,
        api_key,
        endpoint,
        api_key.is_none().then_some(RUNTIME_LOCAL_REWRITE_PROFILE),
    )
}

struct RuntimeDeepSeekResponseAttemptContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeLocalRewriteProxyShared,
    conversations: &'a super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore,
    base_body: &'a [u8],
    model_chain: &'a [String],
    chat_upstream_url: &'a str,
    messages_upstream_url: &'a str,
    api_key_attempt_count: usize,
    strict_tools: bool,
    web_search_mode: super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode,
    binding: &'a RuntimeLocalRewriteBindingContext,
    binding_endpoint: &'a str,
}

#[allow(clippy::too_many_arguments)]
fn send_runtime_deepseek_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_key_attempts: Vec<(String, Option<&str>)>,
    api_key_attempt_count: usize,
    binding: RuntimeLocalRewriteBindingContext,
    binding_endpoint: String,
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
    let (strict_tools, beta_base_url, web_search_mode) = match shared.provider.as_ref() {
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
    let chat_upstream_url = runtime_deepseek_upstream_url(
        upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );
    let messages_upstream_url =
        runtime_deepseek_anthropic_messages_upstream_url(&shared.upstream_base_url);
    let conversations = shared.deepseek_conversations_for_request(request);
    let context = RuntimeDeepSeekResponseAttemptContext {
        request_id,
        request,
        shared,
        conversations: &conversations,
        base_body: &model_selection.body,
        model_chain: &model_chain,
        chat_upstream_url: &chat_upstream_url,
        messages_upstream_url: &messages_upstream_url,
        api_key_attempt_count,
        strict_tools,
        web_search_mode,
        binding: &binding,
        binding_endpoint: &binding_endpoint,
    };
    runtime_deepseek_response_attempts(&context, api_key_attempts)
}

fn runtime_deepseek_response_attempts(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    api_key_attempts: Vec<(String, Option<&str>)>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let mut first_event_retries = 0;
    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        for (model_index, model) in context.model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(context.base_body, model);
            let Some(prepared) =
                runtime_deepseek_prepare_model_request(context, &model_body, model)?
            else {
                return Ok(runtime_deepseek_native_translation_incompatible());
            };
            match runtime_deepseek_attempt_control(
                context,
                &api_key_label,
                model,
                api_key,
                prepared,
                (api_key_index, model_index),
                &mut first_event_retries,
            )? {
                RuntimeDeepSeekAttemptControl::Return(result) => return Ok(*result),
                RuntimeDeepSeekAttemptControl::NextModel => continue,
                RuntimeDeepSeekAttemptControl::NextCredential => break,
            }
        }
    }
    anyhow::bail!("no DeepSeek model attempts were available");
}

enum RuntimeDeepSeekAttemptControl {
    Return(Box<RuntimeLocalRewriteUpstreamResult>),
    NextModel,
    NextCredential,
}

fn runtime_deepseek_attempt_control(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    api_key_label: &str,
    model: &str,
    api_key: Option<&str>,
    prepared: RuntimeDeepSeekPreparedModelRequest,
    attempt_index: (usize, usize),
    first_event_retries: &mut u8,
) -> Result<RuntimeDeepSeekAttemptControl> {
    Ok(
        match runtime_deepseek_send_model_attempt(context, api_key_label, model, api_key, prepared)?
        {
            RuntimeDeepSeekModelAttempt::Live { response, pending } => {
                RuntimeDeepSeekAttemptControl::Return(Box::new(runtime_deepseek_live_result(
                    response, pending,
                )))
            }
            RuntimeDeepSeekModelAttempt::NativeFirstEvent {
                response,
                pending,
                class,
            } => runtime_deepseek_native_first_event_control(
                context,
                api_key_label,
                model,
                attempt_index,
                first_event_retries,
                (response, pending, class),
            ),
            RuntimeDeepSeekModelAttempt::Error {
                status,
                parts,
                class,
            } => runtime_deepseek_error_control(
                context,
                api_key_label,
                model,
                attempt_index,
                status,
                (parts, class),
            ),
        },
    )
}

fn runtime_deepseek_native_first_event_control(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    api_key_label: &str,
    model: &str,
    attempt_index: (usize, usize),
    first_event_retries: &mut u8,
    outcome: (
        RuntimeLocalRewriteLiveResponse,
        RuntimeDeepSeekPendingRequest,
        RuntimeProviderErrorClass,
    ),
) -> RuntimeDeepSeekAttemptControl {
    let (api_key_index, model_index) = attempt_index;
    let (response, pending, class) = outcome;
    let can_retry = deepseek_provider_core_first_event_retry_allowed(*first_event_retries, false);
    if can_retry
        && model_index + 1 < context.model_chain.len()
        && runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::NextModel,
            class,
            model_index,
            context.model_chain.len(),
        )
    {
        runtime_deepseek_log_model_fallback(
            context,
            api_key_label,
            model,
            &context.model_chain[model_index + 1],
            200,
            class,
        );
        *first_event_retries += 1;
        return RuntimeDeepSeekAttemptControl::NextModel;
    }
    if can_retry
        && runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::RotateCredential,
            class,
            api_key_index,
            context.api_key_attempt_count,
        )
    {
        runtime_deepseek_log_auth_rotate(
            context.shared,
            context.request_id,
            api_key_label,
            200,
            class,
        );
        *first_event_retries += 1;
        return RuntimeDeepSeekAttemptControl::NextCredential;
    }
    RuntimeDeepSeekAttemptControl::Return(Box::new(runtime_deepseek_live_result(response, pending)))
}

fn runtime_deepseek_error_control(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    api_key_label: &str,
    model: &str,
    attempt_index: (usize, usize),
    status: u16,
    error: (
        RuntimeHeapTrimmedBufferedResponseParts,
        RuntimeProviderErrorClass,
    ),
) -> RuntimeDeepSeekAttemptControl {
    let (api_key_index, model_index) = attempt_index;
    let (parts, class) = error;
    if model_index + 1 < context.model_chain.len()
        && runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::NextModel,
            class,
            model_index,
            context.model_chain.len(),
        )
    {
        runtime_deepseek_log_model_fallback(
            context,
            api_key_label,
            model,
            &context.model_chain[model_index + 1],
            status,
            class,
        );
        return RuntimeDeepSeekAttemptControl::NextModel;
    }
    if runtime_gateway_application_provider_retry_precommit(
        ProviderRetryCause::RotateCredential,
        class,
        api_key_index,
        context.api_key_attempt_count,
    ) {
        runtime_deepseek_log_auth_rotate(
            context.shared,
            context.request_id,
            api_key_label,
            status,
            class,
        );
        return RuntimeDeepSeekAttemptControl::NextCredential;
    }
    RuntimeDeepSeekAttemptControl::Return(Box::new(runtime_deepseek_buffered_result(parts)))
}

struct RuntimeDeepSeekPreparedModelRequest {
    body: Vec<u8>,
    native_messages: bool,
    pending: RuntimeDeepSeekPendingRequest,
}

enum RuntimeDeepSeekModelAttempt {
    Live {
        response: RuntimeLocalRewriteLiveResponse,
        pending: RuntimeDeepSeekPendingRequest,
    },
    NativeFirstEvent {
        response: RuntimeLocalRewriteLiveResponse,
        pending: RuntimeDeepSeekPendingRequest,
        class: RuntimeProviderErrorClass,
    },
    Error {
        status: u16,
        parts: RuntimeHeapTrimmedBufferedResponseParts,
        class: RuntimeProviderErrorClass,
    },
}

fn runtime_deepseek_prepare_model_request(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    model_body: &[u8],
    model: &str,
) -> Result<Option<RuntimeDeepSeekPreparedModelRequest>> {
    let conformance = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        context.request,
        model_body,
    );
    if let Some(result) = conformance.as_ref() {
        runtime_provider_log_request_conformance(
            &context.shared.runtime_shared,
            context.request_id,
            RuntimeProviderBridgeKind::DeepSeek,
            result,
        );
    }
    let mut translated = runtime_deepseek_chat_request_body_with_options(
        model_body,
        context.conversations,
        RuntimeDeepSeekRewriteOptions {
            strict_tools: context.strict_tools,
            web_search_mode: context.web_search_mode,
        },
    )?;
    if deepseek_provider_core_simple_request(model_body, |previous_response_id| {
        context.conversations.contains(previous_response_id)
    }) && let Some(body) = conformance
        .as_ref()
        .and_then(core_deepseek_provider_core_request_body)
    {
        translated.body = body;
    }
    let mut native_messages = runtime_deepseek_uses_native_web_search(
        context.web_search_mode,
        translated.body.as_slice(),
    );
    let body = if native_messages {
        let mut input =
            ProviderTransformInput::new(ProviderEndpoint::Responses, translated.body.clone());
        input.model = Some(model.to_string());
        let result = translate_openai_chat_request_to_anthropic_messages(input);
        runtime_provider_log_request_conformance(
            &context.shared.runtime_shared,
            context.request_id,
            RuntimeProviderBridgeKind::DeepSeek,
            &result,
        );
        match provider_core_rewritten_body(Some(&result)) {
            Some(body) => body,
            None if runtime_deepseek_native_translation_fallback_is_safe(&result) => {
                let Some(body) = runtime_deepseek_auto_chat_fallback_body(
                    translated.body.as_slice(),
                    context.web_search_mode,
                ) else {
                    return Ok(None);
                };
                native_messages = false;
                runtime_deepseek_log_native_search_fallback(context, model);
                body
            }
            None => return Ok(None),
        }
    } else {
        translated.body
    };
    Ok(Some(RuntimeDeepSeekPreparedModelRequest {
        body,
        native_messages,
        pending: RuntimeDeepSeekPendingRequest {
            messages: translated.messages,
            response_metadata: translated.response_metadata,
        },
    }))
}

fn runtime_deepseek_send_model_attempt(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    api_key_label: &str,
    model: &str,
    api_key: Option<&str>,
    prepared: RuntimeDeepSeekPreparedModelRequest,
) -> Result<RuntimeDeepSeekModelAttempt> {
    let RuntimeDeepSeekPreparedModelRequest {
        body,
        native_messages,
        pending,
    } = prepared;
    let send_result = send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
        RuntimeLocalRewriteSearchFallbackRequest {
            request_id: context.request_id,
            request: context.request,
            shared: context.shared,
            upstream_url: if native_messages {
                context.messages_upstream_url
            } else {
                context.chat_upstream_url
            },
            body,
            provider_kind: RuntimeProviderBridgeKind::DeepSeek,
            auth_label: api_key_label,
            model,
            auth_factory: || RuntimeLocalRewritePreparedAuth::DeepSeek {
                api_key,
                native_messages,
            },
        },
    )?;
    Ok(match send_result {
        RuntimeLocalRewritePreparedSendResult::Live(response) => {
            let mut live_response = if native_messages {
                RuntimeLocalRewriteLiveResponse::with_native_anthropic_messages(response)
            } else {
                RuntimeLocalRewriteLiveResponse::new(response)
            };
            let Some(binding_identity) = runtime_deepseek_binding_identity(
                context.shared,
                api_key,
                context.binding_endpoint,
            ) else {
                return Err(anyhow::anyhow!(
                    "DeepSeek accepted binding identity is unavailable"
                ));
            };
            runtime_local_rewrite_attach_accepted_binding(
                context.shared,
                &mut live_response,
                context.binding,
                binding_identity,
            );
            if native_messages {
                match runtime_local_rewrite_precommit_native_first_event(
                    &mut live_response,
                    RuntimeProviderBridgeKind::DeepSeek,
                    context
                        .shared
                        .runtime_shared
                        .runtime_config
                        .tuning
                        .sse_lookahead_timeout_ms,
                    &context.shared.provider_sse_prefetch_slots,
                )? {
                    RuntimeLocalRewriteNativeFirstEvent::Retry(class) => {
                        RuntimeDeepSeekModelAttempt::NativeFirstEvent {
                            response: live_response,
                            pending,
                            class,
                        }
                    }
                    RuntimeLocalRewriteNativeFirstEvent::Commit => {
                        RuntimeDeepSeekModelAttempt::Live {
                            response: live_response,
                            pending,
                        }
                    }
                }
            } else {
                RuntimeDeepSeekModelAttempt::Live {
                    response: live_response,
                    pending,
                }
            }
        }
        RuntimeLocalRewritePreparedSendResult::Error {
            status,
            parts,
            class,
        } => RuntimeDeepSeekModelAttempt::Error {
            status,
            parts,
            class,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn send_runtime_deepseek_passthrough_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_key_attempts: Vec<(String, Option<&str>)>,
    api_key_attempt_count: usize,
    binding: RuntimeLocalRewriteBindingContext,
    binding_endpoint: String,
    native_messages: bool,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let upstream_url = runtime_deepseek_passthrough_upstream_url(
        &shared.upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
        native_messages,
    );
    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        let response = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            &upstream_url,
            body.clone(),
            RuntimeLocalRewritePreparedAuth::DeepSeek {
                api_key,
                native_messages,
            },
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
        let Some(binding_identity) =
            runtime_deepseek_binding_identity(shared, api_key, &binding_endpoint)
        else {
            anyhow::bail!("DeepSeek accepted binding identity is unavailable");
        };
        let mut live_response = RuntimeLocalRewriteLiveResponse::new(response);
        runtime_local_rewrite_attach_accepted_binding(
            shared,
            &mut live_response,
            &binding,
            binding_identity,
        );
        return Ok(runtime_deepseek_live_result(
            live_response,
            RuntimeDeepSeekPendingRequest::default(),
        ));
    }
    anyhow::bail!("no DeepSeek API key attempts were available")
}

fn runtime_deepseek_passthrough_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
    native_messages: bool,
) -> String {
    if native_messages {
        runtime_deepseek_anthropic_messages_upstream_url(base_url)
    } else {
        runtime_deepseek_upstream_url(base_url, mount_path, path_and_query)
    }
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

fn runtime_deepseek_log_model_fallback(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    api_key_label: &str,
    from_model: &str,
    to_model: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &context.shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_model_fallback",
            [
                runtime_proxy_log_field("request", context.request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::DeepSeek),
                ),
                runtime_proxy_log_field("auth", api_key_label),
                runtime_proxy_log_field("from_model", from_model),
                runtime_proxy_log_field("to_model", to_model),
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
    response: RuntimeLocalRewriteLiveResponse,
    pending_request: RuntimeDeepSeekPendingRequest,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Live(
            response.with_chat_compatible_request(pending_request),
        ),
        gemini_context: None,
        copilot_context: None,
    }
}

fn runtime_deepseek_uses_native_web_search(
    mode: super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode,
    body: &[u8],
) -> bool {
    matches!(
        mode,
        super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode::Auto
            | super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode::Anthropic
    ) && serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("web_search_options").cloned())
        .is_some()
}

fn runtime_deepseek_auto_chat_fallback_body(
    body: &[u8],
    mode: super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode,
) -> Option<Vec<u8>> {
    matches!(
        mode,
        super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode::Auto
    )
    .then(|| runtime_provider_chat_request_body_without_web_search_options(body))
    .flatten()
}

fn runtime_deepseek_native_translation_fallback_is_safe(
    result: &prodex_provider_core::ProviderTransformResult,
) -> bool {
    let ProviderTransformLoss::Rejected { reason } = &result.loss else {
        return false;
    };
    reason.starts_with("Anthropic Messages does not translate chat field ")
        || reason.starts_with("Anthropic Messages does not translate web_search_options field ")
        || reason.starts_with("Anthropic web search ")
}

fn runtime_deepseek_log_native_search_fallback(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    model: &str,
) {
    runtime_proxy_log(
        &context.shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_web_search_options_fallback",
            [
                runtime_proxy_log_field("request", context.request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::DeepSeek),
                ),
                runtime_proxy_log_field("model", model),
                runtime_proxy_log_field("route", "chat"),
                runtime_proxy_log_field("degradation", "web_search_unavailable"),
                runtime_proxy_log_field("phase", "precommit"),
            ],
        ),
    );
}

fn runtime_deepseek_native_translation_incompatible() -> RuntimeLocalRewriteUpstreamResult {
    runtime_deepseek_buffered_result(
        super::super::local_rewrite_upstream::runtime_local_rewrite_json_parts(
            400,
            serde_json::json!({
                "error": {
                    "message": "request is incompatible with DeepSeek native Anthropic web-search translation",
                    "type": "invalid_request_error",
                    "code": "invalid_request",
                }
            }),
        ),
    )
}

#[cfg(test)]
#[path = "local_rewrite_deepseek_send_tests.rs"]
mod tests;
