//! Kiro provider response compatibility helpers.

use prodex_mojo_core::MojoError;
use prodex_mojo_core::rich::{CatalogModel, KIRO_RESPONSE_MAX_BYTES, resolve_catalog_model_exact};
use serde_json::Value;

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
    let indexed = model_catalog
        .iter()
        .enumerate()
        .filter_map(|(index, model)| {
            model
                .get("id")
                .and_then(Value::as_str)
                .map(|id| (index, id))
        })
        .collect::<Vec<_>>();
    let catalog = indexed
        .iter()
        .map(|(_, id)| CatalogModel { id, aliases: &[] })
        .collect::<Vec<_>>();
    if let Some(index) = resolve_catalog_model_exact(&catalog, model_id)
        .expect("Mojo exact Kiro model lookup failed")
    {
        return (200, model_catalog[indexed[index].0].clone());
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
    let canonical = serde_json::to_string(response).expect("Kiro Anthropic response serializes");
    let body =
        prodex_mojo_core::rich::kiro_rewrite_anthropic_response_json(&canonical, requested_model)
            .unwrap_or_else(|error| {
                panic!("Mojo Kiro Anthropic response rewrite failed: {error:?}")
            });
    serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo Kiro Anthropic response rewrite returned invalid JSON: {error}")
    })
}

pub fn kiro_provider_core_anthropic_sse_body(message: &Value) -> Result<Vec<u8>, MojoError> {
    let canonical = serde_json::to_string(message).map_err(|_| MojoError::InvalidInput)?;
    prodex_mojo_core::rich::kiro_anthropic_sse_body(&canonical)
}
