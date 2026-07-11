use super::invoke::runtime_kiro_prompt_turn;
use super::{RuntimeKiroProfileAuth, runtime_kiro_prompt_from_messages};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use crate::runtime_kiro_acp::runtime_kiro_acp_responses_value_from_prompt_turn;
use crate::runtime_launch::proxy_startup::chat_compatible_rewrite::{
    RuntimeChatCompatibleConversationStore, runtime_provider_chat_compatible_request_body,
};
use crate::runtime_launch::proxy_startup::local_rewrite_gemini_compact::{
    runtime_gemini_compact_response_parts, runtime_gemini_local_compact_response_parts,
};
use anyhow::{Context, Result};
use prodex_provider_core::{
    kiro_provider_core_compact_summary_from_response,
    kiro_provider_core_semantic_compact_request_body,
};
use serde_json::Value;
use std::sync::Arc;
use tokio::runtime::Runtime as TokioRuntime;

pub(crate) fn runtime_kiro_compact_response_parts(
    request_id: u64,
    body: &[u8],
    async_runtime: &Arc<TokioRuntime>,
    auth: &RuntimeKiroProfileAuth,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    match runtime_kiro_semantic_compact_summary(request_id, body, async_runtime, auth) {
        Ok(summary) => runtime_gemini_compact_response_parts(&summary),
        Err(_) => runtime_gemini_local_compact_response_parts(body),
    }
}

pub(super) fn runtime_kiro_semantic_compact_summary(
    request_id: u64,
    body: &[u8],
    async_runtime: &Arc<TokioRuntime>,
    auth: &RuntimeKiroProfileAuth,
) -> Result<String> {
    let rewritten =
        kiro_provider_core_semantic_compact_request_body(body).map_err(anyhow::Error::msg)?;

    let translated = runtime_provider_chat_compatible_request_body(
        &rewritten,
        &RuntimeChatCompatibleConversationStore::default(),
        super::super::provider_bridge::RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        Default::default(),
    )?;
    let prompt = runtime_kiro_prompt_from_messages(&translated.messages);
    runtime_kiro_prompt_turn(auth, "compact", &prompt, async_runtime).and_then(|turn| {
        let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
        runtime_kiro_compact_summary_from_response(&response)
    })
}

fn runtime_kiro_compact_summary_from_response(response: &Value) -> Result<String> {
    kiro_provider_core_compact_summary_from_response(response).map_err(anyhow::Error::msg)
}
