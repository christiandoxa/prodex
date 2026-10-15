//! JSON ownership boundary for compiled Gemini conversation semantics.
use prodex_mojo_core::json::GeminiContentOperation;

pub(super) fn apply(contents: &mut Vec<serde_json::Value>, operation: GeminiContentOperation) {
    let output = {
        let mut document = crate::mojo_json::Document::default();
        document.array(contents.iter());
        let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
        prodex_mojo_core::json::gemini_content_hardening(&document.nodes, raw, operation)
            .expect("compiled Mojo Gemini content hardening failed")
    };
    // Keep original contents intact until a complete valid result is decoded.
    *contents = serde_json::from_slice(&output)
        .expect("compiled Mojo Gemini content hardening must emit a JSON array");
}
