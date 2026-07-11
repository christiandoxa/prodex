//! Provider usage token estimation helpers.

pub fn estimate_request_input_tokens(body: &[u8]) -> u64 {
    if body.is_empty() {
        return 0;
    }
    let parsed = serde_json::from_slice::<serde_json::Value>(body).ok();
    let estimated = parsed
        .as_ref()
        .and_then(estimate_request_input_tokens_value)
        .unwrap_or_else(|| estimate_text_tokens(&String::from_utf8_lossy(body)));
    estimated.max(1)
}

pub fn estimate_request_input_tokens_value(value: &serde_json::Value) -> Option<u64> {
    let mut total = 0_u64;
    if let Some(object) = value.as_object() {
        for key in [
            "input",
            "messages",
            "contents",
            "content",
            "parts",
            "prompt",
            "system",
            "instructions",
            "tools",
        ] {
            if let Some(value) = object.get(key) {
                total = total.saturating_add(estimate_value_tokens(value));
            }
        }
    }
    (total > 0).then_some(total)
}

fn estimate_value_tokens(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::String(text) => estimate_text_tokens(text),
        serde_json::Value::Array(values) => values.iter().fold(0_u64, |sum, value| {
            sum.saturating_add(estimate_value_tokens(value))
        }),
        serde_json::Value::Object(object) => object.iter().fold(0_u64, |sum, (key, value)| {
            if matches!(
                key.as_str(),
                "model"
                    | "role"
                    | "type"
                    | "id"
                    | "name"
                    | "metadata"
                    | "temperature"
                    | "top_p"
                    | "stream"
            ) {
                sum
            } else {
                sum.saturating_add(estimate_value_tokens(value))
            }
        }),
        _ => 0,
    }
}

pub fn estimate_text_tokens(text: &str) -> u64 {
    let significant = text.chars().filter(|ch| !ch.is_control()).count() as u64;
    significant.saturating_add(3) / 4
}
