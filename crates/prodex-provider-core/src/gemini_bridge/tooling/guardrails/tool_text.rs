//! Serde acquisition for Mojo-owned Gemini conversation guardrails.
use crate::mojo_json::Document;
use prodex_mojo_core::gemini_guardrails::{GeminiGuardrailJsonOperation, gemini_guardrail_json};
use serde::de::DeserializeOwned;

pub(super) fn gemini_provider_core_guardrail_projection<T: DeserializeOwned>(
    context: &serde_json::Value,
    operation: GeminiGuardrailJsonOperation,
) -> T {
    let mut document = Document::default();
    document.root(context);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let bytes = gemini_guardrail_json(&document.nodes, raw, operation)
        .expect("Mojo Gemini conversation guardrail returned invalid output");
    // Decoder failures must not include provider payloads.
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("Mojo Gemini guardrail output violates its typed contract"))
}

pub(super) fn gemini_provider_core_collect_payload_text(
    value: Option<&serde_json::Value>,
    output: &mut String,
) {
    *output = gemini_provider_core_guardrail_projection(
        &serde_json::json!({"value": value, "prefix": output.as_str()}),
        GeminiGuardrailJsonOperation::PayloadText,
    );
}
