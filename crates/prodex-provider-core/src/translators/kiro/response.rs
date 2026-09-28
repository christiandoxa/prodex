//! Kiro provider response compatibility helpers.

use super::stream::kiro_provider_core_stream_content_text;
use prodex_mojo_core::MojoError;
use prodex_mojo_core::rich::KIRO_RESPONSE_MAX_BYTES;
use serde_json::{Value, json};

use super::stream::{kiro_mojo_body, kiro_mojo_value};
use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation};

/// Legacy value-returning adapter. Use the fallible variant for untrusted responses.
///
/// # Panics
///
/// Panics if the bounded Mojo rewrite fails.
pub fn kiro_provider_core_chat_completion_value_from_response(
    response: &Value,
    request_id: u64,
) -> Value {
    kiro_provider_core_try_chat_completion_value_from_response(response, request_id)
        .unwrap_or_else(|error| panic!("Mojo Kiro raw response rewrite failed: {error:?}"))
}

/// Rewrites a Kiro response as Chat Completions or returns a bounded kernel error.
pub fn kiro_provider_core_try_chat_completion_value_from_response(
    response: &Value,
    request_id: u64,
) -> Result<Value, MojoError> {
    let canonical = serde_json::to_string(response).map_err(|_| MojoError::InvalidInput)?;
    if canonical.len() > KIRO_RESPONSE_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let body = prodex_mojo_core::rich::kiro_rewrite_chat_response_json(&canonical, request_id)?;
    serde_json::from_slice(&body).map_err(|_| MojoError::InvalidOutput)
}

pub fn kiro_provider_core_apply_response_runtime_metadata(
    response: &mut Value,
    profile_name: &str,
    requested_model: Option<&str>,
    created_at: Option<u64>,
) {
    if let Some(created_at) = created_at {
        response["created_at"] = Value::from(created_at);
    }
    response["metadata"]["kiro"]["profile_name"] = Value::String(profile_name.to_string());
    if let Some(model) = requested_model.filter(|model| !model.is_empty()) {
        response["requested_model"] = Value::String(model.to_string());
    }
}

pub fn kiro_provider_core_response_has_tool_calls(response: &Value) -> bool {
    let output_types = response
        .get("output")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| item.get("type").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let output_types =
        serde_json::to_string(&output_types).expect("Kiro response output types serialize");
    let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseHasToolCalls);
    input.output = Some(&output_types);
    serde_json::from_slice::<bool>(&kiro_mojo_body(input))
        .expect("Mojo Kiro tool-call presence is a JSON boolean")
}

pub fn kiro_provider_core_model_list_value(model_catalog: &[Value]) -> Value {
    let catalog = serde_json::to_string(model_catalog).expect("Kiro model catalog serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::ModelList);
    input.output = Some(&catalog);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_model_value_or_not_found(
    model_catalog: &[Value],
    model_id: &str,
) -> (u16, Value) {
    if let Some(model) = model_catalog.iter().find(|model| {
        model
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.eq_ignore_ascii_case(model_id))
    }) {
        return (200, model.clone());
    }
    let mut input = KiroKernelInput::new(KiroKernelOperation::ModelNotFound);
    input.model = Some(model_id);
    (404, kiro_mojo_value(input))
}

pub fn kiro_provider_core_invalid_request_error_value(message: &str, code: &str) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::InvalidRequestError);
    input.content = Some(message);
    input.status = Some(code);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_unsupported_path_error_value(path: &str) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::UnsupportedPathError);
    input.content = Some(path);
    input.status = Some("unsupported_path");
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_chat_completion_finish_reason(
    response: &Value,
    has_tool_calls: bool,
) -> &'static str {
    let mut input = KiroKernelInput::new(KiroKernelOperation::FinishReason);
    input.has_tool_calls = has_tool_calls;
    input.incomplete_reason = response
        .pointer("/incomplete_details/reason")
        .and_then(Value::as_str);
    let reason: String = serde_json::from_slice(&kiro_mojo_body(input))
        .expect("Mojo Kiro finish reason is a JSON string");
    match reason.as_str() {
        "tool_calls" => "tool_calls",
        "length" => "length",
        _ => "stop",
    }
}

pub fn kiro_provider_core_chat_completion_finish_reason_from_response(
    response: &Value,
) -> &'static str {
    kiro_provider_core_chat_completion_finish_reason(
        response,
        kiro_provider_core_response_has_tool_calls(response),
    )
}

pub fn kiro_provider_core_anthropic_message_value_from_response(
    response: &Value,
    requested_model: &str,
) -> Value {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    let text = output
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .and_then(|item| item.get("content"))
        .and_then(kiro_provider_core_stream_content_text)
        .unwrap_or_default();
    let tool_use_blocks = output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .map(|item| {
            let arguments = item
                .get("arguments")
                .and_then(Value::as_str)
                .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
                .unwrap_or_else(|| json!({}));
            let arguments =
                serde_json::to_string(&arguments).expect("Kiro Anthropic tool input serializes");
            let mut input = KiroKernelInput::new(KiroKernelOperation::AnthropicToolUseBlock);
            input.call_id = Some(
                item.get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("call_kiro"),
            );
            input.name = Some(
                item.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool_call"),
            );
            input.input = Some(&arguments);
            let mut block = kiro_mojo_value(input);
            // Preserve raw JSON fields that the string-only ABI cannot represent.
            if let Some(call_id) = item.get("call_id") {
                block["id"] = call_id.clone();
            }
            if let Some(name) = item.get("name") {
                block["name"] = name.clone();
            }
            block
        })
        .collect::<Vec<_>>();
    let has_tool_calls = !tool_use_blocks.is_empty();
    let tool_calls = has_tool_calls.then(|| {
        serde_json::to_string(&tool_use_blocks).expect("Kiro Anthropic tool blocks serialize")
    });
    let response_id = response
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("msg_kiro");
    let usage = response.get("usage");
    let used = usage
        .and_then(|usage| usage.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let size = usage
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reason = response
        .pointer("/incomplete_details/reason")
        .or_else(|| response.pointer("/metadata/kiro/stop_reason"))
        .and_then(Value::as_str);
    let mut input = KiroKernelInput::new(KiroKernelOperation::AnthropicResponse);
    input.response_id = Some(response_id);
    input.requested_model = Some(requested_model);
    input.content = (!text.is_empty()).then_some(text.as_str());
    input.tool_calls = tool_calls.as_deref();
    input.has_tool_calls = has_tool_calls;
    input.reason = reason;
    input.used = used;
    input.size = size;
    let mut message = kiro_mojo_value(input);
    // Copy through raw values without normalizing their JSON types at the ABI.
    if let Some(id) = response.get("id") {
        message["id"] = id.clone();
    }
    for field in ["input_tokens", "output_tokens"] {
        if let Some(value) = usage.and_then(|usage| usage.get(field)) {
            message["usage"][field] = value.clone();
        }
    }
    message
}
