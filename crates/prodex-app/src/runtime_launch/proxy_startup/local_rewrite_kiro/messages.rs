use serde_json::Value;

pub(super) fn runtime_kiro_prompt_from_messages(messages: &[serde_json::Value]) -> String {
    prodex_provider_core::kiro_provider_core_prompt_from_chat_messages(messages)
}

pub(super) fn runtime_kiro_responses_items_from_chat_message(message: &Value) -> Vec<Value> {
    prodex_provider_core::kiro_provider_core_responses_items_from_chat_message(message)
}

pub(super) fn runtime_kiro_tool_from_legacy_chat_function(function: &Value) -> Option<Value> {
    prodex_provider_core::kiro_provider_core_tool_from_legacy_chat_function(function)
}

pub(super) fn runtime_kiro_tool_choice_from_legacy_chat_function_call(
    function_call: &Value,
) -> Option<Value> {
    prodex_provider_core::kiro_provider_core_tool_choice_from_legacy_chat_function_call(
        function_call,
    )
}
