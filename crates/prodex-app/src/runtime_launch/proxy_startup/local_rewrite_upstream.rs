use super::deepseek_rewrite::runtime_chat_compatible_request_body;
use super::local_rewrite::{RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_copilot::{
    RuntimeCopilotRequestContext, send_runtime_copilot_upstream_request,
};
use super::local_rewrite_deepseek::send_runtime_deepseek_upstream_request;
use super::local_rewrite_gemini::{
    RuntimeGeminiRequestContext, send_runtime_gemini_upstream_request,
};
use super::local_rewrite_model_memory::runtime_local_rewrite_model_selection;
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_local_rewrite_anthropic_auth_attempts,
    runtime_local_rewrite_api_key_attempts, runtime_local_rewrite_upstream_url,
    runtime_openai_standard_provider_upstream_url, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_error_class, runtime_provider_model_fallback_chain,
    runtime_provider_request_body_with_model, runtime_provider_should_retry_with_next_model,
    runtime_provider_should_rotate_auth_after_response,
};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, RuntimeRouteKind,
    prepare_runtime_smart_context_http_body, runtime_proxy_log, runtime_proxy_request_lane,
};
use anyhow::Result;
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const RUNTIME_LOCAL_EMBEDDING_DIMENSIONS: usize = 1536;

pub(super) struct RuntimeLocalRewriteUpstreamResult {
    pub(super) response: RuntimeLocalRewriteUpstreamResponse,
    pub(super) gemini_context: Option<RuntimeGeminiRequestContext>,
    pub(super) copilot_context: Option<RuntimeCopilotRequestContext>,
}

pub(super) enum RuntimeLocalRewriteUpstreamResponse {
    Live(RuntimeLocalRewriteLiveResponse),
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
}

pub(super) struct RuntimeLocalRewriteLiveResponse {
    pub(super) response: reqwest::blocking::Response,
    pub(super) prefix: Vec<u8>,
}

impl RuntimeLocalRewriteLiveResponse {
    pub(super) fn new(response: reqwest::blocking::Response) -> Self {
        Self {
            response,
            prefix: Vec::new(),
        }
    }

    pub(super) fn with_prefix(response: reqwest::blocking::Response, prefix: Vec<u8>) -> Self {
        Self { response, prefix }
    }
}

pub(super) fn send_runtime_local_rewrite_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let route_kind = runtime_local_rewrite_route_kind(&request.path_and_query);
    let body = prepare_runtime_smart_context_http_body(
        request_id,
        request,
        &shared.runtime_shared,
        route_kind,
    )
    .into_owned();
    match &shared.provider {
        RuntimeLocalRewriteProviderOptions::Anthropic { auth } => {
            let auth_attempts = runtime_local_rewrite_anthropic_auth_attempts(shared, auth);
            if auth_attempts.is_empty() {
                anyhow::bail!("Anthropic provider has no auth configured");
            }
            let auth_attempt_count = auth_attempts.len();
            if path_without_query(&request.path_and_query).ends_with("/responses") {
                let model_selection = runtime_local_rewrite_model_selection(
                    shared,
                    RuntimeProviderBridgeKind::Anthropic,
                    request,
                    &body,
                    prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
                );
                let model_chain = runtime_provider_model_fallback_chain(
                    RuntimeProviderBridgeKind::Anthropic,
                    &model_selection.model,
                );
                let upstream_url = runtime_openai_standard_provider_upstream_url(
                    RuntimeProviderBridgeKind::Anthropic,
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (auth_index, selected_auth) in auth_attempts.into_iter().enumerate() {
                    for (model_index, model) in model_chain.iter().enumerate() {
                        let model_body =
                            runtime_provider_request_body_with_model(&model_selection.body, model);
                        let translated = runtime_chat_compatible_request_body(
                            &model_body,
                            &shared.deepseek_conversations,
                            RuntimeProviderBridgeKind::Anthropic,
                            prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
                            false,
                        )?;
                        if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                            pending.insert(request_id, translated.messages);
                        }
                        let send_result =
                            send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                                RuntimeLocalRewriteSearchFallbackRequest {
                                    request_id,
                                    request,
                                    shared,
                                    upstream_url: &upstream_url,
                                    body: translated.body,
                                    provider_kind: RuntimeProviderBridgeKind::Anthropic,
                                    auth_label: selected_auth.label.as_str(),
                                    model,
                                    auth_factory: || RuntimeLocalRewritePreparedAuth::Anthropic {
                                        auth: &selected_auth.auth,
                                    },
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
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
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
                        if auth_index + 1 < auth_attempt_count
                            && runtime_provider_should_rotate_auth_after_response(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_auth_rotate",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
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
                    if auth_index + 1 < auth_attempt_count {
                        continue;
                    }
                }
                anyhow::bail!("no Anthropic model attempts were available");
            } else {
                let upstream_url = runtime_local_rewrite_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (auth_index, selected_auth) in auth_attempts.into_iter().enumerate() {
                    let response = send_runtime_local_rewrite_prepared_request(
                        request_id,
                        request,
                        shared,
                        &upstream_url,
                        body.clone(),
                        RuntimeLocalRewritePreparedAuth::Anthropic {
                            auth: &selected_auth.auth,
                        },
                    )?;
                    let status = response.status().as_u16();
                    if status >= 400 {
                        let parts =
                            runtime_local_rewrite_buffered_response_from_response(response)?;
                        let class = runtime_provider_error_class(
                            RuntimeProviderBridgeKind::Anthropic,
                            status,
                            &parts.body,
                        );
                        if auth_index + 1 < auth_attempt_count
                            && runtime_provider_should_rotate_auth_after_response(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_auth_rotate",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field("class", format!("{class:?}")),
                                    ],
                                ),
                            );
                            continue;
                        }
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                            gemini_context: None,
                            copilot_context: None,
                        });
                    }
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }
                anyhow::bail!("no Anthropic auth attempts were available")
            }
        }
        RuntimeLocalRewriteProviderOptions::Copilot { auth } => {
            send_runtime_copilot_upstream_request(request_id, request, shared, body, auth)
        }
        RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys } => {
            let upstream_url = runtime_local_rewrite_upstream_url(
                &shared.upstream_base_url,
                &shared.mount_path,
                &request.path_and_query,
            );
            let body = if path_without_query(&request.path_and_query).ends_with("/responses") {
                runtime_local_rewrite_model_selection(
                    shared,
                    RuntimeProviderBridgeKind::OpenAiResponses,
                    request,
                    &body,
                    "",
                )
                .body
            } else {
                body
            };
            let auth_attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys);
            let selected_api_key = auth_attempts.first().map(|(_, api_key)| *api_key);
            let response = send_runtime_local_rewrite_prepared_request(
                request_id,
                request,
                shared,
                &upstream_url,
                body,
                RuntimeLocalRewritePreparedAuth::OpenAiResponses {
                    api_key: selected_api_key,
                },
            )?;
            Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Live(
                    RuntimeLocalRewriteLiveResponse::new(response),
                ),
                gemini_context: None,
                copilot_context: None,
            })
        }
        RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly { embedding_model } => {
            let parts = if path_without_query(&request.path_and_query).ends_with("/embeddings") {
                runtime_local_embeddings_response_parts(request, embedding_model)
            } else {
                runtime_local_embeddings_only_rejection_parts()
            };
            Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            })
        }
        RuntimeLocalRewriteProviderOptions::DeepSeek { api_keys } => {
            send_runtime_deepseek_upstream_request(request_id, request, shared, body, api_keys)
        }
        RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } => {
            send_runtime_gemini_upstream_request(request_id, request, shared, body, auth)
        }
    }
}

fn runtime_local_embeddings_response_parts(
    request: &RuntimeProxyRequest,
    model: &str,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = match serde_json::from_slice::<Value>(&request.body)
        .map_err(|err| format!("invalid JSON body: {err}"))
        .and_then(|value| runtime_local_embedding_inputs(&value))
    {
        Ok(inputs) => {
            let prompt_tokens = inputs
                .iter()
                .map(|input| input.split_whitespace().count() as u64)
                .sum::<u64>();
            let data = inputs
                .iter()
                .enumerate()
                .map(|(index, input)| {
                    json!({
                        "object": "embedding",
                        "index": index,
                        "embedding": runtime_local_embedding_vector(input),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "object": "list",
                "data": data,
                "model": model,
                "usage": {
                    "prompt_tokens": prompt_tokens,
                    "total_tokens": prompt_tokens,
                },
            })
        }
        Err(message) => {
            return runtime_local_rewrite_json_parts(
                400,
                json!({
                    "error": {
                        "message": message,
                        "type": "invalid_request_error",
                        "code": "invalid_request",
                    }
                }),
            );
        }
    };
    runtime_local_rewrite_json_parts(200, body)
}

fn runtime_local_embedding_inputs(body: &Value) -> std::result::Result<Vec<String>, String> {
    let input = body
        .get("input")
        .ok_or_else(|| "missing required field: input".to_string())?;
    let inputs = match input {
        Value::String(value) => vec![value.clone()],
        Value::Array(values) => {
            if values.is_empty() {
                return Err("input array cannot be empty".to_string());
            }
            values
                .iter()
                .map(|value| match value {
                    Value::String(text) => Ok(text.clone()),
                    Value::Array(tokens) => Ok(tokens
                        .iter()
                        .filter_map(|token| token.as_i64())
                        .map(|token| token.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")),
                    _ => Err("input array must contain strings or token arrays".to_string()),
                })
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
        _ => return Err("input must be a string or array".to_string()),
    };
    if inputs.iter().any(|input| input.trim().is_empty()) {
        return Err("input cannot contain empty strings".to_string());
    }
    Ok(inputs)
}

fn runtime_local_embedding_vector(input: &str) -> Vec<f32> {
    let mut vector = vec![0.0_f32; RUNTIME_LOCAL_EMBEDDING_DIMENSIONS];
    let mut token_count = 0_u32;
    for token in input.split_whitespace() {
        runtime_local_embedding_accumulate_token(token, &mut vector);
        token_count += 1;
    }
    if token_count == 0 {
        runtime_local_embedding_accumulate_token(input, &mut vector);
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn runtime_local_embedding_accumulate_token(token: &str, vector: &mut [f32]) {
    let digest = Sha256::digest(token.as_bytes());
    let mut index_bytes = [0_u8; 8];
    index_bytes.copy_from_slice(&digest[0..8]);
    let index = (u64::from_le_bytes(index_bytes) as usize) % vector.len();
    let sign = if digest[8] & 1 == 0 { 1.0 } else { -1.0 };
    vector[index] += sign;
}

fn runtime_local_embeddings_only_rejection_parts() -> RuntimeHeapTrimmedBufferedResponseParts {
    runtime_local_rewrite_json_parts(
        503,
        json!({
            "error": {
                "message": "managed Mem0 memory is using Prodex local embeddings because no upstream API key was available; generation endpoints are disabled for this internal gateway",
                "type": "service_unavailable",
                "code": "local_embeddings_only",
            }
        }),
    )
}

fn runtime_local_rewrite_json_parts(
    status: u16,
    body: Value,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}

fn runtime_local_rewrite_route_kind(path_and_query: &str) -> RuntimeRouteKind {
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses") || path.ends_with("/chat/completions") {
        RuntimeRouteKind::Responses
    } else if path.ends_with("/responses/compact") {
        RuntimeRouteKind::Compact
    } else {
        runtime_proxy_request_lane(path_and_query, false)
    }
}
