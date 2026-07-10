//! Chat-compatible assistant tool-call item shaping.

use super::rtk::gemini_rtk_wrapped_tool_arguments;
use serde_json::{Value, json};

pub(crate) fn gemini_chat_assistant_tool_call_item_with_call_id(
    part: &Value,
    function_call: &Value,
    call_id_override: Option<&str>,
) -> Value {
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
    let mut item = json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": flat_name,
            "arguments": gemini_rtk_wrapped_tool_arguments(flat_name, &args),
        },
    });
    if let Some(signature) = part
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .or_else(|| {
            function_call
                .get("thoughtSignature")
                .and_then(Value::as_str)
        })
    {
        item["gemini_thought_signature"] = Value::String(signature.to_string());
    }
    item
}
