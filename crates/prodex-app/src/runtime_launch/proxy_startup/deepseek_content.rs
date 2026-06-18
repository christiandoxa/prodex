pub(super) fn runtime_deepseek_responses_content_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(runtime_deepseek_responses_content_part_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

pub(super) fn runtime_deepseek_responses_content_text_value(value: &serde_json::Value) -> String {
    runtime_deepseek_responses_content_text(Some(value))
}

fn runtime_deepseek_responses_content_part_text(value: &serde_json::Value) -> Option<String> {
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
