//! Gemini contents shaping from Responses input items.

use serde_json::{Value, json};

use crate::gemini_bridge::gemini_provider_core_collect_media_parts;

use super::text::{gemini_contextual_user_instruction_text, gemini_message_text};

pub(crate) fn gemini_contains_local_media_path(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().any(gemini_contains_local_media_path),
        Value::Object(object) => {
            let ty = object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if matches!(
                ty,
                "input_image"
                    | "image_url"
                    | "input_file"
                    | "file"
                    | "media"
                    | "input_audio"
                    | "input_video"
            ) && object
                .get("path")
                .or_else(|| object.get("file_path"))
                .or_else(|| object.get("filePath"))
                .is_some()
            {
                return true;
            }
            object.values().any(gemini_contains_local_media_path)
        }
        _ => false,
    }
}

fn gemini_content_parts_from_message_content(message: &Value) -> Vec<Value> {
    let mut parts = Vec::new();
    match message {
        Value::String(text) => {
            if !text.trim().is_empty() {
                parts.push(json!({"text": text}));
            }
        }
        Value::Array(items) => {
            for item in items {
                parts.extend(gemini_content_parts_from_message_content(item));
            }
        }
        Value::Object(object) => {
            let mut media = Vec::new();
            let media_seed = Value::Object(object.clone());
            gemini_provider_core_collect_media_parts(&media_seed, &mut media, &mut |_, _| None);
            if !media.is_empty() {
                parts.extend(media);
            }
            if let Some(text) = object
                .get("text")
                .or_else(|| object.get("content"))
                .and_then(Value::as_str)
            {
                if !text.trim().is_empty() {
                    parts.push(json!({"text": text}));
                }
            }
        }
        _ => {}
    }

    if parts.is_empty() {
        if let Some(text) = gemini_message_text(message).filter(|text| !text.trim().is_empty()) {
            parts.push(json!({"text": text}));
        }
    }

    parts
}

pub(crate) fn gemini_contents_from_request(value: &Value) -> Vec<Value> {
    if let Some(input) = value.get("input") {
        return match input {
            Value::String(text) => vec![json!({"role":"user","parts":[{"text": text}]})],
            Value::Array(items) => gemini_contents_from_input_items(items),
            _ => vec![json!({"role":"user","parts":[{"text":""}]})],
        };
    }
    vec![json!({"role":"user","parts":[{"text":""}]})]
}

fn gemini_contents_from_input_items(items: &[Value]) -> Vec<Value> {
    let mut contents = Vec::new();
    let mut tool_names_by_call_id = std::collections::BTreeMap::new();
    let mut index = 0;
    while index < items.len() {
        let item = &items[index];
        let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
        match role {
            "system" => {}
            "assistant" => {
                let mut parts = Vec::new();
                if let Some(text) = gemini_message_text(item).filter(|text| !text.is_empty()) {
                    parts.push(json!({ "text": text }));
                }
                if let Some(tool_calls) = item.get("tool_calls").and_then(Value::as_array) {
                    for tool_call in tool_calls {
                        let call_id = tool_call
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let Some(function) = tool_call.get("function") else {
                            continue;
                        };
                        let name = function
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("tool_call");
                        if !call_id.is_empty() {
                            tool_names_by_call_id.insert(call_id.to_string(), name.to_string());
                        }
                        let args = function
                            .get("arguments")
                            .and_then(Value::as_str)
                            .and_then(|args| serde_json::from_str::<Value>(args).ok())
                            .unwrap_or_else(|| json!({}));
                        let mut function_call = json!({
                            "name": name,
                            "args": args,
                        });
                        if !call_id.trim().is_empty() {
                            function_call["id"] = Value::String(call_id.to_string());
                        }
                        parts.push(json!({ "functionCall": function_call }));
                    }
                }
                if !parts.is_empty() {
                    contents.push(json!({
                        "role": "model",
                        "parts": parts,
                    }));
                }
            }
            "tool" => {
                let mut parts = Vec::new();
                while index < items.len()
                    && items[index].get("role").and_then(Value::as_str) == Some("tool")
                {
                    let tool_item = &items[index];
                    let call_id = tool_item
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let name = tool_item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| tool_names_by_call_id.get(call_id).cloned())
                        .unwrap_or_else(|| "tool_call".to_string());
                    let response = gemini_tool_response_from_message(tool_item);
                    let mut function_response = json!({
                        "name": name,
                        "response": response,
                    });
                    if !call_id.trim().is_empty() {
                        function_response["id"] = Value::String(call_id.to_string());
                    }
                    parts.push(json!({ "functionResponse": function_response }));
                    index += 1;
                }
                contents.push(json!({
                    "role": "user",
                    "parts": parts,
                }));
                continue;
            }
            _ => {
                if gemini_contextual_user_instruction_text(item).is_some() {
                    index += 1;
                    continue;
                }
                let parts =
                    gemini_content_parts_from_message_content(item.get("content").unwrap_or(item));
                if !parts.is_empty() {
                    contents.push(json!({
                        "role":"user",
                        "parts": parts,
                    }));
                }
            }
        }
        index += 1;
    }
    if contents.is_empty() {
        contents.push(json!({"role":"user","parts":[{"text":""}]}));
    }
    contents
}

fn gemini_tool_response_from_message(message: &Value) -> Value {
    let text = gemini_message_text(message).unwrap_or_default();
    serde_json::from_str::<Value>(&text).unwrap_or_else(|_| {
        json!({
            "output": text
        })
    })
}
