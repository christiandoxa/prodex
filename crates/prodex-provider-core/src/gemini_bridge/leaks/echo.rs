//! Internal-instruction echo detection.

pub fn gemini_provider_core_internal_instruction_corpus(messages: &[serde_json::Value]) -> String {
    // Serde stays responsible only for borrowed structure acquisition;
    // compiled Mojo selects system rows, field order and nested text.
    let mut document = crate::mojo_json::Document::default();
    document.borrowed_array(messages);
    let json = prodex_mojo_core::json::gemini_value_projection(
        &document.nodes,
        "",
        prodex_mojo_core::json::GeminiValueOperation::SystemCorpus,
    )
    .expect("compiled Mojo Gemini instruction corpus projection returned invalid output");
    let corpus: String = serde_json::from_slice(&json)
        .expect("compiled Mojo Gemini instruction corpus must return a JSON string");
    prodex_mojo_core::gemini_internal_instruction::normalize_corpus(&corpus)
        .expect("Mojo Gemini instruction corpus normalizer returned invalid output")
}

pub fn gemini_provider_core_text_echoes_internal_instruction(
    text: &str,
    internal_instruction_corpus: &str,
) -> bool {
    prodex_mojo_core::gemini_internal_instruction::text_echoes(text, internal_instruction_corpus)
        .expect("Mojo Gemini instruction echo detector returned invalid output")
}
