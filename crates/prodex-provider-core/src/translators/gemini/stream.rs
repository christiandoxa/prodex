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
pub use events::{
    gemini_provider_core_function_call_arguments_delta_event,
    gemini_provider_core_output_item_added_event, gemini_provider_core_output_item_done_event,
    gemini_provider_core_response_completed_event, gemini_provider_core_response_created_event,
    gemini_provider_core_response_incomplete_event, gemini_provider_core_response_metadata_event,
    gemini_provider_core_stream_function_call_arguments_delta_source,
};

pub fn gemini_provider_core_stream_output_text_item_id(request_id: u64) -> String {
    format!("msg_gemini_{request_id}")
}

pub fn gemini_provider_core_stream_media_item_id(request_id: u64) -> String {
    format!("msg_gemini_media_{request_id}")
}

pub fn gemini_provider_core_stream_citation_item_id(request_id: u64) -> String {
    format!("msg_gemini_citations_{request_id}")
}

pub fn gemini_provider_core_stream_fallback_response_id(request_id: u64) -> String {
    format!("resp_gemini_{request_id}")
}

pub fn gemini_provider_core_stream_fallback_tool_call_id(request_id: u64, index: usize) -> String {
    format!("call_gemini_{request_id}_{index}")
}

pub fn gemini_provider_core_stream_function_call_delta(
    value: &Value,
) -> GeminiProviderCoreStreamFunctionCallDelta {
    let explicit_call_id = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string);
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call")
        .to_string();
    let arguments = value.get("args").cloned().unwrap_or_else(|| json!({}));
    let arguments = serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string());
    GeminiProviderCoreStreamFunctionCallDelta {
        explicit_call_id,
        name,
        arguments,
    }
}

pub fn gemini_provider_core_stream_tool_call(
    request_id: u64,
    index: usize,
    call_id: Option<&str>,
    name: Option<&str>,
    arguments: &str,
    thought_signature: Option<&str>,
) -> GeminiProviderCoreStreamToolCall {
    GeminiProviderCoreStreamToolCall {
        call_id: call_id.map(str::to_string).unwrap_or_else(|| {
            gemini_provider_core_stream_fallback_tool_call_id(request_id, index)
        }),
        name: name.unwrap_or("tool_call").to_string(),
        arguments: arguments.to_string(),
        thought_signature: thought_signature.map(str::to_string),
    }
}

pub fn gemini_provider_core_stream_tool_call_ids(
    tool_calls: &[GeminiProviderCoreStreamToolCall],
) -> Vec<String> {
    tool_calls
        .iter()
        .map(|tool_call| tool_call.call_id.clone())
        .filter(|call_id| !call_id.trim().is_empty())
        .collect()
}

pub fn gemini_provider_core_stream_should_emit_function_call_arguments_delta(name: &str) -> bool {
    !matches!(name, "tool_search" | "apply_patch")
}

pub fn gemini_provider_core_function_call_arguments_delta_event_with_thought_signature(
    mut event: Value,
    thought_signature: Option<&str>,
) -> Value {
    if let Some(signature) = thought_signature {
        event["thought_signature"] = Value::String(signature.to_string());
    }
    event
}

pub fn gemini_provider_core_stream_completed_tool_call_arguments(
    name: &str,
    arguments: &str,
) -> String {
    if name == "apply_patch" {
        arguments.to_string()
    } else {
        crate::provider_core_chat_compatible_rtk_wrapped_tool_arguments(name, arguments)
    }
}

pub fn gemini_provider_core_stream_tool_call_arguments_value(arguments: &str) -> Value {
    serde_json::from_str::<Value>(arguments)
        .unwrap_or_else(|_| Value::String(arguments.to_string()))
}

pub fn gemini_provider_core_stream_completed_tool_call_item(
    call_id: &str,
    name: &str,
    arguments: &str,
    thought_signature: Option<&str>,
    blocked: bool,
) -> Value {
    if blocked {
        return crate::gemini_provider_core_blocked_tool_call_item(arguments);
    }
    if let Ok(arguments_value) = serde_json::from_str::<Value>(arguments) {
        let mut function_call = json!({
            "name": name,
            "args": arguments_value,
        });
        if let Some(signature) = thought_signature {
            function_call["thoughtSignature"] = Value::String(signature.to_string());
        }
        return super::gemini_response_tool_call_item_with_call_id(
            &json!({}),
            &function_call,
            Some(call_id),
        );
    }
    let mut part = json!({});
    if let Some(signature) = thought_signature {
        part["thoughtSignature"] = Value::String(signature.to_string());
    }
    super::gemini_response_tool_call_raw_item_with_call_id(&part, name, arguments, Some(call_id))
}

pub fn gemini_provider_core_stream_tool_call_added_item(
    call_id: &str,
    name: &str,
    thought_signature: Option<&str>,
) -> Option<Value> {
    let mut function_call = json!({
        "name": name,
    });
    if let Some(signature) = thought_signature {
        function_call["thoughtSignature"] = Value::String(signature.to_string());
    }
    super::gemini_response_tool_call_added_item_with_call_id(
        &json!({}),
        &function_call,
        Some(call_id),
    )
}

pub fn gemini_provider_core_stream_response_id_from_chunk(
    current_response_id: &str,
    value: &Value,
) -> Option<String> {
    value
        .get("responseId")
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)
        .filter(|_| current_response_id.starts_with("resp_gemini_"))
        .map(str::to_string)
}

pub fn gemini_provider_core_stream_chunk_metadata(
    current_response_id: &str,
    value: &Value,
) -> GeminiProviderCoreStreamChunkMetadata {
    GeminiProviderCoreStreamChunkMetadata {
        response_id: gemini_provider_core_stream_response_id_from_chunk(current_response_id, value),
        model: value
            .get("modelVersion")
            .or_else(|| value.get("model"))
            .and_then(Value::as_str)
            .map(str::to_string),
        usage: value
            .get("usageMetadata")
            .and_then(super::gemini_responses_usage),
        response_metadata: super::gemini_response_metadata(value),
        finish_reason: super::gemini_finish_reason(value),
    }
}

pub fn gemini_provider_core_stream_candidate_parts(value: &Value) -> Option<&[Value]> {
    value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
}

pub fn gemini_provider_core_stream_part_text(part: &Value) -> Option<&str> {
    part.get("text")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

pub fn gemini_provider_core_stream_part_is_thought(part: &Value) -> bool {
    part.get("thought")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn gemini_provider_core_stream_part_has_video_metadata(part: &Value) -> bool {
    part.get("videoMetadata").is_some()
}

pub fn gemini_provider_core_stream_part_function_call(part: &Value) -> Option<&Value> {
    part.get("functionCall")
}

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
        return gemini_mojo_value(input);
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
        return gemini_mojo_value(input);
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
        return gemini_mojo_value(input);
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
        return gemini_mojo_value(input);
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
        assistant["tool_calls"] = Value::Array(
            tool_calls
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
                .collect(),
        );
    }
    Some(assistant)
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
        return gemini_mojo_value(input);
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
        return gemini_mojo_value(input);
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
        return gemini_mojo_value(input);
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
        return gemini_mojo_value(input);
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
        return gemini_mojo_value(input);
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
    let Some((event_name, transformed)) = gemini_stream_event_from_generate_value(&value) else {
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

fn gemini_stream_event_from_generate_value(value: &Value) -> Option<(&'static str, Value)> {
    if let Some(function_call) = value.pointer("/candidates/0/content/parts/0/functionCall") {
        let args = function_call
            .get("args")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let arguments = serde_json::to_string(&args).ok()?;
        #[cfg(feature = "mojo")]
        let transformed = {
            let mut kernel_input = GeminiResponseKernelInput::new(
                GeminiResponseKernelOperation::FunctionCallArgumentsDelta,
            );
            kernel_input.call_id = function_call.get("id").and_then(Value::as_str);
            kernel_input.delta = Some(&arguments);
            gemini_mojo_value(kernel_input)
        };
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let transformed = {
            let mut kernel_input =
                GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamReasoningDelta);
            kernel_input.delta = Some(text);
            gemini_mojo_value(kernel_input)
        };
        #[cfg(not(feature = "mojo"))]
        let transformed = json!({
            "type":"response.reasoning_summary_text.delta",
            "delta":text,
        });
        return Some(("response.reasoning_summary_text.delta", transformed));
    }
    let text = value
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(Value::as_str)?;
    #[cfg(feature = "mojo")]
    let transformed = {
        let mut kernel_input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamTextDelta);
        kernel_input.delta = Some(text);
        gemini_mojo_value(kernel_input)
    };
    #[cfg(not(feature = "mojo"))]
    let transformed = json!({
        "type":"response.output_text.delta",
        "delta":text,
    });
    Some(("response.output_text.delta", transformed))
}
