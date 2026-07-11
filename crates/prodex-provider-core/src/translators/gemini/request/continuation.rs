//! Gemini request continuation metadata extraction.

use std::collections::BTreeMap;

use serde_json::Value;

pub(crate) fn gemini_continuation_metadata(
    headers: &BTreeMap<String, String>,
    request: &serde_json::Map<String, Value>,
) -> Option<Value> {
    let mut metadata = serde_json::Map::new();
    for header in ["x-codex-turn-state", "session_id"] {
        if let Some(value) = headers.get(header) {
            metadata.insert(header.to_string(), Value::String(value.clone()));
        }
    }
    if let Some(previous) = request.get("previous_response_id").and_then(Value::as_str) {
        metadata.insert(
            "previous_response_id".to_string(),
            Value::String(previous.to_string()),
        );
    }
    (!metadata.is_empty()).then_some(Value::Object(metadata))
}
