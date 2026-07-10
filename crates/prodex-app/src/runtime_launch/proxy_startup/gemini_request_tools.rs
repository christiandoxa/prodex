use super::super::gemini_request_policy::RuntimeGeminiPolicyCompat;
use prodex_provider_core::gemini_provider_core_tools_from_requests;

pub(super) fn runtime_gemini_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
) -> Option<serde_json::Value> {
    let policy = RuntimeGeminiPolicyCompat::from_request_and_files(original);
    gemini_provider_core_tools_from_requests(original, chat, model, |declarations| {
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
