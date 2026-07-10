pub(crate) use self::response_chat_messages::gemini_chat_assistant_messages_from_generate_value;
pub(crate) use self::response_grounding::{
    gemini_citation_text, gemini_web_search_call_from_grounding,
};
pub(crate) use self::response_media::{
    gemini_image_generation_call_item_from_part, gemini_media_content_item_from_part,
    gemini_text_from_special_part,
};
pub(crate) use self::response_metadata::{gemini_response_metadata, gemini_responses_usage};
use self::response_status::{GeminiResponseStatus, gemini_response_status};
pub(crate) use self::response_status::{
    gemini_finish_reason, gemini_finish_reason_failure, gemini_finish_reason_incomplete,
    gemini_prompt_feedback_failure,
};
pub(crate) use self::response_tool_calls::gemini_chat_assistant_tool_call_item_with_call_id;
pub(crate) use self::response_tool_calls::gemini_custom_apply_patch_input;
pub(crate) use self::response_tool_calls::gemini_response_tool_call_added_item_with_call_id;
use self::response_tool_calls::gemini_response_tool_call_item;
pub(crate) use self::response_tool_calls::gemini_response_tool_call_item_with_call_id;
pub(crate) use self::response_tool_calls::gemini_response_tool_call_raw_item_with_call_id;
use serde_json::Value;
use std::borrow::Cow;

#[path = "response/chat_messages.rs"]
mod response_chat_messages;
#[path = "response/grounding.rs"]
mod response_grounding;
#[path = "response_media.rs"]
mod response_media;
#[path = "response/metadata.rs"]
mod response_metadata;
#[path = "response/status.rs"]
mod response_status;
#[path = "response_tool_calls.rs"]
mod response_tool_calls;

pub(crate) fn gemini_normalized_response_value(value: &Value) -> Cow<'_, Value> {
    let Some(response) = value.get("response") else {
        return Cow::Borrowed(value);
    };
    let Some(trace_id) = value.get("traceId").and_then(Value::as_str) else {
        return Cow::Borrowed(response);
    };
    let mut response = response.clone();
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "responseId".to_string(),
            Value::String(trace_id.to_string()),
        );
    }
    Cow::Owned(response)
}

pub(crate) fn gemini_runtime_responses_value_from_generate_value(
    value: &Value,
    request_id: u64,
    created_at: u64,
    default_model: &str,
    mut blocked_tool_call_message: impl FnMut(&str, &Value) -> Option<String>,
) -> Value {
    let response_id = value
        .get("responseId")
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("resp_gemini_{request_id}"));
    let model = value
        .get("modelVersion")
        .or_else(|| value.get("model"))
        .and_then(Value::as_str)
        .unwrap_or(default_model)
        .to_string();
    gemini_build_response_value(
        value,
        &response_id,
        &model,
        Some(created_at),
        false,
        false,
        true,
        crate::gemini_bridge::gemini_provider_core_visible_text_from_part,
        |part, function_call, index| {
            let call_id = gemini_function_call_id(function_call, request_id, index);
            crate::gemini_bridge::gemini_provider_core_response_tool_call_item(
                part,
                function_call,
                Some(&call_id),
                &mut blocked_tool_call_message,
            )
        },
    )
}

pub(super) fn gemini_responses_value_from_generate_value(value: &Value) -> Value {
    let response_id = value
        .get("responseId")
        .and_then(Value::as_str)
        .unwrap_or("gemini_resp_prodex");
    let model = value
        .get("modelVersion")
        .and_then(Value::as_str)
        .unwrap_or("gemini-2.5-pro");
    gemini_build_response_value(
        value,
        response_id,
        model,
        None,
        true,
        true,
        false,
        |part| {
            part.get("text")
                .and_then(Value::as_str)
                .filter(|_| {
                    !part
                        .get("thought")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .map(str::to_string)
        },
        |part, function_call, _index| gemini_response_tool_call_item(part, function_call),
    )
}

#[path = "response/build.rs"]
mod response_build;

use self::response_build::{gemini_build_response_value, gemini_function_call_id};
