//! Gemini GenerateContent response builder.

use serde_json::{Value, json};

use super::{
    GeminiResponseStatus, gemini_citation_text, gemini_image_generation_call_item_from_part,
    gemini_media_content_item_from_part, gemini_response_metadata, gemini_response_status,
    gemini_responses_usage, gemini_text_from_special_part, gemini_web_search_call_from_grounding,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn gemini_build_response_value(
    value: &Value,
    response_id: &str,
    model: &str,
    created_at: Option<u64>,
    include_empty_usage: bool,
    include_empty_metadata: bool,
    suppress_visible_text_when_tool_calls: bool,
    mut visible_text_from_part: impl FnMut(&Value) -> Option<String>,
    mut function_call_item: impl FnMut(&Value, &Value, usize) -> Value,
) -> Value {
    let parts = value
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut output = Vec::new();
    let mut text = String::new();
    let mut content_items = Vec::new();
    let suppress_visible_text = suppress_visible_text_when_tool_calls
        && parts.iter().any(|part| part.get("functionCall").is_some());
    for (index, part) in parts.into_iter().enumerate() {
        if !suppress_visible_text && let Some(part_text) = visible_text_from_part(&part) {
            text.push_str(&part_text);
        }
        if let Some(part_text) = gemini_text_from_special_part(&part) {
            content_items.push(json!({
                "type": "output_text",
                "text": part_text,
            }));
        }
        if let Some(content_item) = gemini_media_content_item_from_part(&part) {
            content_items.push(content_item);
        }
        if let Some(image_generation) =
            gemini_image_generation_call_item_from_part(response_id, index, &part)
        {
            output.push(image_generation);
        }
        if let Some(function_call) = part.get("functionCall") {
            output.push(function_call_item(&part, function_call, index));
        }
    }
    if !text.is_empty() {
        let mut content = vec![json!({
            "type":"output_text",
            "text": text,
        })];
        content.extend(content_items);
        output.insert(
            0,
            json!({
                "type":"message",
                "role":"assistant",
                "content": content,
            }),
        );
    } else if !content_items.is_empty() {
        output.insert(
            0,
            json!({
                "type":"message",
                "role":"assistant",
                "content": content_items,
            }),
        );
    }
    if let Some(grounding_call) = gemini_web_search_call_from_grounding(value, response_id) {
        output.push(grounding_call);
    }
    if let Some(citations) = gemini_citation_text(value) {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": citations,
            }],
        }));
    }
    let has_visible_output = !output.is_empty();
    let mut response = json!({
        "id": response_id,
        "object": "response",
        "model": model,
        "output": output,
    });
    if let Some(created_at) = created_at {
        response["created_at"] = json!(created_at);
    }
    if let Some(usage) = value.get("usageMetadata").and_then(gemini_responses_usage) {
        response["usage"] = usage;
    } else if include_empty_usage {
        response["usage"] = json!({});
    }
    if let Some(metadata) = gemini_response_metadata(value) {
        response["metadata"] = metadata;
    } else if include_empty_metadata {
        response["metadata"] = json!({});
    }
    if let Some(status) = gemini_response_status(value, has_visible_output) {
        match status {
            GeminiResponseStatus::Failed { code, message } => {
                response["status"] = Value::String("failed".to_string());
                response["error"] = json!({
                    "code": code,
                    "message": message,
                });
            }
            GeminiResponseStatus::Incomplete { reason, message } => {
                response["status"] = Value::String("incomplete".to_string());
                response["incomplete_details"] = json!({
                    "reason": reason,
                    "message": message,
                });
            }
        }
    }
    response
}

pub(super) fn gemini_function_call_id(
    function_call: &Value,
    request_id: u64,
    index: usize,
) -> String {
    gemini_function_call_id_with_fallback(function_call, || {
        format!("call_gemini_{request_id}_{index}")
    })
}

pub(super) fn gemini_function_call_id_with_fallback(
    function_call: &Value,
    fallback_call_id: impl FnOnce() -> String,
) -> String {
    function_call
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(fallback_call_id)
}
