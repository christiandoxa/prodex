//! JSON/text helpers shared by DeepSeek bridge compatibility shims.

pub fn deepseek_provider_core_responses_content_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(deepseek_provider_core_responses_content_part_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

pub fn deepseek_provider_core_responses_content_text_value(value: &serde_json::Value) -> String {
    deepseek_provider_core_responses_content_text(Some(value))
}

pub fn deepseek_provider_core_json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

pub fn deepseek_provider_core_json_string_at_path(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &[&str],
) -> Option<String> {
    let mut value = object.get(*path.first()?)?;
    for key in path.iter().skip(1) {
        value = value.get(*key)?;
    }
    value.as_str().map(str::to_string)
}

fn deepseek_provider_core_responses_content_part_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Object(object) => object
            .get("text")
            .and_then(serde_json::Value::as_str)
            .or_else(|| object.get("input_text").and_then(serde_json::Value::as_str))
            .or_else(|| {
                object
                    .get("output_text")
                    .and_then(serde_json::Value::as_str)
            })
            .map(str::to_string),
        _ => None,
    }
}
