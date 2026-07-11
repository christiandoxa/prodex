//! Gemini tool-call and tool-output bridge helpers.

mod command_response;
mod mask;

pub use self::command_response::gemini_provider_core_structured_command_tool_response;
pub use self::mask::{
    gemini_provider_core_mask_tool_response_for_history, gemini_provider_core_tool_output_preview,
    gemini_provider_core_tool_response_output_string,
};

use super::gemini_provider_core_chat_message_text;

pub fn gemini_provider_core_function_call_part(
    call_id: &str,
    name: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let mut call = serde_json::json!({
        "name": name,
        "args": args,
    });
    if !call_id.trim().is_empty() {
        call["id"] = serde_json::Value::String(call_id.to_string());
    }
    call
}

pub fn gemini_provider_core_function_call_part_from_tool_call(
    tool_call: &serde_json::Value,
    tool_names_by_call_id: &mut std::collections::BTreeMap<String, String>,
) -> Option<serde_json::Value> {
    let call_id = tool_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let function = tool_call.get("function")?;
    let name = function
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_call");
    if !call_id.is_empty() {
        tool_names_by_call_id.insert(call_id.to_string(), name.to_string());
    }
    let args = function
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .and_then(|args| serde_json::from_str::<serde_json::Value>(args).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let mut part = serde_json::json!({
        "functionCall": gemini_provider_core_function_call_part(call_id, name, args),
    });
    if let Some(signature) = tool_call
        .get("gemini_thought_signature")
        .or_else(|| function.get("gemini_thought_signature"))
        .or_else(|| {
            tool_call
                .get("extra_content")
                .and_then(|value| value.get("google"))
                .and_then(|value| value.get("thought_signature"))
        })
        .and_then(serde_json::Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
    {
        part["thoughtSignature"] = serde_json::Value::String(signature.to_string());
    }
    Some(part)
}

pub fn gemini_provider_core_function_response_part(
    call_id: &str,
    name: &str,
    response: serde_json::Value,
) -> serde_json::Value {
    let mut function_response = serde_json::json!({
        "name": name,
        "response": response,
    });
    if !call_id.trim().is_empty() {
        function_response["id"] = serde_json::Value::String(call_id.to_string());
    }
    function_response
}

pub fn gemini_provider_core_function_response_content_part(
    function_response: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "functionResponse": function_response,
    })
}

pub fn gemini_provider_core_function_response_from_tool_message(
    message: &serde_json::Value,
    tool_names_by_call_id: &std::collections::BTreeMap<String, String>,
    mask_tool_response_for_history: impl FnOnce(&str, &str, serde_json::Value) -> serde_json::Value,
) -> serde_json::Value {
    let call_id = message
        .get("tool_call_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let name = message
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| tool_names_by_call_id.get(call_id).cloned())
        .unwrap_or_else(|| "tool_call".to_string());
    let text = gemini_provider_core_chat_message_text(message).unwrap_or_default();
    let response = gemini_provider_core_structured_command_tool_response(&name, &text)
        .or_else(|| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_else(|| {
            serde_json::json!({
                "output": text
            })
        });
    let response = mask_tool_response_for_history(&name, call_id, response);
    gemini_provider_core_function_response_part(call_id, &name, response)
}
