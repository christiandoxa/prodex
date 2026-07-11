use super::super::provider_tools::{
    runtime_provider_chat_tool_choice_from_responses_request,
    runtime_provider_chat_tools_from_responses_request,
    runtime_provider_chat_web_search_options_from_responses_request,
};
use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;
use prodex_provider_core::deepseek_provider_core_validate_web_search_tool_context_size;

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    runtime_provider_chat_tools_from_responses_request(value)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_tool_choice_from_responses_request(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Option<serde_json::Value> {
    runtime_provider_chat_tool_choice_from_responses_request(value, thinking_enabled)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_web_search_options_from_responses_request(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Option<serde_json::Value>> {
    if let Some(options) = value.get("web_search_options") {
        return Ok(Some(options.clone()));
    }
    deepseek_provider_core_validate_web_search_tool_context_size(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)?;
    Ok(runtime_provider_chat_web_search_options_from_responses_request(value))
}
