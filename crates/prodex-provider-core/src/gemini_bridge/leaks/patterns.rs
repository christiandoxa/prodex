//! Internal-instruction leak pattern matching.

mod catalog;

use self::catalog::{
    GEMINI_INTERNAL_INSTRUCTION_LEAK_MARKERS, GEMINI_INTERNAL_INSTRUCTION_LEAK_PREFIXES,
};
pub fn gemini_provider_core_internal_instruction_leak_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    GEMINI_INTERNAL_INSTRUCTION_LEAK_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(&prefix.to_ascii_lowercase()))
        || GEMINI_INTERNAL_INSTRUCTION_LEAK_MARKERS
            .iter()
            .any(|marker| lower.contains(&marker.to_ascii_lowercase()))
        || gemini_provider_core_optimizer_instruction_leak_text(&lower)
        || gemini_provider_core_exact_output_instruction_leak_text(&lower)
        || (lower.contains("all commands run with user privileges")
            && lower.contains("execute requested tool tasks"))
}

fn gemini_provider_core_optimizer_instruction_leak_text(lower: &str) -> bool {
    lower.contains("optimizer")
        || lower.contains("token-savior")
        || lower.contains("claw-compactor")
        || lower.contains("sqz-mcp")
        || lower.contains("prodex-sqz")
        || lower.contains("prodex super")
}

fn gemini_provider_core_exact_output_instruction_leak_text(lower: &str) -> bool {
    lower.contains("answer with only the command output")
        || lower.contains("answer with exactly that output")
        || lower.contains("emit only that requested output")
        || lower.contains("command output only")
        || lower.contains("answer-only output")
}

pub(super) fn gemini_provider_core_safe_tool_status_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("versi terbaru") || lower.contains("diperbarui") || lower.contains("sudah versi")
}
