//! Internal-instruction leak detection for Gemini bridge output.
//!
//! Pure text guards only; transport/runtime policy stays in `prodex-app`.

mod echo;
mod patterns;

pub use self::echo::{
    gemini_provider_core_internal_instruction_corpus,
    gemini_provider_core_text_echoes_internal_instruction,
};
pub use self::patterns::gemini_provider_core_internal_instruction_leak_text;

use self::patterns::gemini_provider_core_safe_tool_status_text;

pub fn gemini_provider_core_sanitize_internal_instruction_leak_text(text: &str) -> Option<String> {
    if !gemini_provider_core_internal_instruction_leak_text(text) {
        return Some(text.to_string());
    }
    let retained = text
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .filter_map(|paragraph| {
            if !gemini_provider_core_internal_instruction_leak_text(paragraph) {
                return Some(paragraph.to_string());
            }
            let retained_lines = paragraph
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .filter(|line| gemini_provider_core_safe_tool_status_text(line))
                .collect::<Vec<_>>();
            (!retained_lines.is_empty()).then(|| retained_lines.join("\n"))
        })
        .collect::<Vec<_>>();
    if retained.is_empty() {
        None
    } else {
        Some(retained.join("\n\n"))
    }
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
