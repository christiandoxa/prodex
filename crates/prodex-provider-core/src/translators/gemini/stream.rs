//! Gemini SSE stream-event normalization.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::{Value, json};

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{
    GeminiResponseKernelInput, GeminiResponseKernelOperation, gemini_response_kernel,
};

#[cfg(feature = "mojo")]
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
    #[cfg(feature = "mojo")]
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
    #[cfg(not(feature = "mojo"))]
    {
        let mut response = json!({
            "id": response_id,
            "output": output,
        });
        if let Some(model) = model {
            response["model"] = Value::String(model.to_string());
        }
        if let Some(usage) = usage {
            response["usage"] = usage;
        }
        if let Some(metadata) = metadata {
            response["metadata"] = metadata;
        }
        response
    }
}

pub fn gemini_provider_core_stream_output_text_content(text: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputTextContent);
        input.delta = Some(text);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "output_text",
        "text": text,
    })
}

pub fn gemini_provider_core_stream_message_item(item_id: &str, content: Vec<Value>) -> Value {
    #[cfg(feature = "mojo")]
    {
        let content = serde_json::to_string(&content).expect("message content serializes");
        let mut input = GeminiResponseKernelInput::new(GeminiResponseKernelOperation::MessageItem);
        input.response_id = Some(item_id);
        input.content = Some(&content);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "id": item_id,
        "type": "message",
        "role": "assistant",
        "content": content,
    })
}

pub fn gemini_provider_core_stream_output_message_item(content: Vec<Value>) -> Value {
    #[cfg(feature = "mojo")]
    {
        let content = serde_json::to_string(&content).expect("message content serializes");
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputMessageItem);
        input.content = Some(&content);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "message",
        "role": "assistant",
        "content": content,
    })
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
    #[cfg(feature = "mojo")]
    let tool_call_items = tool_calls
        .iter()
        .map(shaping::gemini_provider_core_stream_chat_tool_call_item)
        .collect::<Vec<_>>();
    #[cfg(not(feature = "mojo"))]
    let tool_call_items = tool_calls
        .iter()
        .map(|tool_call| {
            let mut item = json!({
                "id": tool_call.call_id,
                "type": "function",
                "function": {
                    "name": tool_call.name,
                    "arguments": tool_call.arguments,
                },
            });
            if let Some(signature) = tool_call.thought_signature.as_deref() {
                item["gemini_thought_signature"] = Value::String(signature.to_string());
            }
            item
        })
        .collect::<Vec<_>>();
    #[cfg(feature = "mojo")]
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
    #[cfg(not(feature = "mojo"))]
    {
        let mut assistant = json!({
            "role": "assistant",
            "content": if output_text.is_empty() {
                if tool_calls.is_empty() {
                    Value::Null
                } else {
                    Value::String(String::new())
                }
            } else {
                Value::String(output_text.to_string())
            },
        });
        if !reasoning_content.is_empty() {
            assistant["reasoning_content"] = Value::String(reasoning_content.to_string());
        }
        if !media_content_items.is_empty() {
            assistant["gemini_media_content"] = Value::Array(media_content_items.to_vec());
        }
        if !native_parts.is_empty() {
            assistant["gemini_native_parts"] = Value::Array(native_parts.to_vec());
        }
        if !image_generation_items.is_empty() {
            assistant["gemini_image_generation"] = Value::Array(image_generation_items.to_vec());
        }
        if let Some(metadata) = metadata {
            assistant["gemini_metadata"] = metadata.clone();
        }
        if !tool_calls.is_empty() {
            assistant["tool_calls"] = Value::Array(tool_call_items);
        }
        Some(assistant)
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
    #[cfg(feature = "mojo")]
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
    #[cfg(not(feature = "mojo"))]
    {
        let mut output = Vec::new();
        if let Some(item) = web_search_call {
            output.push(item.clone());
        }
        output.extend(image_generation_items.iter().cloned());
        if !output_text.is_empty() {
            let mut content = vec![gemini_provider_core_stream_output_text_content(output_text)];
            content.extend(media_content_items.iter().cloned());
            output.push(gemini_provider_core_stream_output_message_item(content));
        } else if !media_content_items.is_empty() {
            output.push(gemini_provider_core_stream_output_message_item(
                media_content_items.to_vec(),
            ));
        }
        if let Some(citations) = citation_text {
            output.push(gemini_provider_core_stream_output_message_item(vec![
                gemini_provider_core_stream_output_text_content(citations),
            ]));
        }
        for tool_call in tool_calls {
            gemini_append_stream_tool_call(&mut output, tool_call, &mut blocked_tool_call_message);
        }
        output
    }
}

fn gemini_append_stream_tool_call(
    output: &mut Vec<Value>,
    tool_call: &GeminiProviderCoreStreamToolCall,
    blocked_tool_call_message: &mut impl FnMut(&str, &Value) -> Option<String>,
) {
    match serde_json::from_str::<Value>(&tool_call.arguments) {
        Ok(args_value) => {
            if let Some(blocked) = blocked_tool_call_message(&tool_call.name, &args_value) {
                output.push(crate::gemini_provider_core_blocked_tool_call_item(&blocked));
                return;
            }
            let mut function_call = json!({
                "name": tool_call.name,
                "args": args_value,
            });
            if let Some(signature) = tool_call.thought_signature.as_deref() {
                function_call["thoughtSignature"] = Value::String(signature.to_string());
            }
            output.push(super::gemini_response_tool_call_item_with_call_id(
                &json!({}),
                &function_call,
                Some(&tool_call.call_id),
            ));
        }
        Err(_) => {
            let raw_arguments_value = Value::String(tool_call.arguments.clone());
            if let Some(blocked) = blocked_tool_call_message(&tool_call.name, &raw_arguments_value)
            {
                output.push(crate::gemini_provider_core_blocked_tool_call_item(&blocked));
                return;
            }
            let mut part = json!({});
            if let Some(signature) = tool_call.thought_signature.as_deref() {
                part["thoughtSignature"] = Value::String(signature.to_string());
            }
            output.push(super::gemini_response_tool_call_raw_item_with_call_id(
                &part,
                &tool_call.name,
                &tool_call.arguments,
                Some(&tool_call.call_id),
            ));
        }
    }
}

pub fn gemini_provider_core_stream_text_delta_source(text: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = GeminiResponseKernelInput::new(GeminiResponseKernelOperation::TextSource);
        input.delta = Some(text);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "text": text,
                }]
            }
        }]
    })
}

pub fn gemini_provider_core_stream_reasoning_delta_source(text: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ReasoningSource);
        input.delta = Some(text);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "text": text,
                    "thought": true,
                }]
            }
        }]
    })
}

pub fn gemini_provider_core_output_text_delta_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    delta: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputTextDelta);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        input.delta = Some(delta);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.output_text.delta",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response_id": response_id,
        "delta": delta,
    })
}

pub fn gemini_provider_core_reasoning_summary_part_added_event(
    sequence_number: u64,
    response_id: &str,
    summary_index: u64,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = GeminiResponseKernelInput::new(
            GeminiResponseKernelOperation::ReasoningSummaryPartAdded,
        );
        input.sequence_number = sequence_number;
        input.response_id = Some(response_id);
        input.summary_index = summary_index;
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.reasoning_summary_part.added",
        "sequence_number": sequence_number,
        "response_id": response_id,
        "summary_index": summary_index,
    })
}

pub fn gemini_provider_core_reasoning_summary_text_delta_event(
    sequence_number: u64,
    response_id: &str,
    summary_index: u64,
    delta: &str,
) -> Value {
    #[cfg(feature = "mojo")]
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
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.reasoning_summary_text.delta",
        "sequence_number": sequence_number,
        "response_id": response_id,
        "summary_index": summary_index,
        "delta": delta,
    })
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
    #[cfg(feature = "mojo")]
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

    #[cfg(not(feature = "mojo"))]
    {
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(error) => {
                return ProviderTransformResult::rejected(
                    ProviderId::Gemini,
                    input.endpoint,
                    ProviderWireFormat::GeminiGenerateContent,
                    ProviderWireFormat::OpenAiResponses,
                    format!("failed to parse Gemini SSE JSON: {error}"),
                );
            }
        };
        let Some((event_name, transformed)) = gemini_stream_event_from_generate_value(&value)
        else {
            return ProviderTransformResult::unsupported(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::GeminiGenerateContent,
                ProviderWireFormat::OpenAiResponses,
                "Gemini SSE event does not contain a supported text or function-call delta",
            );
        };
        let body = format!("event: {event_name}\ndata: {}\n\n", transformed);
        ProviderTransformResult::lossless(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::GeminiGenerateContent,
            ProviderWireFormat::OpenAiResponses,
            body.into_bytes(),
        )
    }
}

#[cfg(not(feature = "mojo"))]
fn gemini_stream_event_from_generate_value(value: &Value) -> Option<(&'static str, Value)> {
    if let Some(function_call) = value.pointer("/candidates/0/content/parts/0/functionCall") {
        let args = function_call
            .get("args")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let arguments = serde_json::to_string(&args).ok()?;
        let transformed = {
            let mut transformed = json!({
                "type":"response.function_call_arguments.delta",
                "delta": arguments,
            });
            if let Some(call_id) = function_call.get("id").and_then(Value::as_str)
                && let Some(object) = transformed.as_object_mut()
            {
                object.insert("call_id".to_string(), Value::String(call_id.to_string()));
            }
            transformed
        };
        return Some(("response.function_call_arguments.delta", transformed));
    }
    if let Some(part) = value.pointer("/candidates/0/content/parts/0")
        && part
            .get("thought")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        let text = part.get("text").and_then(Value::as_str)?;
        let transformed = json!({
            "type":"response.reasoning_summary_text.delta",
            "delta":text,
        });
        return Some(("response.reasoning_summary_text.delta", transformed));
    }
    let text = value
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(Value::as_str)?;
    let transformed = json!({
        "type":"response.output_text.delta",
        "delta":text,
    });
    Some(("response.output_text.delta", transformed))
}
