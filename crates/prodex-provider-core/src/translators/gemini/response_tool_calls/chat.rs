//! Chat-compatible assistant tool-call item shaping.

use super::rtk::gemini_rtk_wrapped_tool_arguments;
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
    let signature = part
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .or_else(|| {
            function_call
                .get("thoughtSignature")
                .and_then(Value::as_str)
        });

    crate::GeminiProviderCoreStreamToolCall {
        call_id: call_id.to_string(),
        name: flat_name.to_string(),
        arguments: args,
        thought_signature: signature.map(str::to_string),
    }
}

pub(crate) fn gemini_chat_assistant_tool_call_item_with_call_id(
    part: &Value,
    function_call: &Value,
    call_id_override: Option<&str>,
) -> Value {
    let tool_call =
        gemini_chat_assistant_tool_call_with_call_id(part, function_call, call_id_override);
    #[cfg(feature = "mojo")]
    {
        crate::translators::gemini::gemini_provider_core_stream_chat_tool_call_item(&tool_call)
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut item = json!({
            "id": tool_call.call_id,
            "type": "function",
            "function": {
                "name": tool_call.name,
                "arguments": tool_call.arguments,
            },
        });
        if let Some(signature) = tool_call.thought_signature {
            item["gemini_thought_signature"] = Value::String(signature);
        }
        item
    }
}
