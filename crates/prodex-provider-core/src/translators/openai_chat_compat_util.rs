//! Shared JSON/text/tool helpers for the OpenAI chat-compatible bridge.

#[cfg(any(not(feature = "mojo"), test))]
use super::Value;

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Array(items) => {
            let parts: Vec<&str> = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("content").and_then(Value::as_str))
                })
                .filter(|text| !text.is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(obj) => obj
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| obj.get("content").and_then(value_to_text))
            .or_else(|| {
                obj.get("output_text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        _ => None,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn copy_if_present(
    source: &serde_json::Map<String, Value>,
    target: &mut serde_json::Map<String, Value>,
    keys: &[&str],
) {
    for key in keys {
        if let Some(value) = source.get(*key) {
            target.insert((*key).to_string(), value.clone());
        }
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn copy_first_if_present(
    source: &serde_json::Map<String, Value>,
    target: &mut serde_json::Map<String, Value>,
    target_key: &str,
    source_keys: &[&str],
) {
    for key in source_keys {
        if let Some(value) = source.get(*key) {
            target.insert(target_key.to_string(), value.clone());
            return;
        }
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn stringify_arguments(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
