//! Gemini compact local fallback and continuation-summary helpers.

mod semantic;
mod snippet;
mod text;

pub use self::semantic::gemini_provider_core_semantic_compact_continuation_summary;
use self::snippet::gemini_provider_core_local_compact_snippet;
use self::text::gemini_provider_core_local_compact_text_from_content;
use prodex_mojo_core::rich::format_gemini_local_compact_summary;

pub const GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX: &str = "Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:";
const GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES: usize = 768;
const GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES: usize = 24 * 1024;

pub fn gemini_provider_core_local_compact_summary(body: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return "Local Prodex compact fallback could not parse the compact request body."
            .to_string();
    };

    let model = value.get("model").and_then(serde_json::Value::as_str);
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let snippets = input
        .iter()
        .filter_map(gemini_provider_core_local_compact_snippet)
        .collect::<Vec<_>>();

    format_gemini_local_compact_summary(
        model,
        &snippets,
        input.len(),
        GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES,
    )
    .expect("Mojo Gemini local compact summary formatter returned invalid output")
}

pub fn gemini_provider_core_compact_response_body(summary: &str) -> Vec<u8> {
    let text = format!(
        "{}\n\n{}",
        GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX,
        summary.trim()
    );
    serde_json::to_vec(&serde_json::json!({
        "output": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": text,
            }],
        }],
    }))
    .unwrap_or_else(|_| b"{\"output\":[]}".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_compact_summary_formatting_is_mojo_owned_at_provider_boundary() {
        let input = (0..26)
            .map(|index| {
                serde_json::json!({
                    "type": "message",
                    "role": "user",
                    "content": format!("message-{index}\nnext"),
                })
            })
            .collect::<Vec<_>>();
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "  gemini-test  ",
            "input": input,
        }))
        .unwrap();
        let summary = gemini_provider_core_local_compact_summary(&body);
        assert!(summary.starts_with(
            "Local Prodex compact fallback summary.\n\nModel: gemini-test\nOriginal input items: 26\nRetained recent items: 24\n\nRecent conversation and tool state:\n"
        ));
        assert!(!summary.contains("- user message: message-0\n"));
        assert!(!summary.contains("- user message: message-1\n"));
        assert!(summary.contains("- user message: message-2\n  next\n"));
        assert!(summary.contains("- user message: message-25\n  next\n"));

        let empty = gemini_provider_core_local_compact_summary(br#"{"model":"   ","input":[]}"#);
        assert!(empty.contains("Model: unknown"));
        assert!(empty.contains("- No parseable recent message or tool content was found."));
        assert_eq!(
            gemini_provider_core_local_compact_summary(b"not-json"),
            "Local Prodex compact fallback could not parse the compact request body."
        );
    }
}
