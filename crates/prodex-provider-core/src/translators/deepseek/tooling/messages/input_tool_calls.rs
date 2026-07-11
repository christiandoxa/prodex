//! Responses input tool-call item shaping for DeepSeek chat messages.

use super::chat_items::deepseek_message_content_text;
use super::thought_signature::deepseek_tool_call_thought_signature;
use serde_json::{Value, json};

pub(super) fn deepseek_input_function_call_message(item: &Value) -> Option<Value> {
    let call_id = item
        .get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let name = item
        .get("name")
        .or_else(|| item.get("tool_name"))
        .or_else(|| {
            item.get("function")
                .and_then(|function| function.get("name"))
        })
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())?;
    let arguments = item
        .get("arguments")
        .or_else(|| item.get("input"))
        .or_else(|| {
            item.get("function")
                .and_then(|function| function.get("arguments"))
        })
        .map(deepseek_stringified_arguments)
        .unwrap_or_else(|| "{}".to_string());
    let mut tool_call = json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(signature) = deepseek_tool_call_thought_signature(item)
        && let Some(tool_call_object) = tool_call.as_object_mut()
    {
        tool_call_object.insert(
            "gemini_thought_signature".to_string(),
            Value::String(signature),
        );
    }
    Some(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [tool_call],
    }))
}

pub(super) fn deepseek_input_custom_tool_call_message(item: &Value) -> Option<Value> {
    let call_id = item
        .get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let name = item
        .get("name")
        .or_else(|| item.get("tool_name"))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())?;
    let input = item
        .get("input")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| deepseek_message_content_text(item.get("input")))
        .or_else(|| item.get("input").map(deepseek_stringified_arguments))
        .unwrap_or_default();
    let arguments = serde_json::to_string(&json!({ "input": input })).ok()?;
    let mut tool_call = json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(signature) = deepseek_tool_call_thought_signature(item)
        && let Some(tool_call_object) = tool_call.as_object_mut()
    {
        tool_call_object.insert(
            "gemini_thought_signature".to_string(),
            Value::String(signature),
        );
    }
    Some(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [tool_call],
    }))
}

pub(super) fn deepseek_input_mcp_call_messages(item: &Value) -> Option<Vec<Value>> {
    let assistant = deepseek_input_function_call_message(item)?;
    let mut messages = vec![assistant];
    if deepseek_mcp_call_has_result(item) {
        let call_id = item
            .get("call_id")
            .or_else(|| item.get("tool_call_id"))
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("call_1");
        messages.push(json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": deepseek_tool_output_value(item),
        }));
    }
    Some(messages)
}

fn deepseek_stringified_arguments(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

pub(super) fn deepseek_tool_output_value(item: &Value) -> Value {
    item.get("output")
        .or_else(|| item.get("content"))
        .or_else(|| item.get("result"))
        .or_else(|| item.get("error"))
        .cloned()
        .unwrap_or_else(|| Value::String(String::new()))
}

fn deepseek_mcp_call_has_result(item: &Value) -> bool {
    item.get("output").is_some()
        || item.get("content").is_some()
        || item.get("result").is_some()
        || item.get("error").is_some()
}
