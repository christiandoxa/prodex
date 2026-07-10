//! Exact-output command marker extraction and matching.

mod matching;

pub(super) use self::matching::gemini_provider_core_tool_command_matches_required;
use super::super::tool_text::gemini_provider_core_collect_payload_text;

pub(super) fn gemini_provider_core_required_exact_output_command(
    message: &serde_json::Value,
) -> Option<String> {
    let mut text = String::new();
    gemini_provider_core_collect_payload_text(message.get("content"), &mut text);
    for marker in [
        "run exactly:",
        "verification command from the workspace:",
        "verification command:",
    ] {
        if let Some(command) = gemini_provider_core_line_after_marker(&text, marker) {
            return Some(command);
        }
    }
    if let Some(command) = gemini_provider_core_inline_command_after_marker(&text, "then run ") {
        return Some(command);
    }
    None
}

pub(super) fn gemini_provider_core_text_contains_required_exact_output_marker(
    required: &str,
    text: &str,
) -> bool {
    gemini_provider_core_exact_output_markers(required)
        .into_iter()
        .any(|marker| text.contains(&marker))
}

fn gemini_provider_core_line_after_marker(text: &str, marker: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find(marker)?;
    let start = index + marker.len();
    let command = text[start..]
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    Some(command.to_string())
}

fn gemini_provider_core_inline_command_after_marker(text: &str, marker: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find(marker)?;
    let start = index + marker.len();
    let rest = text[start..].trim_start();
    let command = rest
        .split(['.', '\n'])
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    Some(command.to_string())
}

pub(super) fn gemini_provider_core_exact_output_markers(text: &str) -> Vec<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| token.starts_with("PRODEX_") && token.len() >= 16)
        .map(str::to_string)
        .collect()
}
