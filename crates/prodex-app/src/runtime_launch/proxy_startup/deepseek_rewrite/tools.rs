use super::super::provider_tools::{
    runtime_provider_chat_tool_choice_from_responses_request,
    runtime_provider_chat_tools_from_responses_request,
};

pub(super) fn runtime_deepseek_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    runtime_provider_chat_tools_from_responses_request(value)
}

pub(super) fn runtime_deepseek_tool_choice_from_responses_request(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Option<serde_json::Value> {
    runtime_provider_chat_tool_choice_from_responses_request(value, thinking_enabled)
}
