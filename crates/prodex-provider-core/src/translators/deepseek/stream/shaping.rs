use super::{
    DeepSeekProviderCoreStreamChoiceDelta, DeepSeekProviderCoreStreamChoiceMetadata,
    DeepSeekProviderCoreStreamChunkMetadata, DeepSeekProviderCoreStreamToolCallDelta,
};
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};
#[cfg(feature = "mojo")]
use serde::de::DeserializeOwned;
use serde_json::Value;

#[cfg(test)]
#[path = "shaping_tests.rs"]
mod tests;

#[cfg(feature = "mojo")]
fn deepseek_provider_core_stream_projection<T: DeserializeOwned>(
    operation: DeepSeekKernelOperation,
    value: &Value,
) -> T {
    let source = serde_json::to_string(value).expect("DeepSeek stream source serializes");
    let mut input = DeepSeekKernelInput::new(operation);
    input.input = Some(&source);
    serde_json::from_value(super::super::deepseek_mojo_value(input))
        .expect("DeepSeek stream projection matches the typed contract")
}
pub fn deepseek_provider_core_response_completed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    let response = serde_json::to_string(response).expect("DeepSeek response serializes");
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseCompletedEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.response = Some(&response);
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_response_created_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Value {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseCreatedEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.response_id = Some(response_id);
    super::super::deepseek_mojo_value(input)
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
    #[cfg(feature = "mojo")]
    {
        let mut projected: DeepSeekProviderCoreStreamToolCallDelta =
            deepseek_provider_core_stream_projection(
                DeepSeekKernelOperation::StreamToolCallDelta,
                value,
            );
        if projected.argument_delta.as_deref() == Some("") {
            projected.argument_delta = None;
        }
        if projected
            .thought_signature
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            projected.thought_signature = None;
        }
        projected
    }
    #[cfg(not(feature = "mojo"))]
    {
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
            thought_signature:
                crate::bridge::provider_core_chat_compatible_tool_call_thought_signature(value),
        }
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
    #[cfg(feature = "mojo")]
    {
        let projected: Value = deepseek_provider_core_stream_projection(
            DeepSeekKernelOperation::StreamChunkMetadata,
            value,
        );
        DeepSeekProviderCoreStreamChunkMetadata {
            model: projected
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string),
            created_at: projected.get("created").and_then(Value::as_u64),
            system_fingerprint: projected
                .get("system_fingerprint")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            usage: projected.get("usage").and_then(|usage| {
                crate::bridge::provider_core_chat_compatible_responses_usage(usage, provider_label)
            }),
        }
    }
    #[cfg(not(feature = "mojo"))]
    DeepSeekProviderCoreStreamChunkMetadata {
        model: value
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        created_at: value.get("created").and_then(Value::as_u64),
        system_fingerprint: value
            .get("system_fingerprint")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
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
    #[cfg(feature = "mojo")]
    {
        let mut projected: DeepSeekProviderCoreStreamChoiceMetadata =
            deepseek_provider_core_stream_projection(
                DeepSeekKernelOperation::StreamChoiceMetadata,
                choice,
            );
        if projected.logprobs.as_ref().is_some_and(Value::is_null) {
            projected.logprobs = None;
        }
        projected
    }
    #[cfg(not(feature = "mojo"))]
    DeepSeekProviderCoreStreamChoiceMetadata {
        logprobs: choice
            .get("logprobs")
            .filter(|value| !value.is_null())
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
    #[cfg(feature = "mojo")]
    {
        let mut projected: DeepSeekProviderCoreStreamChoiceDelta =
            deepseek_provider_core_stream_projection(
                DeepSeekKernelOperation::StreamChoiceDelta,
                choice,
            );
        for text in [
            &mut projected.reasoning_content,
            &mut projected.refusal,
            &mut projected.content,
        ] {
            if text.as_deref() == Some("") {
                *text = None;
            }
        }
        projected
    }
    #[cfg(not(feature = "mojo"))]
    {
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
                .map(|items| items.to_vec())
                .unwrap_or_default(),
            content: delta
                .get("content")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_string),
            tool_calls: delta
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(|items| items.to_vec())
                .unwrap_or_default(),
        }
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
    #[cfg(feature = "mojo")]
    {
        let logprobs = logprobs
            .as_ref()
            .map(|value| serde_json::to_string(value).expect("DeepSeek logprobs serialize"));
        let annotations = (!annotations.is_empty()).then(|| {
            serde_json::to_string(annotations).expect("DeepSeek stream annotations serialize")
        });
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::StreamResponseMetadata);
        input.role = Some(provider_label);
        input.metadata = logprobs.as_deref();
        input.reasoning_content = (!reasoning_content.is_empty()).then_some(reasoning_content);
        input.content = (!refusal.is_empty()).then_some(refusal);
        input.item = annotations.as_deref();
        input.name = finish_reason;
        input.signature = system_fingerprint;
        let metadata = super::super::deepseek_mojo_value(input);
        (!metadata.is_null()).then_some(metadata)
    }
    #[cfg(not(feature = "mojo"))]
    {
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
        if let Some(value) = finish_reason {
            metadata.insert(
                "finish_reason".to_string(),
                Value::String(value.to_string()),
            );
        }
        if let Some(value) = system_fingerprint {
            metadata.insert(
                "system_fingerprint".to_string(),
                Value::String(value.to_string()),
            );
        }
        (!metadata.is_empty()).then(|| serde_json::json!({ provider_label: metadata }))
    }
}

pub fn deepseek_provider_core_stream_output_text_item(text: &str) -> Value {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputTextItem);
    input.delta = Some(text);
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_stream_tool_call_added_item(
    call_id: &str,
    flat_name: &str,
) -> Option<Value> {
    if flat_name == "tool_search" {
        return None;
    }
    if flat_name == "apply_patch" {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::CustomToolCallItem);
        input.call_id = Some(call_id);
        input.name = Some(flat_name);
        input.input = Some("");
        return Some(super::super::deepseek_mojo_value(input));
    }
    let (namespace, name) = crate::bridge::provider_core_split_flat_namespace_tool_name(flat_name);
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::AddedFunctionCallItem);
    input.call_id = Some(call_id);
    input.name = Some(&name);
    input.namespace = namespace.as_deref();
    Some(super::super::deepseek_mojo_value(input))
}

pub fn deepseek_provider_core_stream_tool_call_item(
    call_id: &str,
    flat_name: &str,
    arguments: &str,
    thought_signature: Option<&str>,
) -> Value {
    if flat_name == "tool_search" {
        let arguments =
            serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| serde_json::json!({}));
        let arguments =
            serde_json::to_string(&arguments).expect("DeepSeek tool arguments serialize");
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ToolSearchItem);
        input.call_id = Some(call_id);
        input.arguments = Some(&arguments);
        return super::super::deepseek_mojo_value(input);
    }
    if flat_name == "apply_patch" {
        let input_value =
            crate::gemini_bridge::gemini_provider_core_custom_tool_input_from_arguments(arguments);
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::CustomToolCallItem);
        input.call_id = Some(call_id);
        input.name = Some(flat_name);
        input.input = Some(&input_value);
        return super::super::deepseek_mojo_value(input);
    }
    let (namespace, name) = crate::bridge::provider_core_split_flat_namespace_tool_name(flat_name);
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::FunctionCallItem);
    input.call_id = Some(call_id);
    input.name = Some(&name);
    input.arguments = Some(arguments);
    input.namespace = namespace.as_deref();
    input.signature = thought_signature;
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_stream_function_call_arguments_delta_source(
    call_id: &str,
    arguments: &str,
) -> Value {
    let mut input =
        DeepSeekKernelInput::new(DeepSeekKernelOperation::FunctionCallArgumentsDeltaSource);
    input.call_id = Some(call_id);
    input.arguments = Some(arguments);
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_stream_text_delta_source(text: &str) -> Value {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::TextDeltaSource);
    input.delta = Some(text);
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_output_item_added_event(sequence_number: u64, item: &Value) -> Value {
    let item = serde_json::to_string(item).expect("DeepSeek output item serializes");
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputItemAddedEvent);
    input.sequence_number = sequence_number;
    input.item = Some(&item);
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_function_call_arguments_delta_event(
    sequence_number: u64,
    call_id: &str,
    arguments: &str,
) -> Value {
    let mut input =
        DeepSeekKernelInput::new(DeepSeekKernelOperation::FunctionCallArgumentsDeltaEvent);
    input.sequence_number = sequence_number;
    input.call_id = Some(call_id);
    input.delta = Some(arguments);
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_output_text_delta_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    delta: &str,
) -> Value {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputTextDeltaEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.response_id = Some(response_id);
    input.delta = Some(delta);
    super::super::deepseek_mojo_value(input)
}

pub fn deepseek_provider_core_output_item_done_event(sequence_number: u64, item: &Value) -> Value {
    let item = serde_json::to_string(item).expect("DeepSeek output item serializes");
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::OutputItemDoneEvent);
    input.sequence_number = sequence_number;
    input.item = Some(&item);
    super::super::deepseek_mojo_value(input)
}
