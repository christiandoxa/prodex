//! Gemini compact text extraction and UTF-8-safe truncation.

pub(super) fn gemini_provider_core_local_compact_text_from_content(
    value: &serde_json::Value,
) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.to_string()),
        serde_json::Value::Array(values) => {
            let text = values
                .iter()
                .filter_map(gemini_provider_core_local_compact_text_from_content)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "output", "input", "query", "command", "commands"] {
                if let Some(text) = object
                    .get(key)
                    .and_then(gemini_provider_core_local_compact_text_from_content)
                    .filter(|text| !text.trim().is_empty())
                {
                    return Some(text);
                }
            }
            let text = object
                .values()
                .filter_map(gemini_provider_core_local_compact_text_from_content)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Null => None,
    }
}

pub(super) fn gemini_provider_core_truncate_utf8(mut text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n[truncated]");
    text
}

pub(super) fn gemini_provider_core_truncate_utf8_edges(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let head_bytes = max_bytes / 3;
    let tail_bytes = max_bytes.saturating_sub(head_bytes);
    let mut head_end = head_bytes.min(text.len());
    while head_end > 0 && !text.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let mut tail_start = text.len().saturating_sub(tail_bytes);
    while tail_start < text.len() && !text.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    format!(
        "{}\n[... middle truncated ...]\n{}",
        &text[..head_end],
        &text[tail_start..]
    )
}
