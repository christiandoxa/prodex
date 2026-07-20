use super::RuntimeDeepSeekConversationStore;
use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;
use prodex_provider_core::{
    deepseek_provider_core_first_function_call_output_call_id,
    deepseek_provider_core_messages_from_responses_request,
};
#[path = "input_items_messages.rs"]
mod input_items_messages;
use self::input_items_messages::runtime_deepseek_history_for_tool_call;

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_messages_from_responses_request(
    value: &serde_json::Value,
    conversations: &RuntimeDeepSeekConversationStore,
    gemini_compat: bool,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Option<Vec<serde_json::Value>>> {
    let provider_label = provider_kind.chat_compatible_adapter_label();
    let mut history_messages = Vec::new();
    let mut has_history = false;
    if let Some(previous_response_id) = value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        && let Some(history) = conversations.history(previous_response_id)
    {
        history_messages.extend(history);
        has_history = true;
    }
    if !has_history
        && let Some(call_id) = deepseek_provider_core_first_function_call_output_call_id(value)
        && let Some(history) = runtime_deepseek_history_for_tool_call(conversations, &call_id)
    {
        history_messages.extend(history);
    }
    let messages = deepseek_provider_core_messages_from_responses_request(
        value,
        &history_messages,
        gemini_compat,
        provider_label,
    )
    .map_err(anyhow::Error::msg)?;

    Ok(messages)
}
