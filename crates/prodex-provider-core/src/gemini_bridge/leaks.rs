//! Internal-instruction leak detection for Gemini bridge output.
//!
//! Pure text guards only; transport/runtime policy stays in `prodex-app`.

mod echo;

pub use self::echo::{
    gemini_provider_core_internal_instruction_corpus,
    gemini_provider_core_text_echoes_internal_instruction,
};

pub fn gemini_provider_core_internal_instruction_leak_text(text: &str) -> bool {
    prodex_mojo_core::gemini_internal_instruction::leak_text(text)
        .expect("Mojo Gemini instruction leak classifier returned invalid output")
}

pub fn gemini_provider_core_sanitize_internal_instruction_leak_text(text: &str) -> Option<String> {
    prodex_mojo_core::gemini_internal_instruction::sanitize_text(text)
        .expect("Mojo Gemini instruction leak sanitizer returned invalid output")
}

pub fn gemini_provider_core_visible_text_from_part(part: &serde_json::Value) -> Option<String> {
    if part
        .get("thought")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    part.get("text")
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .and_then(gemini_provider_core_sanitize_internal_instruction_leak_text)
}

#[cfg(test)]
mod tests;
