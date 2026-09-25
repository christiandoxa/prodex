//! Gemini stream payload extraction and deterministic shaping.

use super::{
    GeminiProviderCoreStreamChunkMetadata, GeminiProviderCoreStreamFunctionCallDelta,
    GeminiProviderCoreStreamToolCall,
};
use serde_json::{Value, json};

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{GeminiResponseKernelInput, GeminiResponseKernelOperation};

#[cfg(feature = "mojo")]
fn gemini_stream_identifier(
    operation: GeminiResponseKernelOperation,
    request_id: u64,
    index: usize,
) -> String {
    let mut input = GeminiResponseKernelInput::new(operation);
    input.sequence_number = request_id;
    input.summary_index = index as u64;
    super::gemini_mojo_value(input)
        .as_str()
        .unwrap_or_else(|| panic!("Mojo Gemini stream identifier kernel returned a non-string"))
        .to_string()
}

pub fn gemini_provider_core_stream_output_text_item_id(request_id: u64) -> String {
    #[cfg(feature = "mojo")]
    return gemini_stream_identifier(
        GeminiResponseKernelOperation::StreamOutputTextItemId,
        request_id,
        0,
    );
    #[cfg(not(feature = "mojo"))]
    format!("msg_gemini_{request_id}")
}

pub fn gemini_provider_core_stream_media_item_id(request_id: u64) -> String {
    #[cfg(feature = "mojo")]
    return gemini_stream_identifier(
        GeminiResponseKernelOperation::StreamMediaItemId,
        request_id,
        0,
    );
    #[cfg(not(feature = "mojo"))]
    format!("msg_gemini_media_{request_id}")
}

pub fn gemini_provider_core_stream_citation_item_id(request_id: u64) -> String {
    #[cfg(feature = "mojo")]
    return gemini_stream_identifier(
        GeminiResponseKernelOperation::StreamCitationItemId,
        request_id,
        0,
    );
    #[cfg(not(feature = "mojo"))]
    format!("msg_gemini_citations_{request_id}")
}

pub fn gemini_provider_core_stream_fallback_response_id(request_id: u64) -> String {
    #[cfg(feature = "mojo")]
    return gemini_stream_identifier(
        GeminiResponseKernelOperation::StreamFallbackResponseId,
        request_id,
        0,
    );
    #[cfg(not(feature = "mojo"))]
    format!("resp_gemini_{request_id}")
}

pub fn gemini_provider_core_stream_fallback_tool_call_id(request_id: u64, index: usize) -> String {
    #[cfg(feature = "mojo")]
    return gemini_stream_identifier(
        GeminiResponseKernelOperation::StreamFallbackToolCallId,
        request_id,
        index,
    );
    #[cfg(not(feature = "mojo"))]
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
    let name = value.get("name").and_then(Value::as_str);
    let arguments = value.get("args").cloned().unwrap_or_else(|| json!({}));
    let arguments = serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string());
    #[cfg(feature = "mojo")]
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamFunctionCallDelta);
        input.call_id = explicit_call_id.as_deref();
        input.name = name;
        // Operation-local presence bit keeps `None` distinct from an explicit empty name.
        input.reason_present = name.is_some();
        input.arguments = Some(&arguments);
        let value = super::gemini_mojo_value(input);
        GeminiProviderCoreStreamFunctionCallDelta {
            explicit_call_id: value
                .get("explicit_call_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("Mojo Gemini stream delta kernel omitted name"))
                .to_string(),
            arguments: value
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("Mojo Gemini stream delta kernel omitted arguments"))
                .to_string(),
        }
    }
    #[cfg(not(feature = "mojo"))]
    GeminiProviderCoreStreamFunctionCallDelta {
        explicit_call_id,
        name: name.unwrap_or("tool_call").to_string(),
        arguments,
    }
}

#[cfg(feature = "mojo")]
pub(crate) fn gemini_provider_core_stream_chat_tool_call_item(
    tool_call: &GeminiProviderCoreStreamToolCall,
) -> Value {
    let mut input =
        GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ChatFunctionCallItem);
    input.call_id = Some(&tool_call.call_id);
    input.name = Some(&tool_call.name);
    input.arguments = Some(&tool_call.arguments);
    input.signature = tool_call.thought_signature.as_deref();
    super::gemini_mojo_value(input)
}

pub fn gemini_provider_core_stream_tool_call(
    request_id: u64,
    index: usize,
    call_id: Option<&str>,
    name: Option<&str>,
    arguments: &str,
    thought_signature: Option<&str>,
) -> GeminiProviderCoreStreamToolCall {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamToolCall);
        input.sequence_number = request_id;
        input.summary_index = index as u64;
        input.call_id = call_id;
        input.name = name;
        // Operation-local presence bit keeps `None` distinct from an explicit empty name.
        input.reason_present = name.is_some();
        input.arguments = Some(arguments);
        input.signature = thought_signature;
        let value = super::gemini_mojo_value(input);
        GeminiProviderCoreStreamToolCall {
            call_id: value["call_id"]
                .as_str()
                .unwrap_or_else(|| panic!("Mojo Gemini stream tool-call kernel omitted call_id"))
                .to_string(),
            name: value["name"]
                .as_str()
                .unwrap_or_else(|| panic!("Mojo Gemini stream tool-call kernel omitted name"))
                .to_string(),
            arguments: value["arguments"]
                .as_str()
                .unwrap_or_else(|| panic!("Mojo Gemini stream tool-call kernel omitted arguments"))
                .to_string(),
            thought_signature: value
                .get("thought_signature")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    {
        let mut input = GeminiResponseKernelInput::new(
            GeminiResponseKernelOperation::StreamShouldEmitArgumentsDelta,
        );
        input.name = Some(name);
        super::gemini_mojo_value(input)
            .as_bool()
            .unwrap_or_else(|| panic!("Mojo Gemini stream emission kernel returned a non-boolean"))
    }
    #[cfg(not(feature = "mojo"))]
    !matches!(name, "tool_search" | "apply_patch")
}

pub fn gemini_provider_core_function_call_arguments_delta_event_with_thought_signature(
    mut event: Value,
    thought_signature: Option<&str>,
) -> Value {
    #[cfg(feature = "mojo")]
    if let Some(signature) = thought_signature
        && event.get("type").and_then(Value::as_str)
            == Some("response.function_call_arguments.delta")
        && let Some(delta) = event.get("delta").and_then(Value::as_str)
        && event.as_object().is_some_and(|object| {
            object.keys().all(|key| {
                matches!(
                    key.as_str(),
                    "type" | "sequence_number" | "call_id" | "delta"
                )
            })
        })
    {
        let has_sequence_number = event.get("sequence_number").is_some();
        let sequence_number = event.get("sequence_number").and_then(Value::as_u64);
        if !has_sequence_number || sequence_number.is_some() {
            let operation = if has_sequence_number {
                GeminiResponseKernelOperation::FunctionCallArgumentsDelta
            } else {
                GeminiResponseKernelOperation::FunctionCallArgumentsDeltaWithoutSequence
            };
            let mut input = GeminiResponseKernelInput::new(operation);
            input.sequence_number = sequence_number.unwrap_or_default();
            input.call_id = event.get("call_id").and_then(Value::as_str);
            input.delta = Some(delta);
            input.signature = Some(signature);
            return super::gemini_mojo_value(input);
        }
    }
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
        return super::super::gemini_response_tool_call_item_with_call_id(
            &json!({}),
            &function_call,
            Some(call_id),
        );
    }
    let mut part = json!({});
    if let Some(signature) = thought_signature {
        part["thoughtSignature"] = Value::String(signature.to_string());
    }
    super::super::gemini_response_tool_call_raw_item_with_call_id(
        &part,
        name,
        arguments,
        Some(call_id),
    )
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
    super::super::gemini_response_tool_call_added_item_with_call_id(
        &json!({}),
        &function_call,
        Some(call_id),
    )
}

pub fn gemini_provider_core_stream_response_id_from_chunk(
    current_response_id: &str,
    value: &Value,
) -> Option<String> {
    #[cfg(feature = "mojo")]
    {
        let candidate = value
            .get("responseId")
            .or_else(|| value.get("id"))
            .and_then(Value::as_str);
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamResponseId);
        input.response_id = Some(current_response_id);
        input.call_id = candidate;
        super::gemini_mojo_value(input).as_str().map(str::to_string)
    }
    #[cfg(not(feature = "mojo"))]
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
            .and_then(super::super::gemini_responses_usage),
        response_metadata: super::super::gemini_response_metadata(value),
        finish_reason: super::super::gemini_finish_reason(value),
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
