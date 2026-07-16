use super::{
    RuntimeGeminiConfig, chat_message_text, runtime_gemini_function_call_part,
    runtime_gemini_function_response_from_tool_message,
};
use std::collections::BTreeMap;

pub(super) fn runtime_gemini_assistant_content(
    message: &serde_json::Value,
    tool_names_by_call_id: &mut BTreeMap<String, String>,
) -> Option<serde_json::Value> {
    let mut parts = Vec::new();
    if let Some(content) = chat_message_text(message).filter(|text| !text.is_empty()) {
        parts.push(serde_json::json!({ "text": content }));
    }
    if let Some(tool_calls) = message
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
    {
        parts.extend(tool_calls.iter().filter_map(|tool_call| {
            runtime_gemini_assistant_tool_call_part(tool_call, tool_names_by_call_id)
        }));
    }
    if let Some(native_parts) = message
        .get("gemini_native_parts")
        .and_then(serde_json::Value::as_array)
    {
        parts.extend(native_parts.iter().cloned());
    }
    (!parts.is_empty()).then(|| {
        serde_json::json!({
            "role": "model",
            "parts": parts,
        })
    })
}

fn runtime_gemini_assistant_tool_call_part(
    tool_call: &serde_json::Value,
    tool_names_by_call_id: &mut BTreeMap<String, String>,
) -> Option<serde_json::Value> {
    let function = tool_call.get("function")?;
    let call_id = tool_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
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
        "functionCall": runtime_gemini_function_call_part(call_id, name, args),
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

pub(super) fn runtime_gemini_tool_response_content(
    messages: &[serde_json::Value],
    index: &mut usize,
    tool_names_by_call_id: &BTreeMap<String, String>,
    persist_tool_output: bool,
    config: &RuntimeGeminiConfig,
) -> serde_json::Value {
    let mut parts = Vec::new();
    while *index < messages.len()
        && messages[*index]
            .get("role")
            .and_then(serde_json::Value::as_str)
            == Some("tool")
    {
        parts.push(serde_json::json!({
            "functionResponse": runtime_gemini_function_response_from_tool_message(
                &messages[*index],
                tool_names_by_call_id,
                persist_tool_output,
                config,
            ),
        }));
        *index += 1;
    }
    serde_json::json!({
        "role": "user",
        "parts": parts,
    })
}
