use super::super::gemini_request_policy::RuntimeGeminiPolicyCompat;
use crate::runtime_launch::proxy_startup::provider_tools;
use prodex_provider_core::gemini_provider_core_tools_from_requests;
use std::collections::BTreeSet;

pub(super) fn runtime_gemini_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
) -> Option<serde_json::Value> {
    let policy = RuntimeGeminiPolicyCompat::from_request_and_files(original);
    let mut chat = chat.clone();
    if let Some(mut tools) =
        provider_tools::runtime_provider_chat_tools_from_responses_request(original)
    {
        let mut seen = BTreeSet::new();
        tools.retain(|tool| {
            tool.get("function")
                .and_then(|function| function.get("name"))
                .and_then(serde_json::Value::as_str)
                .is_none_or(|name| seen.insert(name.to_string()))
        });
        chat["tools"] = serde_json::Value::Array(tools);
    }
    gemini_provider_core_tools_from_requests(original, &chat, model, |declarations| {
        policy.filter_function_declarations(declarations);
    })
}

pub(in super::super::super) fn runtime_gemini_blocked_tool_call_message(
    name: &str,
    args: &serde_json::Value,
) -> Option<String> {
    RuntimeGeminiPolicyCompat::from_request_and_files(&serde_json::Value::Null)
        .blocked_tool_call_message(name, args)
}
