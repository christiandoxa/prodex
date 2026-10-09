//! Chat-compatible assistant tool-call item shaping.

use super::{gemini_response_tool_call_signature, rtk::gemini_rtk_wrapped_tool_arguments};
use serde_json::{Value, json};

pub(crate) fn gemini_chat_assistant_tool_call_with_call_id(
    part: &Value,
    function_call: &Value,
    call_id_override: Option<&str>,
) -> crate::GeminiProviderCoreStreamToolCall {
    let call_id = function_call
        .get("id")
        .and_then(Value::as_str)
        .or(call_id_override)
        .unwrap_or("call_1");
    let flat_name = function_call
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call");
    let args = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
    let args = gemini_rtk_wrapped_tool_arguments(flat_name, &args);
    let signature = gemini_response_tool_call_signature(part, function_call);

    crate::GeminiProviderCoreStreamToolCall {
        call_id: call_id.to_string(),
        name: flat_name.to_string(),
        arguments: args,
        thought_signature: signature,
    }
}

pub(crate) fn gemini_chat_assistant_tool_call_item_with_call_id(
    part: &Value,
    function_call: &Value,
    call_id_override: Option<&str>,
) -> Value {
    let tool_call =
        gemini_chat_assistant_tool_call_with_call_id(part, function_call, call_id_override);
    crate::translators::gemini::gemini_provider_core_stream_chat_tool_call_item(&tool_call)
}
