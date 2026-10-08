//! Gemini semantic compact request/summary helpers.

use crate::mojo_json::Document;
use prodex_mojo_core::rich::{gemini_compact_request_json, gemini_compact_summary_json};
use std::sync::OnceLock;

static GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS: OnceLock<String> = OnceLock::new();

/// Legacy accessor backed by the Mojo request kernel.
pub fn gemini_provider_core_semantic_compact_instructions() -> &'static str {
    GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS
        .get_or_init(|| {
            let value = serde_json::json!({"input": []});
            let mut document = Document::default();
            document.push(&value, None, "");
            let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
            let body = gemini_compact_request_json(&document.nodes, raw)
                .expect("Mojo Gemini compact request rewrite returned invalid output");
            let value: serde_json::Value = serde_json::from_slice(&body)
                .expect("Mojo Gemini compact request rewrite returned invalid JSON");
            value["instructions"]
                .as_str()
                .expect("Mojo Gemini compact request rewrite omitted instructions")
                .to_owned()
        })
        .as_str()
}

pub fn gemini_provider_core_semantic_compact_request_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let value = serde_json::from_slice::<serde_json::Value>(body)
        .map_err(|err| format!("failed to parse Gemini compact request JSON: {err}"))?;
    let mut document = Document::default();
    document.push(&value, None, "");
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|err| format!("failed to materialize Gemini compact request JSON: {err}"))?;
    gemini_compact_request_json(&document.nodes, raw)
        .map_err(|err| format!("Mojo Gemini compact request rewrite failed: {err:?}"))
}

pub fn gemini_provider_core_semantic_compact_summary(
    value: &serde_json::Value,
    _request_id: u64,
) -> Result<String, String> {
    let mut document = Document::default();
    document.push(value, None, "");
    let Ok(raw) = std::str::from_utf8(&document.raw) else {
        return Err("failed to materialize Gemini compact response JSON".to_string());
    };
    let summary = gemini_compact_summary_json(&document.nodes, raw)
        .map_err(|err| format!("Mojo Gemini compact response summary failed: {err:?}"))?;
    let Some(summary) = summary else {
        return Ok(String::new());
    };
    String::from_utf8(summary)
        .map_err(|err| format!("Mojo Gemini compact response summary was not UTF-8: {err}"))
}
