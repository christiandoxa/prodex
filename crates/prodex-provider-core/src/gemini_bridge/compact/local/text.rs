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

pub(super) fn gemini_provider_core_truncate_utf8_edges(text: String, max_bytes: usize) -> String {
    prodex_mojo_core::rich::truncate_gemini_compact_utf8_edges(&text, max_bytes)
        .expect("Mojo Gemini compact edge truncation returned invalid output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_limits_include_markers_and_preserve_utf8() {
        let text = "月".repeat(100);
        for max_bytes in [0, 1, 12, 64] {
            let tail = prodex_mojo_core::rich::truncate_gemini_compact_utf8(&text, max_bytes)
                .expect("Mojo Gemini compact tail truncation returned invalid output");
            let edges = gemini_provider_core_truncate_utf8_edges(text.clone(), max_bytes);
            assert!(tail.len() <= max_bytes);
            assert!(edges.len() <= max_bytes);
            assert!(std::str::from_utf8(tail.as_bytes()).is_ok());
            assert!(std::str::from_utf8(edges.as_bytes()).is_ok());
            if max_bytes == 64 {
                assert!(tail.ends_with("\n[truncated]"));
                assert!(edges.contains("\n[... middle truncated ...]\n"));
            }
        }
    }
}
