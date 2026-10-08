//! Gemini GenerateContent response conversion into chat-compatible assistant messages.

use prodex_mojo_core::rich::{
    GeminiChatAssistantResponseOperation, gemini_chat_assistant_response_kernel,
};
use serde_json::{Value, json};

use super::response_media::{gemini_media_content_item_from_part, gemini_text_from_special_part};
use super::response_metadata::gemini_response_metadata;
use super::response_tool_calls::gemini_rtk_wrapped_tool_arguments;
use crate::mojo_json::Document;

#[derive(serde::Deserialize)]
struct GeminiChatBlockedToolCall {
    name: String,
    args: Value,
}

pub(crate) fn gemini_chat_assistant_messages_from_generate_value(
    value: &Value,
    request_id: u64,
    mut blocked_tool_call_message: impl FnMut(&str, &Value) -> Option<String>,
) -> Vec<Value> {
    let parts = value
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array);
    let part_data = Value::Array(
        parts
            .into_iter()
            .flatten()
            .map(|part| {
                json!({
                    "visible": crate::gemini_bridge::gemini_provider_core_visible_text_from_part(part),
                    "special": gemini_text_from_special_part(part),
                    "media": gemini_media_content_item_from_part(part),
                })
            })
            .collect(),
    );

    let plan = gemini_chat_response_kernel(
        &[value],
        GeminiChatAssistantResponseOperation::BlockedToolCallPlan,
    )
    .expect("Mojo Gemini chat callback planner returned no result");
    let calls: Vec<GeminiChatBlockedToolCall> = serde_json::from_slice(&plan)
        .expect("Mojo Gemini chat callback planner returned invalid JSON");
    let mut blocked = Vec::with_capacity(calls.len());
    let mut wrapped_arguments = Vec::with_capacity(calls.len());
    for call in calls {
        blocked.push(
            blocked_tool_call_message(&call.name, &call.args)
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        let arguments = serde_json::to_string(&call.args)
            .expect("Gemini chat function-call arguments serialize");
        wrapped_arguments.push(Value::String(gemini_rtk_wrapped_tool_arguments(
            &call.name, &arguments,
        )));
    }

    let metadata = gemini_response_metadata(value).unwrap_or(Value::Null);
    let request_id = Value::String(request_id.to_string());
    let blocked = Value::Array(blocked);
    let wrapped_arguments = Value::Array(wrapped_arguments);
    let bytes = gemini_chat_response_kernel(
        &[
            value,
            &part_data,
            &blocked,
            &wrapped_arguments,
            &metadata,
            &request_id,
        ],
        GeminiChatAssistantResponseOperation::Assemble,
    );
    bytes
        .map(|bytes| {
            vec![
                serde_json::from_slice(&bytes)
                    .expect("Mojo Gemini chat message assembler returned invalid JSON"),
            ]
        })
        .unwrap_or_default()
}

fn gemini_chat_response_kernel(
    values: &[&Value],
    operation: GeminiChatAssistantResponseOperation,
) -> Option<Vec<u8>> {
    let mut document = Document::default();
    document.array(values.iter().copied());
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    gemini_chat_assistant_response_kernel(&document.nodes, raw, operation)
        .unwrap_or_else(|error| panic!("Mojo Gemini chat response kernel failed: {error:?}"))
}
