//! Gemini GenerateContent response builder.

use serde_json::{Value, json};

use super::{GeminiResponseStatus, gemini_response_status};
use super::{
    gemini_citation_text, gemini_image_generation_call_item_from_part,
    gemini_media_content_item_from_part, gemini_response_metadata, gemini_responses_usage,
    gemini_text_from_special_part, gemini_web_search_call_from_grounding,
};
use crate::{GeminiProviderCoreResponsePartInput, gemini_provider_core_response_part_plan};

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{GeminiResponseKernelInput, GeminiResponseKernelOperation};

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
    let (text, content_items) = gemini_collect_response_parts(
        parts,
        response_id,
        suppress_visible_text_when_tool_calls,
        &mut visible_text_from_part,
        &mut function_call_item,
        &mut output,
    );
    let citations = gemini_append_grounding_and_citations(&mut output, value, response_id);
    #[cfg(feature = "mojo")]
    {
        let has_visible_output = !output.is_empty()
            || !text.is_empty()
            || !content_items.is_empty()
            || citations.is_some();
        let output = serde_json::to_string(&output).expect("Gemini response output serializes");
        let content =
            serde_json::to_string(&content_items).expect("Gemini response content serializes");
        let usage = value
            .get("usageMetadata")
            .and_then(gemini_responses_usage)
            .map(|value| serde_json::to_string(&value).expect("Gemini response usage serializes"));
        let metadata = gemini_response_metadata(value).map(|value| {
            serde_json::to_string(&value).expect("Gemini response metadata serializes")
        });
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::BufferedResponse);
        input.response_id = Some(response_id);
        input.model = Some(model);
        input.created_at = created_at.unwrap_or_default();
        input.created_at_present = created_at.is_some();
        input.include_empty_usage = include_empty_usage;
        input.include_empty_metadata = include_empty_metadata;
        input.delta = (!text.is_empty()).then_some(text.as_str());
        input.content = (!content_items.is_empty()).then_some(content.as_str());
        input.output = Some(&output);
        input.usage = usage.as_deref();
        input.metadata = metadata.as_deref();
        input.citations = citations.as_deref();
        let mut response = super::super::stream::gemini_mojo_value(input);
        gemini_apply_response_status(&mut response, value, has_visible_output);
        response
    }
    #[cfg(not(feature = "mojo"))]
    {
        gemini_insert_response_message(&mut output, text, content_items);
        if let Some(citations) = citations {
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
        gemini_apply_response_status(&mut response, value, has_visible_output);
        response
    }
}

fn gemini_collect_response_parts(
    parts: Vec<Value>,
    response_id: &str,
    suppress_visible_text_when_tool_calls: bool,
    visible_text_from_part: &mut impl FnMut(&Value) -> Option<String>,
    function_call_item: &mut impl FnMut(&Value, &Value, usize) -> Value,
    output: &mut Vec<Value>,
) -> (String, Vec<Value>) {
    let mut text = String::new();
    let mut content_items = Vec::new();
    let suppress_visible_text = suppress_visible_text_when_tool_calls
        && parts.iter().any(|part| part.get("functionCall").is_some());
    for (index, part) in parts.into_iter().enumerate() {
        let visible_text = visible_text_from_part(&part);
        let special_text = gemini_text_from_special_part(&part);
        let content_item = gemini_media_content_item_from_part(&part);
        let image_generation =
            gemini_image_generation_call_item_from_part(response_id, index, &part);
        let plan = gemini_provider_core_response_part_plan(GeminiProviderCoreResponsePartInput {
            has_text: part
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.is_empty()),
            is_thought: part
                .get("thought")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            has_visible_text: visible_text.is_some(),
            has_special_text: special_text.is_some(),
            has_media: content_item.is_some(),
            has_video_metadata: part.get("videoMetadata").is_some(),
            has_image_generation: image_generation.is_some(),
            has_function_call: part.get("functionCall").is_some(),
            command_output_only: false,
            forced_output: false,
            internal_instruction_echo: false,
            suppress_visible_text,
        })
        .expect("Gemini response part planner returned invalid output");
        if plan.emit_visible_text
            && let Some(part_text) = visible_text
        {
            text.push_str(&part_text);
        }
        if plan.emit_special_text
            && let Some(part_text) = special_text
        {
            content_items.push(json!({
                "type": "output_text",
                "text": part_text,
            }));
        }
        if plan.record_media
            && let Some(content_item) = content_item
        {
            content_items.push(content_item);
        }
        if plan.record_image
            && let Some(image_generation) = image_generation
        {
            output.push(image_generation);
        }
        if plan.emit_function
            && let Some(function_call) = part.get("functionCall")
        {
            output.push(function_call_item(&part, function_call, index));
        }
    }
    (text, content_items)
}

#[cfg(not(feature = "mojo"))]
fn gemini_insert_response_message(
    output: &mut Vec<Value>,
    text: String,
    content_items: Vec<Value>,
) {
    if text.is_empty() && content_items.is_empty() {
        return;
    }
    let content = if text.is_empty() {
        content_items
    } else {
        let mut content = vec![json!({
            "type":"output_text",
            "text": text,
        })];
        content.extend(content_items);
        content
    };
    output.insert(
        0,
        json!({
            "type":"message",
            "role":"assistant",
            "content": content,
        }),
    );
}

fn gemini_append_grounding_and_citations(
    output: &mut Vec<Value>,
    value: &Value,
    response_id: &str,
) -> Option<String> {
    if let Some(grounding_call) = gemini_web_search_call_from_grounding(value, response_id) {
        output.push(grounding_call);
    }
    gemini_citation_text(value)
}

fn gemini_apply_response_status(response: &mut Value, value: &Value, has_visible_output: bool) {
    let Some(status) = gemini_response_status(value, has_visible_output) else {
        return;
    };
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
