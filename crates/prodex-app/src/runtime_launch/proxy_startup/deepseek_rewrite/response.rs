use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_label, runtime_provider_log_response_conformance,
    runtime_provider_response_conformance_result,
};
use super::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekPendingMessages, RuntimeDeepSeekPendingRequest,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result};
use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
use prodex_provider_core::{
    deepseek_provider_core_chat_assistant_messages_from_response_value,
    deepseek_provider_core_merge_response_metadata,
    deepseek_provider_core_normalize_assistant_tool_call_content,
    provider_core_chat_compatible_responses_value_from_chat_value,
    provider_core_rewritten_json_value,
};
use std::io::Read;

#[allow(clippy::too_many_arguments)]
pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_chat_buffered_response_parts(
    provider_kind: RuntimeProviderBridgeKind,
    status: u16,
    mut response: reqwest::blocking::Response,
    request_id: u64,
    conversation_messages: Vec<serde_json::Value>,
    response_metadata: Option<serde_json::Value>,
    conversations: &RuntimeDeepSeekConversationStore,
    runtime_shared: &crate::RuntimeRotationProxyShared,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let provider_label = provider_kind.chat_compatible_adapter_label();
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .with_context(|| format!("failed to read {provider_label} chat response body"))?;
    let value: serde_json::Value = serde_json::from_slice(&body)
        .with_context(|| format!("failed to parse {provider_label} chat response JSON"))?;
    let translated = runtime_provider_response_conformance_result(provider_kind, status, &body);
    if let Some(result) = translated.as_ref() {
        runtime_provider_log_response_conformance(
            runtime_shared,
            request_id,
            provider_kind,
            result,
        );
    }
    let mut response = translated
        .as_ref()
        .and_then(|result| provider_core_rewritten_json_value(Some(result)))
        .unwrap_or_else(|| {
            provider_core_chat_compatible_responses_value_from_chat_value(
                &value,
                request_id,
                runtime_provider_label(provider_kind),
                provider_kind.chat_compatible_adapter_label(),
                SUPER_DEEPSEEK_DEFAULT_MODEL,
                "resp_deepseek",
            )
        });
    runtime_deepseek_merge_response_metadata(&mut response, response_metadata);
    if response.get("status").and_then(serde_json::Value::as_str) != Some("failed")
        && let Some(response_id) = response.get("id").and_then(serde_json::Value::as_str)
    {
        runtime_deepseek_store_conversation(
            conversations,
            response_id,
            conversation_messages,
            runtime_deepseek_chat_assistant_messages_from_response_value(&value),
        );
    }
    let body = serde_json::to_vec(&response).context("failed to serialize Responses JSON")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_take_pending_messages(
    pending: &RuntimeDeepSeekPendingMessages,
    request_id: u64,
) -> RuntimeDeepSeekPendingRequest {
    pending
        .lock()
        .ok()
        .and_then(|mut pending| pending.remove(&request_id))
        .unwrap_or_default()
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_store_conversation(
    conversations: &RuntimeDeepSeekConversationStore,
    response_id: &str,
    mut messages: Vec<serde_json::Value>,
    assistant_messages: Vec<serde_json::Value>,
) {
    if response_id.trim().is_empty() {
        return;
    }
    messages.extend(
        assistant_messages
            .into_iter()
            .map(deepseek_provider_core_normalize_assistant_tool_call_content),
    );
    if let Ok(mut conversations) = conversations.lock() {
        conversations.insert(response_id.to_string(), messages);
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_merge_response_metadata(
    response: &mut serde_json::Value,
    metadata: Option<serde_json::Value>,
) {
    deepseek_provider_core_merge_response_metadata(response, metadata);
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_chat_assistant_messages_from_response_value(
    value: &serde_json::Value,
) -> Vec<serde_json::Value> {
    deepseek_provider_core_chat_assistant_messages_from_response_value(value)
}
