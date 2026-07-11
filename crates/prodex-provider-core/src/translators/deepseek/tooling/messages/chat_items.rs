//! Generic DeepSeek chat message content and tool-call shaping.

use serde_json::{Value, json};

pub(super) fn deepseek_message_content_text(value: Option<&Value>) -> Option<String> {
    let parts = value?.as_array()?;
    let text = parts
        .iter()
        .filter_map(deepseek_message_content_part_text)
        .collect::<Vec<_>>()
        .join("\n");
    Some(text)
}

fn deepseek_message_content_part_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(object) => object
            .get("text")
            .and_then(Value::as_str)
            .or_else(|| object.get("input_text").and_then(Value::as_str))
            .or_else(|| object.get("output_text").and_then(Value::as_str))
            .map(str::to_string),
        _ => None,
    }
}

pub(super) fn deepseek_message_tool_calls(item: &Value) -> Option<Vec<Value>> {
    let tool_calls = item.get("tool_calls")?.as_array()?;
    let translated = tool_calls
        .iter()
        .filter_map(deepseek_message_tool_call)
        .collect::<Vec<_>>();
    (!translated.is_empty()).then_some(translated)
}

fn deepseek_message_tool_call(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let function = object.get("function").and_then(Value::as_object)?;
    let name = function.get("name").and_then(Value::as_str)?;
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            function
                .get("arguments")
                .map(|arguments| arguments.to_string())
        })
        .unwrap_or_else(|| "{}".to_string());
    let mut tool_call = json!({
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(call_id) = object.get("id").and_then(Value::as_str)
        && let Some(tool_call_object) = tool_call.as_object_mut()
    {
        tool_call_object.insert("id".to_string(), Value::String(call_id.to_string()));
    }
    Some(tool_call)
}
