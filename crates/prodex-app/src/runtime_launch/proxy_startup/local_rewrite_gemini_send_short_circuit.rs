use super::super::super::gemini_rewrite::RuntimeGeminiTranslatedRequest;
use super::super::super::gemini_sse::{
    RuntimeGeminiGenerateSseReader, RuntimeGeminiSseReaderConfig,
    runtime_gemini_forced_command_output,
};
use super::super::super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
};
use super::super::local_rewrite_gemini_oauth_pool::{
    RuntimeGeminiSelectedAuth, runtime_gemini_binding_recorder,
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, runtime_proxy_log};
use anyhow::{Context, Result};
use prodex_provider_core::gemini_provider_core_exact_output_sse_stream;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::Read;

pub(super) fn runtime_gemini_exact_output_short_circuit(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    selected: &RuntimeGeminiSelectedAuth,
    translated: &RuntimeGeminiTranslatedRequest,
) -> Result<Option<RuntimeLocalRewriteUpstreamResult>> {
    if !translated.stream {
        return Ok(None);
    }
    let Some(output) = runtime_gemini_forced_command_output(&translated.messages) else {
        return Ok(None);
    };
    let binding_recorder = shared
        .gemini_oauth_pool
        .as_ref()
        .map(|pool| runtime_gemini_binding_recorder(pool, selected.profile_name.clone(), None));
    let fake_stream =
        gemini_provider_core_exact_output_sse_stream(request_id, &translated.model, &output);
    let mut reader = RuntimeGeminiGenerateSseReader::new_with_config(
        std::io::Cursor::new(fake_stream.into_bytes()),
        request_id,
        translated.messages.clone(),
        shared.gemini_conversations.clone(),
        binding_recorder,
        RuntimeGeminiSseReaderConfig {
            observer: None,
            harness_mode: shared.resolved_harness.effective,
            harness_model: Some(translated.model.clone()),
            gemini: shared.runtime_shared.runtime_config.gemini.clone(),
        },
    );
    let mut body = String::new();
    reader
        .read_to_string(&mut body)
        .context("failed to build Gemini exact-output SSE response")?;
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_exact_output_short_circuit",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                runtime_proxy_log_field("model", translated.model.as_str()),
                runtime_proxy_log_field("bytes", body.len().to_string()),
            ],
        ),
    );
    let mut headers = vec![
        (
            "content-type".to_string(),
            b"text/event-stream; charset=utf-8".to_vec(),
        ),
        ("x-reasoning-included".to_string(), b"true".to_vec()),
    ];
    if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
        headers.extend(
            pool.quota_headers_for_profile(&selected.profile_name)
                .into_iter()
                .map(|(name, value)| (name, value.into_bytes())),
        );
    }
    Ok(Some(RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(
            RuntimeHeapTrimmedBufferedResponseParts {
                status: 200,
                headers,
                body: body.into_bytes().into(),
            },
        ),
        gemini_context: None,
        copilot_context: None,
    }))
}

pub(super) fn runtime_gemini_thinking_budget_tokens(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<u64> {
    match provider {
        RuntimeLocalRewriteProviderOptions::Gemini {
            thinking_budget_tokens,
            ..
        } => *thinking_budget_tokens,
        _ => None,
    }
}
