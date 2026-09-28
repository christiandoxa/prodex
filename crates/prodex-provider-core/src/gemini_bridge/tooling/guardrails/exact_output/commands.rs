//! Exact-output command marker extraction and matching.

mod matching;

pub(super) use self::matching::gemini_provider_core_tool_command_matches_required;
use super::super::tool_text::gemini_provider_core_collect_payload_text;
use prodex_mojo_core::gemini_guardrails::{
    gemini_exact_output_marker_match, gemini_required_exact_output_command,
};

pub(super) fn gemini_provider_core_required_exact_output_command(
    message: &serde_json::Value,
) -> Option<String> {
    let mut text = String::new();
    gemini_provider_core_collect_payload_text(message.get("content"), &mut text);
    gemini_required_exact_output_command(&text)
        .expect("Mojo Gemini required-command extraction should accept Rust strings")
        .map(str::to_string)
}

pub(super) fn gemini_provider_core_text_contains_required_exact_output_marker(
    required: &str,
    text: &str,
) -> bool {
    gemini_exact_output_marker_match(required, text)
        .expect("Mojo Gemini exact-output marker matching should accept Rust strings")
}
