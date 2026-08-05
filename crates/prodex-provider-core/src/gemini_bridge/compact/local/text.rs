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
    const SUFFIX: &str = "\n[truncated]";
    if max_bytes <= SUFFIX.len() {
        return SUFFIX[..max_bytes].to_string();
    }
    let mut end = max_bytes - SUFFIX.len();
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(SUFFIX);
    text
}

pub(super) fn gemini_provider_core_truncate_utf8_edges(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    const SEPARATOR: &str = "\n[... middle truncated ...]\n";
    if max_bytes <= SEPARATOR.len() {
        return SEPARATOR[..max_bytes].to_string();
    }
    let retained_bytes = max_bytes - SEPARATOR.len();
    let head_bytes = retained_bytes / 3;
    let tail_bytes = retained_bytes - head_bytes;
    let mut head_end = head_bytes.min(text.len());
    while head_end > 0 && !text.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let mut tail_start = text.len().saturating_sub(tail_bytes);
    while tail_start < text.len() && !text.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    format!("{}{}{}", &text[..head_end], SEPARATOR, &text[tail_start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_limits_include_markers_and_preserve_utf8() {
        let text = "月".repeat(100);
        for max_bytes in [0, 1, 12, 64] {
            let tail = gemini_provider_core_truncate_utf8(text.clone(), max_bytes);
            let edges = gemini_provider_core_truncate_utf8_edges(text.clone(), max_bytes);
            assert!(tail.len() <= max_bytes);
            assert!(edges.len() <= max_bytes);
            assert!(std::str::from_utf8(tail.as_bytes()).is_ok());
            assert!(std::str::from_utf8(edges.as_bytes()).is_ok());
        }
    }
}
