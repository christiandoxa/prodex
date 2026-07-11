//! Gemini exact command-output forcing helpers.

mod commands;

use self::commands::{
    gemini_provider_core_required_exact_output_command,
    gemini_provider_core_text_contains_required_exact_output_marker,
    gemini_provider_core_tool_command_matches_required,
};
use super::tool_text::gemini_provider_core_collect_payload_text;

pub fn gemini_provider_core_forced_command_output(
    messages: &[serde_json::Value],
) -> Option<String> {
    let user_index = messages.iter().rposition(|message| {
        message.get("role").and_then(serde_json::Value::as_str) == Some("user")
    })?;
    if !gemini_provider_core_requests_command_output_only(&messages[user_index]) {
        return None;
    }
    let required_command =
        gemini_provider_core_required_exact_output_command(&messages[user_index]);
    let (tool_index, tool_message) = messages
        .iter()
        .skip(user_index + 1)
        .enumerate()
        .rev()
        .find(|(_, message)| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("tool")
        })?;
    let tool_output = gemini_provider_core_command_output_from_tool_message(tool_message)?;
    if let Some(required_command) = required_command
        && !gemini_provider_core_tool_command_matches_required(
            &messages[user_index + 1..user_index + 1 + tool_index],
            &required_command,
        )
        && !gemini_provider_core_text_contains_required_exact_output_marker(
            &required_command,
            &tool_output,
        )
    {
        return None;
    }
    Some(tool_output)
}

pub fn gemini_provider_core_conversation_requests_command_output_only(
    messages: &[serde_json::Value],
) -> bool {
    messages
        .iter()
        .rfind(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("user"))
        .is_some_and(gemini_provider_core_requests_command_output_only)
}

fn gemini_provider_core_requests_command_output_only(message: &serde_json::Value) -> bool {
    let mut text = String::new();
    gemini_provider_core_collect_payload_text(message.get("content"), &mut text);
    let lower = text.to_ascii_lowercase();
    lower.contains("only the command output")
        || lower.contains("command output only")
        || lower.contains("only with the command output")
}

fn gemini_provider_core_command_output_from_tool_message(
    message: &serde_json::Value,
) -> Option<String> {
    let mut text = String::new();
    gemini_provider_core_collect_payload_text(
        message.get("output").or_else(|| message.get("content")),
        &mut text,
    );
    gemini_provider_core_extract_command_output(&text)
}

fn gemini_provider_core_extract_command_output(text: &str) -> Option<String> {
    let marker = "Output:\n";
    let mut output = text
        .rfind(marker)
        .map(|index| &text[index + marker.len()..])
        .unwrap_or(text)
        .trim();
    for delimiter in ["\n\ndiff --git ", "\ndiff --git "] {
        if let Some(diff_index) = output.find(delimiter) {
            output = output[..diff_index].trim_end();
            break;
        }
    }
    if output.starts_with("Success. Updated the following files:")
        || output.starts_with("Success. No files changed.")
    {
        return None;
    }
    (!output.is_empty()).then(|| output.to_string())
}
