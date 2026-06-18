pub(in super::super) fn runtime_gemini_tool_intent_without_call(
    text: &str,
) -> Option<&'static str> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.len() < 16 {
        return None;
    }
    let future_tool_intent = [
        "i'll use",
        "i will use",
        "i'll call",
        "i will call",
        "i'll run",
        "i will run",
        "i'll search",
        "i will search",
        "i'll inspect",
        "i will inspect",
        "i'll read",
        "i will read",
        "i'm going to use",
        "i am going to use",
        "now, i'll use",
        "next, i'll use",
    ]
    .iter()
    .any(|phrase| lower.contains(phrase));
    if !future_tool_intent {
        return None;
    }
    [
        "exec_command",
        "write_stdin",
        "apply_patch",
        "sqz_grep",
        "sqz_read_file",
        "sqz_list_dir",
        "read_mcp_resource",
        "list_mcp_resources",
        "tool_search",
        "rg",
        "grep",
    ]
    .into_iter()
    .find(|tool| runtime_gemini_contains_tool_token(&lower, tool))
}

pub(in super::super) fn runtime_gemini_non_actionable_wait_or_poll_text(
    text: &str,
) -> Option<&'static str> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.len() < 8 {
        return None;
    }
    [
        ("i will poll", "i will poll"),
        ("i'll poll", "i'll poll"),
        ("i need to wait", "i need to wait"),
        ("let's wait", "let's wait"),
        ("still running", "still running"),
        ("is still running", "is still running"),
        ("i will wait", "i will wait"),
        ("i'll wait", "i'll wait"),
    ]
    .into_iter()
    .find_map(|(needle, reason)| lower.contains(needle).then_some(reason))
}

pub(in super::super) fn runtime_gemini_unverified_success_claim(
    text: &str,
    conversation_messages: &[serde_json::Value],
) -> bool {
    let lower = text.to_ascii_lowercase();
    let claims_success = [
        "blocker/unresolved: none",
        "blockers/unresolved: none",
        "unresolved: none",
        "everything is complete",
        "semua optional tools berhasil",
        "berhasil diupdate",
        "successfully updated all",
        "all optional tools",
        "up-to-date",
        "latest version",
        "latest versions",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !claims_success {
        return false;
    }
    let tool_texts = runtime_gemini_tool_texts_since_latest_user(conversation_messages);
    if tool_texts.is_empty() {
        return true;
    }
    let last_tool = tool_texts.last().map(|text| text.as_str()).unwrap_or("");
    let last_tool_lower = last_tool.to_ascii_lowercase();
    if runtime_gemini_tool_text_has_failure(last_tool) {
        return true;
    }
    let has_verification_marker = [
        "--version",
        "version:",
        "verification:",
        "already up to date",
        "up-to-date",
    ]
    .iter()
    .any(|needle| last_tool_lower.contains(needle))
        || runtime_gemini_tool_text_has_version_lines(last_tool);
    let clean_final_verification = has_verification_marker
        && !tool_texts
            .iter()
            .skip(tool_texts.len().saturating_sub(2))
            .any(|tool| {
                runtime_gemini_tool_text_has_failure(tool)
                    && !tool
                        .to_ascii_lowercase()
                        .contains("process exited with code 0")
            });
    !clean_final_verification
}

fn runtime_gemini_tool_text_has_version_lines(text: &str) -> bool {
    text.lines().any(|line| {
        let lower = line.trim().to_ascii_lowercase();
        let starts_with_tool = [
            "rtk ",
            "sqz ",
            "sqz-mcp ",
            "token-savior ",
            "claw-compactor ",
            "prodex ",
            "codex ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
        starts_with_tool && lower.chars().any(|ch| ch.is_ascii_digit())
    })
}

fn runtime_gemini_tool_texts_since_latest_user(messages: &[serde_json::Value]) -> Vec<String> {
    let latest_user = messages
        .iter()
        .rposition(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("user")
        })
        .unwrap_or(0);
    messages
        .iter()
        .skip(latest_user + 1)
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("tool"))
        .map(|message| {
            let mut text = String::new();
            runtime_gemini_collect_payload_text(
                message.get("output").or_else(|| message.get("content")),
                &mut text,
            );
            text
        })
        .filter(|text| !text.trim().is_empty())
        .collect()
}

fn runtime_gemini_tool_text_has_failure(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "process exited with code 1",
        "process exited with code 2",
        "process exited with code 127",
        "no such file or directory",
        "command not found",
        "error:",
        "failed",
        "not found",
        "virtual manifest",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn runtime_gemini_contains_tool_token(text: &str, token: &str) -> bool {
    let mut offset = 0;
    while let Some(index) = text[offset..].find(token) {
        let start = offset + index;
        let end = start + token.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let before_boundary = before.is_none_or(runtime_gemini_tool_token_boundary);
        let after_boundary = after.is_none_or(runtime_gemini_tool_token_boundary);
        if before_boundary && after_boundary {
            return true;
        }
        offset = end;
    }
    false
}

fn runtime_gemini_tool_token_boundary(ch: char) -> bool {
    !(ch.is_ascii_alphanumeric() || ch == '_')
}

pub(in super::super) fn runtime_gemini_forced_command_output(
    messages: &[serde_json::Value],
) -> Option<String> {
    let user_index = messages.iter().rposition(|message| {
        message.get("role").and_then(serde_json::Value::as_str) == Some("user")
    })?;
    if !runtime_gemini_requests_command_output_only(&messages[user_index]) {
        return None;
    }
    let required_command = runtime_gemini_required_exact_output_command(&messages[user_index]);
    let (tool_index, tool_message) = messages
        .iter()
        .skip(user_index + 1)
        .enumerate()
        .rev()
        .find(|(_, message)| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("tool")
        })?;
    let tool_output = runtime_gemini_command_output_from_tool_message(tool_message)?;
    if let Some(required_command) = required_command
        && !runtime_gemini_tool_command_matches_required(
            &messages[user_index + 1..user_index + 1 + tool_index],
            &required_command,
        )
        && !runtime_gemini_text_contains_required_exact_output_marker(
            &required_command,
            &tool_output,
        )
    {
        return None;
    }
    Some(tool_output)
}

pub(in super::super) fn runtime_gemini_conversation_requests_command_output_only(
    messages: &[serde_json::Value],
) -> bool {
    messages
        .iter()
        .rfind(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("user"))
        .is_some_and(runtime_gemini_requests_command_output_only)
}

fn runtime_gemini_requests_command_output_only(message: &serde_json::Value) -> bool {
    let mut text = String::new();
    runtime_gemini_collect_payload_text(message.get("content"), &mut text);
    let lower = text.to_ascii_lowercase();
    lower.contains("only the command output")
        || lower.contains("command output only")
        || lower.contains("only with the command output")
}

fn runtime_gemini_required_exact_output_command(message: &serde_json::Value) -> Option<String> {
    let mut text = String::new();
    runtime_gemini_collect_payload_text(message.get("content"), &mut text);
    for marker in [
        "run exactly:",
        "verification command from the workspace:",
        "verification command:",
    ] {
        if let Some(command) = runtime_gemini_line_after_marker(&text, marker) {
            return Some(command);
        }
    }
    if let Some(command) = runtime_gemini_inline_command_after_marker(&text, "then run ") {
        return Some(command);
    }
    None
}

fn runtime_gemini_line_after_marker(text: &str, marker: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find(marker)?;
    let start = index + marker.len();
    let command = text[start..]
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    Some(command.to_string())
}

fn runtime_gemini_inline_command_after_marker(text: &str, marker: &str) -> Option<String> {
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

fn runtime_gemini_tool_command_matches_required(
    messages_before_tool: &[serde_json::Value],
    required_command: &str,
) -> bool {
    let required = runtime_gemini_normalize_command_for_match(required_command);
    if required.is_empty() {
        return true;
    }
    messages_before_tool
        .iter()
        .rev()
        .filter(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("assistant")
        })
        .flat_map(runtime_gemini_assistant_tool_commands)
        .any(|command| {
            let command = runtime_gemini_normalize_command_for_match(&command);
            command == required
                || command.contains(&required)
                || required.contains(&command)
                || runtime_gemini_commands_share_exact_output_marker(&required, &command)
        })
}

fn runtime_gemini_commands_share_exact_output_marker(required: &str, command: &str) -> bool {
    runtime_gemini_exact_output_markers(required)
        .into_iter()
        .any(|marker| command.contains(&marker))
}

fn runtime_gemini_text_contains_required_exact_output_marker(required: &str, text: &str) -> bool {
    runtime_gemini_exact_output_markers(required)
        .into_iter()
        .any(|marker| text.contains(&marker))
}

fn runtime_gemini_exact_output_markers(text: &str) -> Vec<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| token.starts_with("PRODEX_") && token.len() >= 16)
        .map(str::to_string)
        .collect()
}

fn runtime_gemini_assistant_tool_commands(message: &serde_json::Value) -> Vec<String> {
    let mut commands = Vec::new();
    if let Some(tool_calls) = message
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
    {
        for tool_call in tool_calls {
            if let Some(function) = tool_call.get("function") {
                runtime_gemini_collect_command_from_function(function, &mut commands);
            }
        }
    }
    runtime_gemini_collect_command_from_function(message, &mut commands);
    commands
}

fn runtime_gemini_collect_command_from_function(
    function: &serde_json::Value,
    commands: &mut Vec<String>,
) {
    let arguments = function
        .get("arguments")
        .or_else(|| function.get("args"))
        .or_else(|| function.get("input"));
    let Some(arguments) = arguments else {
        return;
    };
    let parsed = match arguments {
        serde_json::Value::String(text) => {
            serde_json::from_str::<serde_json::Value>(text).unwrap_or_else(|_| arguments.clone())
        }
        _ => arguments.clone(),
    };
    runtime_gemini_collect_command_strings(&parsed, commands);
}

fn runtime_gemini_collect_command_strings(value: &serde_json::Value, commands: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        return;
    };
    for key in ["cmd", "command", "shell_command"] {
        if let Some(command) = object.get(key).and_then(serde_json::Value::as_str)
            && !command.trim().is_empty()
        {
            commands.push(command.trim().to_string());
        }
    }
}

fn runtime_gemini_normalize_command_for_match(command: &str) -> String {
    command
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['"', '\''])
        .to_string()
}

fn runtime_gemini_command_output_from_tool_message(message: &serde_json::Value) -> Option<String> {
    let mut text = String::new();
    runtime_gemini_collect_payload_text(
        message.get("output").or_else(|| message.get("content")),
        &mut text,
    );
    runtime_gemini_extract_command_output(&text)
}

fn runtime_gemini_extract_command_output(text: &str) -> Option<String> {
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

fn runtime_gemini_collect_payload_text(value: Option<&serde_json::Value>, output: &mut String) {
    match value {
        None => {}
        Some(value) => match value {
            serde_json::Value::String(text) => {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(text);
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    runtime_gemini_collect_payload_text(Some(value), output);
                }
            }
            serde_json::Value::Object(object) => {
                for key in ["output", "content", "text"] {
                    if let Some(value) = object.get(key) {
                        runtime_gemini_collect_payload_text(Some(value), output);
                        break;
                    }
                }
            }
            _ => {}
        },
    }
}

pub(in super::super) fn runtime_gemini_blocked_tool_call_item(message: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": message,
        }],
    })
}

pub(in super::super) fn runtime_gemini_stream_error(
    value: &serde_json::Value,
) -> Option<(String, String)> {
    let error = value.get("error")?;
    let code = error
        .get("status")
        .or_else(|| error.get("code"))
        .and_then(|code| {
            code.as_str()
                .map(str::to_string)
                .or_else(|| code.as_i64().map(|code| code.to_string()))
        })
        .unwrap_or_else(|| "provider_stream_error".to_string());
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Gemini stream returned an embedded provider error")
        .to_string();
    Some((code, message))
}
