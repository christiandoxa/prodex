//! DeepSeek tool-call thought-signature extraction.

use serde_json::Value;

pub(crate) fn deepseek_tool_call_thought_signature_object(
    object: &serde_json::Map<String, Value>,
) -> Option<String> {
    deepseek_string_field(
        object,
        &[
            "gemini_thought_signature",
            "thought_signature",
            "thoughtSignature",
        ],
    )
    .or_else(|| {
        object
            .get("provider_specific_fields")
            .and_then(Value::as_object)
            .and_then(|nested| nested.get("thought_signature"))
            .and_then(Value::as_str)
            .map(str::to_string)
    })
    .or_else(|| {
        object
            .get("extra_content")
            .and_then(|value| value.get("google"))
            .and_then(|value| value.get("thought_signature"))
            .and_then(Value::as_str)
            .map(str::to_string)
    })
    .filter(|signature| !signature.trim().is_empty())
}

pub(super) fn deepseek_tool_call_thought_signature(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    deepseek_tool_call_thought_signature_object(object)
}

fn deepseek_string_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}
