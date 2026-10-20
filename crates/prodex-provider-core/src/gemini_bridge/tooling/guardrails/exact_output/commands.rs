//! Exact-output command marker extraction and matching.

mod matching;

pub(super) use self::matching::gemini_provider_core_tool_command_matches_required;
use prodex_mojo_core::gemini_guardrails::gemini_exact_output_marker_match;

pub(super) fn gemini_provider_core_text_contains_required_exact_output_marker(
    required: &str,
    text: &str,
) -> bool {
    gemini_exact_output_marker_match(required, text)
        .expect("Mojo Gemini exact-output marker matching should accept Rust strings")
}
