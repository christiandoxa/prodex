//! Gemini schema composition sanitization.

use serde_json::Value;

use super::sanitize_function_schema;

pub(super) fn collapse_schema_union(object: &serde_json::Map<String, Value>) -> Option<Value> {
    let alternatives = object
        .get("anyOf")
        .or_else(|| object.get("oneOf"))
        .and_then(Value::as_array)?;
    let non_null = alternatives
        .iter()
        .filter(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .is_none_or(|schema_type| schema_type != "null")
        })
        .collect::<Vec<_>>();
    if non_null.len() != 1 {
        return None;
    }
    let mut sanitized = sanitize_function_schema(non_null[0]);
    if alternatives.len() != non_null.len()
        && let Some(sanitized_object) = sanitized.as_object_mut()
    {
        sanitized_object.insert("nullable".to_string(), Value::Bool(true));
    }
    Some(sanitized)
}

pub(super) fn sanitized_composition(value: Option<&Value>) -> Option<Value> {
    let values = value?.as_array()?;
    let sanitized = values
        .iter()
        .map(sanitize_function_schema)
        .collect::<Vec<_>>();
    (!sanitized.is_empty()).then_some(Value::Array(sanitized))
}

pub(super) fn has_composition(object: &serde_json::Map<String, Value>) -> bool {
    ["anyOf", "oneOf", "allOf"]
        .iter()
        .any(|key| object.contains_key(*key))
}
