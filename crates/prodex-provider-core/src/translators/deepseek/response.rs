use super::tooling::deepseek_responses_tool_call_item;
use crate::bridge::{
    provider_core_chat_compatible_created_at, provider_core_chat_compatible_responses_usage,
};
use serde_json::{Value, json};

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};

#[path = "response/metadata.rs"]
mod metadata;

#[cfg(feature = "mojo")]
pub(super) fn deepseek_stream_event_from_chat_value(value: &Value) -> Option<Vec<u8>> {
    let mut document = crate::mojo_json::Document::default();
    document.openai_chat_context(value, None);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    prodex_mojo_core::json::transform_deepseek_chat_stream_event(&document.nodes, raw)
        .unwrap_or_else(|error| panic!("Mojo DeepSeek stream event failed: {error:?}"))
}

#[cfg(not(feature = "mojo"))]
pub(super) fn deepseek_stream_event_from_chat_value(value: &Value) -> Option<Vec<u8>> {
    deepseek_stream_event_from_chat_value_rust(value)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn deepseek_stream_event_from_chat_value_rust(value: &Value) -> Option<Vec<u8>> {
    let delta = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))?;
    if let Some(tool_call) = delta
        .get("tool_calls")
        .and_then(Value::as_array)
        .and_then(|tool_calls| tool_calls.first())
    {
        let arguments = tool_call
            .get("function")
            .and_then(|function| function.get("arguments"))
            .and_then(Value::as_str)?;
        let mut transformed = json!({
            "type":"response.function_call_arguments.delta",
            "delta": arguments,
        });
        if let Some(call_id) = tool_call.get("id").and_then(Value::as_str)
            && let Some(object) = transformed.as_object_mut()
        {
            object.insert("call_id".to_string(), Value::String(call_id.to_string()));
        }
        return Some(
            format!("event: response.function_call_arguments.delta\ndata: {transformed}\n\n")
                .into_bytes(),
        );
    }
    let text = delta
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let transformed = json!({
        "type":"response.output_text.delta",
        "delta":text,
    });
    Some(format!("event: response.output_text.delta\ndata: {transformed}\n\n").into_bytes())
}

pub(super) fn deepseek_responses_value_from_chat_value(value: &Value) -> Value {
    let response_id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("chatcmpl_prodex");
    let created_at = value
        .get("created")
        .and_then(Value::as_u64)
        .unwrap_or_else(deepseek_created_at);
    let message = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"));
    let mut output = Vec::new();
    let mut tool_call_error = None;
    if let Some(text) = message
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        output.push(json!({
            "type":"message",
            "role":"assistant",
            "content":[{"type":"output_text","text":text}],
        }));
    }
    if let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(Value::as_array)
    {
        for tool_call in tool_calls {
            match deepseek_responses_tool_call_item(tool_call) {
                Ok(Some(item)) => output.push(item),
                Ok(None) => {}
                Err(error) => {
                    tool_call_error = Some(error);
                    break;
                }
            }
        }
    }
    #[cfg(feature = "mojo")]
    {
        let output = serde_json::to_string(&output).expect("DeepSeek response output serializes");
        let usage = value
            .get("usage")
            .and_then(deepseek_responses_usage)
            .map(|value| {
                serde_json::to_string(&value).expect("DeepSeek response usage serializes")
            });
        let metadata = metadata::deepseek_response_metadata(value, message).map(|value| {
            serde_json::to_string(&value).expect("DeepSeek response metadata serializes")
        });
        let error_message = tool_call_error.as_deref();
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::BufferedResponse);
        input.response_id = Some(response_id);
        input.created_at = created_at;
        input.model = Some(
            value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("deepseek-chat"),
        );
        input.output = Some(&output);
        input.usage = usage.as_deref();
        input.metadata = metadata.as_deref();
        input.error_code = error_message.map(|_| "invalid_tool_call_arguments");
        input.error_message = error_message;
        super::deepseek_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut response = json!({
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "model": value.get("model").and_then(Value::as_str).unwrap_or("deepseek-chat"),
            "output": output,
        });
        if let Some(error) = tool_call_error {
            response["status"] = Value::String("failed".to_string());
            response["error"] = json!({
                "code": "invalid_tool_call_arguments",
                "message": error,
            });
        }
        if let Some(usage) = value.get("usage").and_then(deepseek_responses_usage) {
            response["usage"] = usage;
        }
        if let Some(metadata) = metadata::deepseek_response_metadata(value, message) {
            response["metadata"] = metadata;
        }
        response
    }
}

pub(super) fn deepseek_responses_usage(usage: &Value) -> Option<Value> {
    provider_core_chat_compatible_responses_usage(usage, "deepseek")
}

pub(super) fn deepseek_created_at() -> u64 {
    provider_core_chat_compatible_created_at()
}
