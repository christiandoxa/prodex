//! Copilot response-shape helpers used by runtime affinity tracking.

use serde_json::Value;

pub fn copilot_provider_core_response_id_from_value(value: &Value) -> Option<String> {
    value
        .get("response")
        .and_then(|response| response.get("id"))
        .and_then(Value::as_str)
        .or_else(|| value.get("id").and_then(Value::as_str))
        .or_else(|| value.get("response_id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|response_id| !response_id.is_empty())
        .map(str::to_string)
}
