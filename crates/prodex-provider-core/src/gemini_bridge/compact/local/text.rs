//! Gemini compact text extraction and UTF-8-safe truncation.

pub(super) fn gemini_provider_core_local_compact_text_from_content(
    value: &serde_json::Value,
) -> Option<String> {
    let raw = serde_json::to_string(value).expect("Serde JSON value serializes");
    prodex_mojo_core::rich::extract_gemini_compact_text(&raw)
        .expect("Mojo Gemini compact text extraction returned invalid output")
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
