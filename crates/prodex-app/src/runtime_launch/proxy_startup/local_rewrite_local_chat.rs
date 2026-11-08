//! Host transport for the local-ai launcher's Chat Completions models.
//! Reuse the shared function/custom/namespace and continuation adapters; never
//! drop coding tools or change ordinary Responses/cloud-provider routing.
use super::chat_compatible_rewrite::{
    RuntimeDeepSeekRewriteOptions, runtime_provider_chat_compatible_request_body,
};
use super::deepseek_rewrite::RuntimeDeepSeekPendingRequest;
use super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::RuntimeProviderBridgeKind;
use crate::RuntimeProxyRequest;
use anyhow::{Context, Result};

pub(super) fn send_local_chat_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
    model: &str,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let value: serde_json::Value = serde_json::from_slice(body)?;
    let catalog = prodex_provider_core::render_local_runtime_model_catalog(model, 0, 0)?;
    let effort = value
        .pointer("/reasoning/effort")
        .and_then(serde_json::Value::as_str)
        .or_else(|| catalog["default_reasoning_level"].as_str())
        .context("local model has no reasoning effort")?;
    let supported = catalog["supported_reasoning_levels"]
        .as_array()
        .context("local model has no supported reasoning levels")?;
    if !supported
        .iter()
        .any(|entry| entry["effort"].as_str() == Some(effort))
    {
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Buffered(crate::RuntimeHeapTrimmedBufferedResponseParts {
                status: 400,
                headers: vec![("content-type".into(), b"application/json".to_vec())],
                body: serde_json::to_vec(&serde_json::json!({"error": {
                    "type": "invalid_request_error", "code": "unsupported_reasoning_effort",
                    "message": "Local model reasoning effort is unsupported; select low, medium, or xhigh."
                }}))?.into(),
            }),
            gemini_context: None, copilot_context: None,
        });
    }
    let conversations = shared.deepseek_conversations_for_request(request);
    let mut local_request = value.clone();
    let parallel =
        prodex_provider_core::provider_core_local_chat_parallel_control(&mut local_request)
            .map_err(anyhow::Error::msg)?;
    let translated = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&local_request)?,
        &conversations,
        RuntimeProviderBridgeKind::OpenAiResponses,
        model,
        false,
        RuntimeDeepSeekRewriteOptions::default(),
    )?;
    let mut chat: serde_json::Value = serde_json::from_slice(&translated.body)?;
    chat["reasoning_effort"] = effort.into();
    if let Some(enabled) = parallel {
        chat["parallel_tool_calls"] = enabled.into();
    }
    let messages = chat["messages"]
        .as_array_mut()
        .context("local chat request has no messages")?;
    prodex_provider_core::provider_core_local_chat_system_messages(messages)
        .map_err(anyhow::Error::msg)?;
    let upstream_url = format!(
        "{}/chat/completions",
        shared.upstream_base_url.trim_end_matches('/')
    );
    let response = send_runtime_local_rewrite_prepared_request(
        request_id,
        request,
        shared,
        &upstream_url,
        serde_json::to_vec(&chat)?,
        // Local models must never inherit account credentials from another provider.
        RuntimeLocalRewritePreparedAuth::LocalChat,
    )?;
    Ok(RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Live(
            RuntimeLocalRewriteLiveResponse::new(response).with_chat_compatible_request(
                RuntimeDeepSeekPendingRequest {
                    messages: translated.messages,
                    response_metadata: translated.response_metadata,
                },
            ),
        ),
        gemini_context: None,
        copilot_context: None,
    })
}

pub(super) fn send_openai_or_local_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    endpoint: prodex_provider_core::ProviderEndpoint,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys, local } =
        shared.provider.as_ref()
    else {
        anyhow::bail!("application provider dispatch does not match configured adapter")
    };
    if *local && endpoint == prodex_provider_core::ProviderEndpoint::Responses {
        let model =
            super::provider_bridge::runtime_provider_model_from_body(&body).unwrap_or_default();
        if prodex_mojo_core::json::local_model_uses_chat_completions(&model)
            .map_err(|error| anyhow::anyhow!("local model protocol selection failed: {error:?}"))?
        {
            return send_local_chat_request(request_id, request, shared, &body, &model);
        }
    }
    super::local_rewrite_upstream::send_runtime_openai_upstream_request(
        request_id, request, shared, body, api_keys, endpoint,
    )
}
