//! Gemini session import and checkpoint helpers.

pub fn gemini_provider_core_import_contents_from_value(
    value: &serde_json::Value,
    import_byte_limit: usize,
) -> Vec<serde_json::Value> {
    if let Some(array) = value.get("contents").and_then(serde_json::Value::as_array) {
        return array
            .iter()
            .filter_map(gemini_provider_core_import_content_from_value)
            .collect();
    }
    if let Some(array) = value.get("history").and_then(serde_json::Value::as_array) {
        return array
            .iter()
            .filter_map(gemini_provider_core_import_content_from_value)
            .collect();
    }
    if let Some(array) = value.as_array() {
        return array
            .iter()
            .flat_map(|value| {
                gemini_provider_core_import_contents_from_value(value, import_byte_limit)
            })
            .collect();
    }
    if let Some(content) = gemini_provider_core_import_content_from_value(value) {
        return vec![content];
    }
    if let Some(text) = value.as_str() {
        return gemini_provider_core_import_contents_from_text(text, import_byte_limit);
    }
    Vec::new()
}

pub fn gemini_provider_core_session_checkpoint_value(
    request: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "format": "gemini-generate-content",
        "source": "prodex-gemini-bridge",
        "request": serde_json::Value::Object(request.clone()),
    })
}

fn gemini_provider_core_import_content_from_value(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    if value
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .is_some()
    {
        let role =
            gemini_provider_core_import_role(value.get("role").and_then(serde_json::Value::as_str));
        let parts = value.get("parts")?.clone();
        return Some(serde_json::json!({
            "role": role,
            "parts": parts,
        }));
    }
    let role = gemini_provider_core_import_role(
        value
            .get("role")
            .or_else(|| value.get("author"))
            .or_else(|| value.get("type"))
            .and_then(serde_json::Value::as_str),
    );
    gemini_provider_core_import_text_value(value).map(|text| {
        serde_json::json!({
            "role": role,
            "parts": [{ "text": text }],
        })
    })
}

fn gemini_provider_core_import_contents_from_text(
    text: &str,
    import_byte_limit: usize,
) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(content) = gemini_provider_core_import_content_from_value(&value)
        {
            contents.push(content);
        }
    }
    if !contents.is_empty() {
        return contents;
    }
    let text = gemini_provider_core_truncate_to_bytes(text.trim(), import_byte_limit);
    if text.is_empty() {
        Vec::new()
    } else {
        vec![serde_json::json!({
            "role": "user",
            "parts": [{ "text": format!("Imported Gemini session/checkpoint context:\n{text}") }],
        })]
    }
}

fn gemini_provider_core_import_text_value(value: &serde_json::Value) -> Option<String> {
    for key in ["content", "text", "message", "prompt"] {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str) {
            return Some(text.to_string());
        }
    }
    value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(serde_json::Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
}

fn gemini_provider_core_import_role(role: Option<&str>) -> &'static str {
    match role.unwrap_or_default() {
        "assistant" | "model" => "model",
        _ => "user",
    }
}

pub fn gemini_provider_core_truncate_to_bytes(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}
