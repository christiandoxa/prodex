//! Supported-parameter checks for Kiro chat request compatibility.

use super::KiroProviderCoreRequestError;
use serde_json::Value;

pub(super) fn kiro_provider_core_supported_chat_response_format(value: &Value) -> bool {
    value.is_null()
        || value
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| matches!(kind, "text"))
}

pub(super) fn kiro_provider_core_has_requested_stop_sequences(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => items.iter().any(|item| match item {
            Value::String(text) => !text.is_empty(),
            _ => true,
        }),
        _ => true,
    }
}

pub(super) fn kiro_provider_core_has_requested_sampling_value(value: &Value) -> bool {
    !matches!(value, Value::Null)
}

pub(super) fn kiro_provider_core_has_requested_nondefault_number(
    value: &Value,
    default: f64,
) -> bool {
    match value {
        Value::Null => false,
        Value::Number(number) => number
            .as_f64()
            .is_none_or(|value| (value - default).abs() > f64::EPSILON),
        _ => true,
    }
}

pub(super) fn kiro_provider_core_has_requested_parallel_tool_calls_control(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(true) => false,
        Value::Bool(false) => true,
        _ => true,
    }
}

pub(super) fn kiro_provider_core_strip_accepted_token_limit_controls(
    object: &mut serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        let Some(value) = object.get(field) else {
            continue;
        };
        if value.as_u64().is_none_or(|count| count == 0) {
            return Err(KiroProviderCoreRequestError::new(
                format!("Kiro {field} must be a positive integer"),
                "unsupported_token_limit",
            ));
        }
    }
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        object.remove(field);
    }
    Ok(())
}
