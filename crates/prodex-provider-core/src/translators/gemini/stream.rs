//! Gemini SSE stream-event normalization.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

use prodex_mojo_core::rich::{
    GeminiResponseKernelInput, GeminiResponseKernelOperation, gemini_response_kernel,
};

pub(super) fn gemini_mojo_value(input: GeminiResponseKernelInput<'_>) -> Value {
    let body = gemini_response_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Gemini response kernel failed: {error:?}"));
    serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo Gemini response kernel returned invalid JSON: {error}")
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeminiProviderCoreStreamChunkMetadata {
    pub response_id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<Value>,
    pub response_metadata: Option<Value>,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeminiProviderCoreStreamToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub thought_signature: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeminiProviderCoreStreamFunctionCallDelta {
    pub explicit_call_id: Option<String>,
    pub name: String,
    pub arguments: String,
}

#[path = "stream/events.rs"]
mod events;
#[path = "stream/shaping.rs"]
mod shaping;
#[cfg(feature = "mojo")]
pub(crate) use self::shaping::gemini_provider_core_stream_chat_tool_call_item;
pub use events::{
    gemini_provider_core_function_call_arguments_delta_event,
    gemini_provider_core_output_item_added_event, gemini_provider_core_output_item_done_event,
    gemini_provider_core_response_completed_event, gemini_provider_core_response_created_event,
    gemini_provider_core_response_incomplete_event, gemini_provider_core_response_metadata_event,
    gemini_provider_core_stream_function_call_arguments_delta_source,
};
pub use shaping::*;

pub fn gemini_provider_core_stream_response_value(
    response_id: &str,
    output: Vec<Value>,
    model: Option<&str>,
    usage: Option<Value>,
    metadata: Option<Value>,
) -> Value {
    {
        let output = serde_json::to_string(&output).expect("stream output serializes");
        let usage = usage.map(|value| serde_json::to_string(&value).expect("usage serializes"));
        let metadata =
            metadata.map(|value| serde_json::to_string(&value).expect("metadata serializes"));
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ResponseValue);
        input.response_id = Some(response_id);
        input.output = Some(&output);
        input.model = model;
        input.usage = usage.as_deref();
        input.metadata = metadata.as_deref();
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_stream_output_text_content(text: &str) -> Value {
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputTextContent);
        input.delta = Some(text);
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_stream_message_item(item_id: &str, content: Vec<Value>) -> Value {
    {
        let content = serde_json::to_string(&content).expect("message content serializes");
        let mut input = GeminiResponseKernelInput::new(GeminiResponseKernelOperation::MessageItem);
        input.response_id = Some(item_id);
        input.content = Some(&content);
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_stream_output_message_item(content: Vec<Value>) -> Value {
    {
        let content = serde_json::to_string(&content).expect("message content serializes");
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputMessageItem);
        input.content = Some(&content);
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_stream_chat_assistant_message(
    output_text: &str,
    reasoning_content: &str,
    media_content_items: &[Value],
    native_parts: &[Value],
    image_generation_items: &[Value],
    metadata: Option<&Value>,
    tool_calls: &[GeminiProviderCoreStreamToolCall],
) -> Option<Value> {
    if output_text.is_empty()
        && reasoning_content.is_empty()
        && media_content_items.is_empty()
        && image_generation_items.is_empty()
        && tool_calls.is_empty()
    {
        return None;
    }
    let tool_call_items = tool_calls
        .iter()
        .map(shaping::gemini_provider_core_stream_chat_tool_call_item)
        .collect::<Vec<_>>();
    {
        let media_content = (!media_content_items.is_empty()).then(|| {
            serde_json::to_string(media_content_items).expect("stream media content serializes")
        });
        let native_parts = (!native_parts.is_empty())
            .then(|| serde_json::to_string(native_parts).expect("stream native parts serialize"));
        let image_generation = (!image_generation_items.is_empty()).then(|| {
            serde_json::to_string(image_generation_items)
                .expect("stream image generation items serialize")
        });
        let metadata = metadata.map(|value| {
            serde_json::to_string(value).expect("stream assistant metadata serializes")
        });
        let tool_calls = (!tool_call_items.is_empty())
            .then(|| serde_json::to_string(&tool_call_items).expect("stream tool calls serialize"));
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamAssistantMessage);
        input.delta = (!output_text.is_empty()).then_some(output_text);
        input.reason = (!reasoning_content.is_empty()).then_some(reasoning_content);
        input.content = media_content.as_deref();
        input.item = native_parts.as_deref();
        input.output = image_generation.as_deref();
        input.metadata = metadata.as_deref();
        input.arguments = tool_calls.as_deref();
        Some(gemini_mojo_value(input))
    }
}

pub fn gemini_provider_core_stream_output_items(
    web_search_call: Option<&Value>,
    image_generation_items: &[Value],
    output_text: &str,
    media_content_items: &[Value],
    citation_text: Option<&str>,
    tool_calls: &[GeminiProviderCoreStreamToolCall],
    mut blocked_tool_call_message: impl FnMut(&str, &Value) -> Option<String>,
) -> Vec<Value> {
    {
        let mut tool_call_items = Vec::new();
        for tool_call in tool_calls {
            gemini_append_stream_tool_call(
                &mut tool_call_items,
                tool_call,
                &mut blocked_tool_call_message,
            );
        }
        let web_search_call = web_search_call
            .map(|item| serde_json::to_string(item).expect("stream web search call serializes"));
        let image_generation_items = (!image_generation_items.is_empty()).then(|| {
            serde_json::to_string(image_generation_items)
                .expect("stream image generation items serialize")
        });
        let media_content_items = (!media_content_items.is_empty()).then(|| {
            serde_json::to_string(media_content_items).expect("stream media content serializes")
        });
        let tool_call_items = (!tool_call_items.is_empty()).then(|| {
            serde_json::to_string(&tool_call_items).expect("stream output tool calls serialize")
        });
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamOutputItems);
        input.response = web_search_call.as_deref();
        input.output = image_generation_items.as_deref();
        input.delta = (!output_text.is_empty()).then_some(output_text);
        input.content = media_content_items.as_deref();
        input.reason = citation_text;
        input.reason_present = citation_text.is_some();
        input.arguments = tool_call_items.as_deref();
        let value = gemini_mojo_value(input);
        value.as_array().cloned().unwrap_or_else(|| {
            panic!("Mojo Gemini stream output-items kernel returned a non-array")
        })
    }
}

fn gemini_append_stream_tool_call(
    output: &mut Vec<Value>,
    tool_call: &GeminiProviderCoreStreamToolCall,
    blocked_tool_call_message: &mut impl FnMut(&str, &Value) -> Option<String>,
) {
    let args_value = serde_json::from_str::<Value>(&tool_call.arguments)
        .unwrap_or_else(|_| Value::String(tool_call.arguments.clone()));
    if let Some(blocked) = blocked_tool_call_message(&tool_call.name, &args_value) {
        output.push(gemini_provider_core_stream_output_message_item(vec![
            gemini_provider_core_stream_output_text_content(&blocked),
        ]));
    } else {
        output.push(
            shaping::gemini_provider_core_stream_completed_tool_call_item(
                &tool_call.call_id,
                &tool_call.name,
                &tool_call.arguments,
                tool_call.thought_signature.as_deref(),
                false,
            ),
        );
    }
}

pub fn gemini_provider_core_stream_text_delta_source(text: &str) -> Value {
    {
        let mut input = GeminiResponseKernelInput::new(GeminiResponseKernelOperation::TextSource);
        input.delta = Some(text);
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_stream_reasoning_delta_source(text: &str) -> Value {
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ReasoningSource);
        input.delta = Some(text);
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_output_text_delta_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    delta: &str,
) -> Value {
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputTextDelta);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        input.delta = Some(delta);
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_reasoning_summary_part_added_event(
    sequence_number: u64,
    response_id: &str,
    summary_index: u64,
) -> Value {
    {
        let mut input = GeminiResponseKernelInput::new(
            GeminiResponseKernelOperation::ReasoningSummaryPartAdded,
        );
        input.sequence_number = sequence_number;
        input.response_id = Some(response_id);
        input.summary_index = summary_index;
        gemini_mojo_value(input)
    }
}

pub fn gemini_provider_core_reasoning_summary_text_delta_event(
    sequence_number: u64,
    response_id: &str,
    summary_index: u64,
    delta: &str,
) -> Value {
    {
        let mut input = GeminiResponseKernelInput::new(
            GeminiResponseKernelOperation::ReasoningSummaryTextDelta,
        );
        input.sequence_number = sequence_number;
        input.response_id = Some(response_id);
        input.summary_index = summary_index;
        input.delta = Some(delta);
        gemini_mojo_value(input)
    }
}

pub(super) fn gemini_transform_stream_event(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if super::gemini_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::GeminiGenerateContent,
            ProviderWireFormat::OpenAiResponses,
            input.body,
        );
    }
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::GeminiGenerateContent,
            ProviderWireFormat::OpenAiResponses,
            format!(
                "Gemini translator does not support {}",
                input.endpoint.label()
            ),
        );
    }
    let event = String::from_utf8_lossy(&input.body);
    let Some(data) = event
        .strip_prefix("data: ")
        .and_then(|s| s.strip_suffix("\n\n"))
    else {
        return ProviderTransformResult::unsupported(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::GeminiGenerateContent,
            ProviderWireFormat::OpenAiResponses,
            "Gemini SSE event must use data: <json> framing",
        );
    };
    {
        let mut kernel_input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamEventTransform);
        kernel_input.response = Some(data);
        let packet = gemini_mojo_value(kernel_input);
        match packet.get("status").and_then(Value::as_str) {
            Some("ok") => {
                let event_name = packet
                    .get("event")
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| panic!("Mojo Gemini stream transform omitted event"));
                let transformed = packet
                    .get("value")
                    .unwrap_or_else(|| panic!("Mojo Gemini stream transform omitted value"));
                let body = format!("event: {event_name}\ndata: {transformed}\n\n");
                ProviderTransformResult::lossless(
                    ProviderId::Gemini,
                    input.endpoint,
                    ProviderWireFormat::GeminiGenerateContent,
                    ProviderWireFormat::OpenAiResponses,
                    body.into_bytes(),
                )
            }
            Some("invalid") => ProviderTransformResult::rejected(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::GeminiGenerateContent,
                ProviderWireFormat::OpenAiResponses,
                "failed to parse Gemini SSE JSON",
            ),
            Some("unsupported") => ProviderTransformResult::unsupported(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::GeminiGenerateContent,
                ProviderWireFormat::OpenAiResponses,
                "Gemini SSE event does not contain a supported text or function-call delta",
            ),
            status => panic!("Mojo Gemini stream transform returned invalid status: {status:?}"),
        }
    }
}
