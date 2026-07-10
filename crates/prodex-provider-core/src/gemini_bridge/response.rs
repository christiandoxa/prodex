//! Gemini response-shape bridge helpers.

mod simple;
mod status;
mod tool_calls;

pub use self::simple::gemini_provider_core_simple_response;
pub use self::status::{
    gemini_provider_core_finish_reason, gemini_provider_core_finish_reason_failure,
    gemini_provider_core_finish_reason_incomplete,
    gemini_provider_core_finish_reason_retryable_invalid,
    gemini_provider_core_prompt_feedback_failure,
    gemini_provider_core_response_terminal_without_history,
};
pub use self::tool_calls::{
    gemini_provider_core_chat_assistant_tool_call_item,
    gemini_provider_core_custom_tool_input_from_arguments,
    gemini_provider_core_preserve_tool_call_signatures,
    gemini_provider_core_response_tool_call_added_item,
    gemini_provider_core_response_tool_call_item, gemini_provider_core_response_tool_call_raw_item,
};

use crate::translators::{
    gemini_chat_assistant_messages_from_generate_value, gemini_citation_text,
    gemini_image_generation_call_item_from_part, gemini_media_content_item_from_part,
    gemini_normalized_response_value, gemini_response_metadata, gemini_responses_usage,
    gemini_runtime_responses_value_from_generate_value, gemini_text_from_special_part,
    gemini_web_search_call_from_grounding,
};
use crate::{ProviderTransformResult, provider_core_rewritten_json_value};
use std::borrow::Cow;

pub fn gemini_provider_core_normalized_response_value(
    value: &serde_json::Value,
) -> Cow<'_, serde_json::Value> {
    gemini_normalized_response_value(value)
}

pub fn gemini_provider_core_responses_usage(
    usage: &serde_json::Value,
) -> Option<serde_json::Value> {
    gemini_responses_usage(usage)
}

pub fn gemini_provider_core_response_metadata(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    gemini_response_metadata(value)
}

pub fn gemini_provider_core_media_content_item_from_part(
    part: &serde_json::Value,
) -> Option<serde_json::Value> {
    gemini_media_content_item_from_part(part)
}

pub fn gemini_provider_core_text_from_special_part(part: &serde_json::Value) -> Option<String> {
    gemini_text_from_special_part(part)
}

pub fn gemini_provider_core_image_generation_call_item_from_part(
    response_id: &str,
    index: usize,
    part: &serde_json::Value,
) -> Option<serde_json::Value> {
    gemini_image_generation_call_item_from_part(response_id, index, part)
}

pub fn gemini_provider_core_citation_text(value: &serde_json::Value) -> Option<String> {
    gemini_citation_text(value)
}

pub fn gemini_provider_core_web_search_call_from_grounding(
    value: &serde_json::Value,
    response_id: &str,
) -> Option<serde_json::Value> {
    gemini_web_search_call_from_grounding(value, response_id)
}

pub fn gemini_provider_core_chat_assistant_messages(
    value: &serde_json::Value,
    request_id: u64,
    blocked_tool_call_message: impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> Vec<serde_json::Value> {
    gemini_chat_assistant_messages_from_generate_value(value, request_id, blocked_tool_call_message)
}

pub fn gemini_provider_core_buffered_responses_value(
    value: &serde_json::Value,
    request_id: u64,
    translated: Option<&ProviderTransformResult>,
    mut blocked_tool_call_message: impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> serde_json::Value {
    let value = gemini_provider_core_normalized_response_value(value);
    let created_at = crate::provider_core_chat_compatible_created_at();
    let mut response = translated
        .filter(|_| {
            gemini_provider_core_simple_response(value.as_ref(), |name, args| {
                blocked_tool_call_message(name, args)
            })
        })
        .and_then(|result| provider_core_rewritten_json_value(Some(result)))
        .unwrap_or_else(|| {
            gemini_provider_core_runtime_responses_value(
                value.as_ref(),
                request_id,
                created_at,
                crate::PRODEX_GEMINI_DEFAULT_MODEL,
                |name, args| blocked_tool_call_message(name, args),
            )
        });
    if response.get("created_at").is_none() {
        response["created_at"] = serde_json::Value::Number(serde_json::Number::from(created_at));
    }
    response
}

pub fn gemini_provider_core_runtime_responses_value(
    value: &serde_json::Value,
    request_id: u64,
    created_at: u64,
    default_model: &str,
    blocked_tool_call_message: impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> serde_json::Value {
    gemini_runtime_responses_value_from_generate_value(
        value,
        request_id,
        created_at,
        default_model,
        blocked_tool_call_message,
    )
}
