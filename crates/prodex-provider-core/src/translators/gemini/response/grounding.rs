//! Gemini grounding, citations, and web-search response items.

use serde_json::Value;

use crate::mojo_json::Document;
use prodex_mojo_core::json::{GeminiGroundingOperation, gemini_grounding};

fn gemini_grounding_value(
    value: &Value,
    operation: GeminiGroundingOperation,
    response_id: Option<&str>,
) -> Option<Value> {
    let mut document = Document::default();
    document.push(value, None, "");
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let bytes = gemini_grounding(&document.nodes, raw, operation, response_id).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn gemini_citation_text(value: &Value) -> Option<String> {
    gemini_grounding_value(value, GeminiGroundingOperation::CitationText, None)?
        .as_str()
        .map(str::to_string)
}

pub(crate) fn gemini_web_search_call_from_grounding(
    value: &Value,
    response_id: &str,
) -> Option<Value> {
    let value = gemini_grounding_value(
        value,
        GeminiGroundingOperation::WebSearchCall,
        Some(response_id),
    )?;
    (!value.is_null()).then_some(value)
}
