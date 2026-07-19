//! Gemini compact local fallback and continuation-summary helpers.

mod semantic;
mod snippet;
mod text;

pub use self::semantic::gemini_provider_core_semantic_compact_continuation_summary;
use self::snippet::gemini_provider_core_local_compact_snippet;
use self::text::{
    gemini_provider_core_local_compact_text_from_content, gemini_provider_core_truncate_utf8,
};

pub const GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX: &str = "Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:";
const GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPETS: usize = 24;
const GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES: usize = 768;
const GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES: usize = 24 * 1024;

pub fn gemini_provider_core_local_compact_summary(body: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return "Local Prodex compact fallback could not parse the compact request body."
            .to_string();
    };

    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("unknown");
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut snippets = input
        .iter()
        .filter_map(gemini_provider_core_local_compact_snippet)
        .collect::<Vec<_>>();
    if snippets.len() > GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPETS {
        snippets =
            snippets.split_off(snippets.len() - GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPETS);
    }

    let mut summary = String::new();
    summary.push_str("Local Prodex compact fallback summary.\n\n");
    summary.push_str(&format!("Model: {model}\n"));
    summary.push_str(&format!("Original input items: {}\n", input.len()));
    summary.push_str(&format!("Retained recent items: {}\n\n", snippets.len()));
    summary.push_str("Recent conversation and tool state:\n");

    if snippets.is_empty() {
        summary.push_str("- No parseable recent message or tool content was found.\n");
    } else {
        for snippet in snippets {
            summary.push_str("- ");
            summary.push_str(&snippet.replace('\n', "\n  "));
            summary.push('\n');
        }
    }

    gemini_provider_core_truncate_utf8(
        summary,
        GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES,
    )
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
