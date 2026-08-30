//! DeepSeek SSE stream event translation.

use super::{deepseek_passthrough_endpoint, deepseek_stream_event_from_chat_value};
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};

#[path = "stream/response_values.rs"]
mod response_values;
pub use response_values::{
    deepseek_provider_core_stream_chat_assistant_message,
    deepseek_provider_core_stream_response_value,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeepSeekProviderCoreStreamChatToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub thought_signature: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeepSeekProviderCoreStreamToolCallDelta {
    pub index: usize,
    pub call_id: Option<String>,
    pub name: Option<String>,
    pub argument_delta: Option<String>,
    pub thought_signature: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeepSeekProviderCoreStreamChunkMetadata {
    pub model: Option<String>,
    pub created_at: Option<u64>,
    pub system_fingerprint: Option<String>,
    pub usage: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeepSeekProviderCoreStreamChoiceMetadata {
    pub logprobs: Option<Value>,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeepSeekProviderCoreStreamChoiceDelta {
    pub reasoning_content: Option<String>,
    pub refusal: Option<String>,
    pub annotations: Vec<Value>,
    pub content: Option<String>,
    pub tool_calls: Vec<Value>,
}

pub fn deepseek_provider_core_response_completed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let response = serde_json::to_string(response).expect("DeepSeek response serializes");
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseCompletedEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response = Some(&response);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "type": "response.completed",
            "sequence_number": sequence_number,
            "created_at": created_at,
            "response": response,
        })
    }
}

pub fn deepseek_provider_core_response_created_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseCreatedEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "type": "response.created",
            "sequence_number": sequence_number,
            "created_at": created_at,
            "response": {"id": response_id},
        })
    }
}

pub fn deepseek_provider_core_chat_stream_error(value: &Value) -> Option<(String, String)> {
    let error = value.get("error")?;
    if let Some(message) = error.as_str() {
        return Some(("provider_stream_error".to_string(), message.to_string()));
    }
    let code = error
        .get("type")
        .or_else(|| error.get("code"))
        .and_then(|code| {
            code.as_str()
                .map(str::to_string)
                .or_else(|| code.as_i64().map(|code| code.to_string()))
        })
        .unwrap_or_else(|| "provider_stream_error".to_string());
    let message = error
        .get("message")
        .or_else(|| error.get("detail"))
        .and_then(Value::as_str)
        .unwrap_or("Provider stream returned an embedded error")
        .to_string();
    Some((code, message))
}

pub fn deepseek_provider_core_validate_stream_tool_call_arguments(
    provider_label: &str,
    index: usize,
    name: Option<&str>,
    arguments: &str,
) -> Result<(), String> {
    let Some(name) = name.filter(|name| !name.trim().is_empty()) else {
        return Err(format!(
            "{provider_label} streamed a tool call without a function name at index {index}"
        ));
    };
    if arguments.trim().is_empty() {
        return Ok(());
    }
    serde_json::from_str::<Value>(arguments)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "{provider_label} streamed malformed JSON arguments for tool call `{name}` at index {index}: {error}"
            )
        })
}

pub fn deepseek_provider_core_validate_stream_tool_call_delta(
    provider_label: &str,
    existing_tool_call: bool,
    value: &Value,
) -> Result<(), String> {
    if value.get("function").and_then(Value::as_object).is_none() && !existing_tool_call {
        return Err(format!(
            "{provider_label} streamed a tool call without a function object"
        ));
    }
    Ok(())
}

pub fn deepseek_provider_core_stream_tool_call_delta(
    value: &Value,
) -> DeepSeekProviderCoreStreamToolCallDelta {
    let index = value
        .get("index")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or(0);
    let function = value.get("function");
    DeepSeekProviderCoreStreamToolCallDelta {
        index,
        call_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        name: function
            .and_then(|function| function.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        argument_delta: function
            .and_then(|function| function.get("arguments"))
            .and_then(Value::as_str)
            .filter(|arguments| !arguments.is_empty())
            .map(str::to_string),
        thought_signature: crate::bridge::provider_core_chat_compatible_tool_call_thought_signature(
            value,
        ),
    }
}

pub fn deepseek_provider_core_incremental_tool_argument_delta(
    _previous: &str,
    incoming: &str,
) -> Option<String> {
    (!incoming.is_empty()).then(|| incoming.to_string())
}

pub const DEEPSEEK_PROVIDER_CORE_FIRST_EVENT_RETRY_LIMIT: u8 = 1;

pub fn deepseek_provider_core_first_event_retry_allowed(
    attempted_retries: u8,
    first_event_committed: bool,
) -> bool {
    !first_event_committed && attempted_retries < DEEPSEEK_PROVIDER_CORE_FIRST_EVENT_RETRY_LIMIT
}

pub fn deepseek_provider_core_stream_fallback_tool_call_id(
    provider_label: &str,
    request_id: u64,
    index: usize,
) -> String {
    format!("call_{provider_label}_{request_id}_{index}")
}

pub fn deepseek_provider_core_stream_output_text_item_id(
    provider_label: &str,
    request_id: u64,
) -> String {
    format!("msg_{provider_label}_{request_id}")
}

pub fn deepseek_provider_core_stream_fallback_response_id(
    provider_label: &str,
    request_id: u64,
) -> String {
    format!("resp_{provider_label}_{request_id}")
}

pub fn deepseek_provider_core_stream_response_id_from_chunk(
    provider_label: &str,
    current_response_id: &str,
    value: &Value,
) -> Option<String> {
    let fallback_response_id_prefix = format!("resp_{provider_label}_");
    value
        .get("id")
        .and_then(Value::as_str)
        .filter(|_| current_response_id.starts_with(&fallback_response_id_prefix))
        .map(str::to_string)
}

pub fn deepseek_provider_core_stream_chunk_metadata(
    value: &Value,
    provider_label: &str,
) -> DeepSeekProviderCoreStreamChunkMetadata {
    DeepSeekProviderCoreStreamChunkMetadata {
        model: value
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        created_at: value.get("created").and_then(Value::as_u64),
        system_fingerprint: value
            .get("system_fingerprint")
            .and_then(Value::as_str)
            .filter(|system_fingerprint| !system_fingerprint.is_empty())
            .map(str::to_string),
        usage: value.get("usage").and_then(|usage| {
            crate::bridge::provider_core_chat_compatible_responses_usage(usage, provider_label)
        }),
    }
}

pub fn deepseek_provider_core_stream_first_choice(value: &Value) -> Option<&Value> {
    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
}

pub fn deepseek_provider_core_stream_choice_metadata(
    choice: &Value,
) -> DeepSeekProviderCoreStreamChoiceMetadata {
    DeepSeekProviderCoreStreamChoiceMetadata {
        logprobs: choice
            .get("logprobs")
            .filter(|logprobs| !logprobs.is_null())
            .cloned(),
        finish_reason: choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

pub fn deepseek_provider_core_stream_choice_delta(
    choice: &Value,
) -> DeepSeekProviderCoreStreamChoiceDelta {
    let Some(delta) = choice.get("delta") else {
        return DeepSeekProviderCoreStreamChoiceDelta::default();
    };
    DeepSeekProviderCoreStreamChoiceDelta {
        reasoning_content: delta
            .get("reasoning_content")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        refusal: delta
            .get("refusal")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        annotations: delta
            .get("annotations")
            .and_then(Value::as_array)
            .map(|annotations| annotations.to_vec())
            .unwrap_or_default(),
        content: delta
            .get("content")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        tool_calls: delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .map(|tool_calls| tool_calls.to_vec())
            .unwrap_or_default(),
    }
}

pub fn deepseek_provider_core_stream_response_metadata(
    provider_label: &str,
    logprobs: Option<Value>,
    reasoning_content: &str,
    refusal: &str,
    annotations: &[Value],
    finish_reason: Option<&str>,
    system_fingerprint: Option<&str>,
) -> Option<Value> {
    let mut metadata = serde_json::Map::new();
    if let Some(logprobs) = logprobs {
        metadata.insert("logprobs".to_string(), logprobs);
    }
    if !reasoning_content.is_empty() {
        metadata.insert(
            "reasoning_content".to_string(),
            Value::String(reasoning_content.to_string()),
        );
    }
    if !refusal.is_empty() {
        metadata.insert("refusal".to_string(), Value::String(refusal.to_string()));
    }
    if !annotations.is_empty() {
        metadata.insert(
            "annotations".to_string(),
            Value::Array(annotations.to_vec()),
        );
    }
    if let Some(finish_reason) = finish_reason {
        metadata.insert(
            "finish_reason".to_string(),
            Value::String(finish_reason.to_string()),
        );
    }
    if let Some(system_fingerprint) = system_fingerprint {
        metadata.insert(
            "system_fingerprint".to_string(),
            Value::String(system_fingerprint.to_string()),
        );
    }
    (!metadata.is_empty()).then(|| serde_json::json!({ provider_label: metadata }))
}

pub fn deepseek_provider_core_stream_output_text_item(text: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputTextItem);
        input.delta = Some(text);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": text,
            }],
        })
    }
}

pub fn deepseek_provider_core_stream_tool_call_added_item(
    call_id: &str,
    flat_name: &str,
) -> Option<Value> {
    #[cfg(feature = "mojo")]
    {
        if flat_name == "tool_search" {
            return None;
        }
        if flat_name == "apply_patch" {
            let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::CustomToolCallItem);
            input.call_id = Some(call_id);
            input.name = Some(flat_name);
            input.input = Some("");
            return Some(super::deepseek_mojo_value(input));
        }
        let (namespace, name) =
            crate::bridge::provider_core_split_flat_namespace_tool_name(flat_name);
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::AddedFunctionCallItem);
        input.call_id = Some(call_id);
        input.name = Some(&name);
        input.namespace = namespace.as_deref();
        Some(super::deepseek_mojo_value(input))
    }
    #[cfg(not(feature = "mojo"))]
    {
        if flat_name == "tool_search" {
            return None;
        }
        if flat_name == "apply_patch" {
            return Some(serde_json::json!({
                "type": "custom_tool_call",
                "call_id": call_id,
                "name": flat_name,
                "input": "",
            }));
        }
        let (namespace, name) =
            crate::bridge::provider_core_split_flat_namespace_tool_name(flat_name);
        let mut item = serde_json::json!({
            "type": "function_call",
            "call_id": call_id,
            "name": name,
        });
        if let Some(namespace) = namespace {
            item["namespace"] = Value::String(namespace);
        }
        Some(item)
    }
}

pub fn deepseek_provider_core_stream_tool_call_item(
    call_id: &str,
    flat_name: &str,
    arguments: &str,
    thought_signature: Option<&str>,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        if flat_name == "tool_search" {
            let arguments =
                serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| serde_json::json!({}));
            let arguments =
                serde_json::to_string(&arguments).expect("DeepSeek tool arguments serialize");
            let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ToolSearchItem);
            input.call_id = Some(call_id);
            input.arguments = Some(&arguments);
            return super::deepseek_mojo_value(input);
        }
        if flat_name == "apply_patch" {
            let input_value =
                crate::gemini_bridge::gemini_provider_core_custom_tool_input_from_arguments(
                    arguments,
                );
            let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::CustomToolCallItem);
            input.call_id = Some(call_id);
            input.name = Some(flat_name);
            input.input = Some(&input_value);
            return super::deepseek_mojo_value(input);
        }
        let (namespace, name) =
            crate::bridge::provider_core_split_flat_namespace_tool_name(flat_name);
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::FunctionCallItem);
        input.call_id = Some(call_id);
        input.name = Some(&name);
        input.arguments = Some(arguments);
        input.namespace = namespace.as_deref();
        input.signature = thought_signature;
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        if flat_name == "tool_search" {
            let arguments =
                serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| serde_json::json!({}));
            return serde_json::json!({
                "type": "tool_search_call",
                "call_id": call_id,
                "execution": "client",
                "arguments": arguments,
            });
        }
        if flat_name == "apply_patch" {
            return serde_json::json!({
                "type": "custom_tool_call",
                "call_id": call_id,
                "name": flat_name,
                "input": crate::gemini_bridge::gemini_provider_core_custom_tool_input_from_arguments(arguments),
            });
        }
        let (namespace, name) =
            crate::bridge::provider_core_split_flat_namespace_tool_name(flat_name);
        let mut item = serde_json::json!({
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        });
        if let Some(namespace) = namespace {
            item["namespace"] = Value::String(namespace);
        }
        if let Some(signature) = thought_signature {
            item["gemini_thought_signature"] = Value::String(signature.to_string());
        }
        item
    }
}

pub fn deepseek_provider_core_stream_function_call_arguments_delta_source(
    call_id: &str,
    arguments: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            DeepSeekKernelInput::new(DeepSeekKernelOperation::FunctionCallArgumentsDeltaSource);
        input.call_id = Some(call_id);
        input.arguments = Some(arguments);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": call_id,
                        "function": {
                            "arguments": arguments,
                        }
                    }]
                }
            }]
        })
    }
}

pub fn deepseek_provider_core_stream_text_delta_source(text: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::TextDeltaSource);
        input.delta = Some(text);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "choices": [{
                "delta": {
                    "content": text,
                }
            }]
        })
    }
}

pub fn deepseek_provider_core_output_item_added_event(sequence_number: u64, item: &Value) -> Value {
    #[cfg(feature = "mojo")]
    {
        let item = serde_json::to_string(item).expect("DeepSeek output item serializes");
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputItemAddedEvent);
        input.sequence_number = sequence_number;
        input.item = Some(&item);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "type": "response.output_item.added",
            "sequence_number": sequence_number,
            "item": item,
        })
    }
}

pub fn deepseek_provider_core_function_call_arguments_delta_event(
    sequence_number: u64,
    call_id: &str,
    arguments: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            DeepSeekKernelInput::new(DeepSeekKernelOperation::FunctionCallArgumentsDeltaEvent);
        input.sequence_number = sequence_number;
        input.call_id = Some(call_id);
        input.delta = Some(arguments);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "type": "response.function_call_arguments.delta",
            "sequence_number": sequence_number,
            "call_id": call_id,
            "delta": arguments,
        })
    }
}

pub fn deepseek_provider_core_output_text_delta_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    delta: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputTextDeltaEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        input.delta = Some(delta);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "type": "response.output_text.delta",
            "sequence_number": sequence_number,
            "created_at": created_at,
            "response_id": response_id,
            "delta": delta,
        })
    }
}

pub fn deepseek_provider_core_output_item_done_event(sequence_number: u64, item: &Value) -> Value {
    #[cfg(feature = "mojo")]
    {
        let item = serde_json::to_string(item).expect("DeepSeek output item serializes");
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputItemDoneEvent);
        input.sequence_number = sequence_number;
        input.item = Some(&item);
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        serde_json::json!({
            "type": "response.output_item.done",
            "sequence_number": sequence_number,
            "item": item,
        })
    }
}

pub(super) fn deepseek_transform_stream_event(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if deepseek_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            input.body,
        );
    }
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            format!(
                "DeepSeek translator does not support {}",
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
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "DeepSeek SSE event must use data: <json> framing",
        );
    };
    if data == "[DONE]" {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            b"event: response.completed\ndata: {}\n\n".to_vec(),
        );
    }
    let value: Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                format!("failed to parse DeepSeek SSE JSON: {error}"),
            );
        }
    };
    let Some((event_name, transformed)) = deepseek_stream_event_from_chat_value(&value) else {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "DeepSeek SSE event does not contain a supported text or function-call delta",
        );
    };
    let body = format!("event: {event_name}\ndata: {}\n\n", transformed);
    ProviderTransformResult::lossless(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::OpenAiResponses,
        body.into_bytes(),
    )
}
