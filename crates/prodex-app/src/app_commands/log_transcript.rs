use super::log_transcript_text::{
    transcript_text_from_content, transcript_visible_message_text, transcript_visible_tool_output,
};
use super::*;
use crate::app_commands::log_format::local_log_timestamp;
use prodex_runtime_doctor::read_runtime_log_tail;
use std::path::Path;

const MAX_TRANSCRIPT_EVENT_TEXT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct TranscriptEvent {
    pub(crate) timestamp: String,
    pub(crate) source: String,
    pub(crate) text: String,
}

#[cfg(test)]
pub(crate) fn read_new_transcript_events(path: &Path, state: &mut FollowedLog) -> Result<()> {
    for event in collect_new_transcript_events(path, state)? {
        print_transcript_event(&event)?;
    }
    Ok(())
}

pub(crate) fn collect_new_transcript_events(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<TranscriptEvent>> {
    let mut events = Vec::new();
    for line in collect_new_followed_lines(path, state)? {
        for event in transcript_events_from_session_line(&line) {
            if events.last().is_some_and(|last: &TranscriptEvent| {
                last.timestamp == event.timestamp
                    && last.source == event.source
                    && last.text == event.text
            }) {
                continue;
            }
            events.push(event);
        }
    }
    Ok(events)
}

pub(crate) fn latest_transcript_event() -> Result<Option<TranscriptEvent>> {
    for path in recent_session_log_paths()? {
        let tail = match read_runtime_log_tail(&path, SESSION_SNAPSHOT_TAIL_BYTES) {
            Ok(tail) => tail,
            Err(_) => continue,
        };
        let mut latest = None;
        for line in String::from_utf8_lossy(&tail).lines() {
            for event in transcript_events_from_session_line(line) {
                if latest
                    .as_ref()
                    .is_none_or(|current: &TranscriptEvent| event.timestamp >= current.timestamp)
                {
                    latest = Some(event);
                }
            }
        }
        if latest.is_some() {
            return Ok(latest);
        }
    }
    Ok(None)
}

pub(crate) fn transcript_events_from_session_line(line: &str) -> Vec<TranscriptEvent> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };
    let timestamp = value
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .to_string();
    let timestamp = local_log_timestamp(&timestamp);
    let Some(record_type) = value.get("type").and_then(serde_json::Value::as_str) else {
        return Vec::new();
    };
    let Some(payload) = value.get("payload") else {
        return Vec::new();
    };

    let events = match record_type {
        "event_msg" => event_msg_transcript_event(timestamp, payload)
            .into_iter()
            .collect(),
        "session_meta" => session_meta_transcript_events(timestamp, payload),
        "turn_context" => turn_context_transcript_event(timestamp, payload)
            .into_iter()
            .collect(),
        "response_item" => response_item_transcript_event(timestamp, payload)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    };
    events
        .into_iter()
        .map(|mut event| {
            event.text = transcript_safe_text(&event.text);
            event
        })
        .filter(|event| !event.text.trim().is_empty())
        .collect()
}

/// Returns exact visible user-message text from a raw response-item payload.
///
/// Output rendering normalizes whitespace. Prompt delivery verification cannot use that
/// projection because trailing whitespace and newlines are part of the submitted message.
pub(crate) fn transcript_exact_visible_user_message(payload: &serde_json::Value) -> Option<String> {
    if payload.get("type").and_then(serde_json::Value::as_str) != Some("message")
        || payload.get("role").and_then(serde_json::Value::as_str) != Some("user")
        || !transcript_user_message_is_visible(payload)
    {
        return None;
    }
    let content = payload.get("content")?.as_array()?;
    let parts = content
        .iter()
        .map(|item| {
            item.get("text")
                .or_else(|| item.get("content"))
                .and_then(serde_json::Value::as_str)
        })
        .collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn transcript_safe_text(text: &str) -> String {
    let redacted = redaction::redaction_redact_secret_like_text(text);
    let redacted = redacted
        .chars()
        .map(|character| {
            if character.is_control() && !matches!(character, '\n' | '\r' | '\t') {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    if redacted.len() <= MAX_TRANSCRIPT_EVENT_TEXT_BYTES {
        return redacted;
    }
    let end = redacted
        .char_indices()
        .take_while(|(index, _)| *index < MAX_TRANSCRIPT_EVENT_TEXT_BYTES.saturating_sub(16))
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    format!("{} …[truncated]", &redacted[..end])
}

fn session_meta_transcript_events(
    timestamp: String,
    payload: &serde_json::Value,
) -> Vec<TranscriptEvent> {
    let mut events = Vec::new();
    if let Some(text) = payload
        .get("base_instructions")
        .and_then(|base| base.get("text"))
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.trim().is_empty())
    {
        events.push(TranscriptEvent {
            timestamp: timestamp.clone(),
            source: "prompt-engineering".to_string(),
            text: text.to_string(),
        });
    }

    let mut fields = Vec::new();
    if let Some(provider) = payload
        .get("model_provider")
        .or_else(|| payload.get("provider"))
        .and_then(serde_json::Value::as_str)
    {
        fields.push(format!("provider={provider}"));
    }
    if let Some(source) = payload.get("source").and_then(serde_json::Value::as_str) {
        fields.push(format!("source={source}"));
    }
    if let Some(originator) = payload
        .get("originator")
        .and_then(serde_json::Value::as_str)
    {
        fields.push(format!("originator={originator}"));
    }
    if let Some(cwd) = payload.get("cwd").and_then(serde_json::Value::as_str) {
        fields.push(format!("cwd={cwd}"));
    }
    if !fields.is_empty() {
        events.push(TranscriptEvent {
            timestamp,
            source: "session-context".to_string(),
            text: fields.join(" "),
        });
    }
    events
}

fn event_msg_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    let event_type = payload.get("type").and_then(serde_json::Value::as_str)?;
    if event_type.contains("mcp")
        || event_type.contains("subagent")
        || event_type.contains("sub_agent")
        || event_type.contains("tool_call")
    {
        return protocol_operation_transcript_event(timestamp, payload, event_type);
    }
    if event_msg_is_status(event_type) {
        return status_transcript_event(timestamp, payload, event_type);
    }
    let text_field = match event_type {
        "agent_reasoning" => "text",
        _ => "message",
    };
    let text = payload
        .get(text_field)
        .and_then(serde_json::Value::as_str)
        .and_then(transcript_visible_message_text)?;
    let source = match event_type {
        "user_message" => "user",
        "agent_message" => "assistant",
        "agent_reasoning" => "reasoning",
        _ => return None,
    };
    Some(TranscriptEvent {
        timestamp,
        source: source.to_string(),
        text: text.to_string(),
    })
}

fn event_msg_is_status(event_type: &str) -> bool {
    matches!(
        event_type,
        "task_started"
            | "task_complete"
            | "task_completed"
            | "turn_started"
            | "turn_complete"
            | "turn_completed"
            | "turn_aborted"
            | "turn_cancelled"
            | "turn_interrupted"
            | "turn_failed"
            | "command_execution_started"
            | "command_execution_completed"
            | "command_execution_finished"
            | "command_execution_output"
            | "exec_command_begin"
            | "exec_command_end"
            | "error"
    ) || (event_type.contains("command") && event_type.contains("status"))
}

fn status_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
    event_type: &str,
) -> Option<TranscriptEvent> {
    let source = if event_type.contains("fail")
        || event_type.contains("abort")
        || event_type == "error"
        || payload
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| status.contains("fail") || status.contains("error"))
    {
        "error"
    } else {
        "terminal"
    };
    let mut details = Vec::new();
    for key in [
        "status",
        "exit_code",
        "exit_status",
        "reason",
        "duration_ms",
        "message",
    ] {
        if let Some(value) = payload.get(key).and_then(transcript_json_scalar) {
            details.push(format!("{key}={value}"));
        }
    }
    for key in ["stdout", "stderr", "output"] {
        if let Some(value) = payload
            .get(key)
            .and_then(transcript_json_text)
            .filter(|value| !value.trim().is_empty())
        {
            details.push(format!("{key}:\n{}", transcript_safe_text(&value)));
        }
    }
    let text = if details.is_empty() {
        event_type.replace('_', " ")
    } else {
        details.join(" ")
    };
    Some(TranscriptEvent {
        timestamp,
        source: source.to_string(),
        text,
    })
}

fn transcript_json_scalar(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => transcript_safe_operation_value(value),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn transcript_json_text(value: &serde_json::Value) -> Option<String> {
    transcript_json_scalar(value).or_else(|| match value {
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).ok()
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    })
}

fn turn_context_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    let mut fields = Vec::new();
    if let Some(model) = payload.get("model").and_then(serde_json::Value::as_str) {
        fields.push(format!("model={model}"));
    }
    if let Some(effort) = payload.get("effort").and_then(serde_json::Value::as_str) {
        fields.push(format!("effort={effort}"));
    }
    if let Some(summary) = payload.get("summary").and_then(serde_json::Value::as_str) {
        fields.push(format!("summary={summary}"));
    }
    if let Some(approval) = payload
        .get("approval_policy")
        .and_then(serde_json::Value::as_str)
    {
        fields.push(format!("approval={approval}"));
    }
    if let Some(cwd) = payload.get("cwd").and_then(serde_json::Value::as_str) {
        fields.push(format!("cwd={cwd}"));
    }
    (!fields.is_empty()).then(|| TranscriptEvent {
        timestamp,
        source: "turn-context".to_string(),
        text: fields.join(" "),
    })
}

fn response_item_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    match payload.get("type").and_then(serde_json::Value::as_str)? {
        "message" => {
            let source = payload
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("message")
                .to_string();
            if source == "user" && !transcript_user_message_is_visible(payload) {
                return None;
            }
            let text = transcript_text_from_content(payload.get("content")?)?;
            Some(TranscriptEvent {
                timestamp,
                source,
                text,
            })
        }
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(transcript_tool_name)
                .unwrap_or_else(|| "tool".to_string());
            let arguments = payload
                .get("arguments")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            Some(TranscriptEvent {
                timestamp,
                source: format!("tool-call:{name}"),
                text: arguments.to_string(),
            })
        }
        "function_call_output" => {
            let output = transcript_visible_tool_output(
                payload.get("output").and_then(serde_json::Value::as_str)?,
            )?;
            Some(TranscriptEvent {
                timestamp,
                source: "tool-output".to_string(),
                text: output,
            })
        }
        "custom_tool_call" => {
            let name = payload
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(transcript_tool_name)
                .unwrap_or_else(|| "custom-tool".to_string());
            let input = payload
                .get("input")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            Some(TranscriptEvent {
                timestamp,
                source: format!("tool-call:{name}"),
                text: input.to_string(),
            })
        }
        "custom_tool_call_output" => {
            let output = transcript_visible_tool_output(
                payload.get("output").and_then(serde_json::Value::as_str)?,
            )?;
            Some(TranscriptEvent {
                timestamp,
                source: "tool-output".to_string(),
                text: output,
            })
        }
        "local_shell_call" | "shell_call" => shell_call_transcript_event(timestamp, payload),
        "local_shell_call_output" | "shell_call_output" => {
            shell_output_transcript_event(timestamp, payload)
        }
        "reasoning" => transcript_text_from_reasoning(payload).map(|text| TranscriptEvent {
            timestamp,
            source: "reasoning".to_string(),
            text,
        }),
        item_type
            if item_type.contains("mcp")
                || item_type.contains("subagent")
                || item_type.contains("sub_agent")
                || matches!(
                    item_type,
                    "computer_call"
                        | "computer_call_output"
                        | "web_search_call"
                        | "file_search_call"
                        | "code_interpreter_call"
                ) =>
        {
            protocol_operation_transcript_event(timestamp, payload, item_type)
        }
        _ => None,
    }
    .filter(|event| !event.text.trim().is_empty())
}

fn shell_call_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    let name = payload
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(transcript_tool_name)
        .unwrap_or_else(|| "shell".to_string());
    let command = payload
        .get("command")
        .or_else(|| payload.get("arguments"))
        .or_else(|| payload.get("action"))
        .and_then(transcript_json_text)
        .unwrap_or_default();
    Some(TranscriptEvent {
        timestamp,
        source: format!("tool-call:{name}"),
        text: command,
    })
}

fn transcript_tool_name(value: &str) -> String {
    let value = redaction::redaction_redact_secret_like_text(value);
    let mut name = value
        .chars()
        .take(96)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.is_empty() {
        name.push_str("tool");
    }
    name
}

fn shell_output_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    let output = payload
        .get("output")
        .or_else(|| payload.get("aggregated_output"))
        .or_else(|| payload.get("stdout"))
        .and_then(transcript_json_text)
        .and_then(|output| transcript_visible_tool_output(&output))?;
    Some(TranscriptEvent {
        timestamp,
        source: "tool-output".to_string(),
        text: output,
    })
}

fn transcript_user_message_is_visible(payload: &serde_json::Value) -> bool {
    let Some(kinds) = payload
        .get("internal_chat_message_metadata_passthrough")
        .and_then(|metadata| metadata.get("content_item_kinds"))
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    !kinds.is_empty() && kinds.iter().all(|kind| kind.as_str() == Some("user.text"))
}

fn protocol_operation_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
    event_type: &str,
) -> Option<TranscriptEvent> {
    let source = if event_type.contains("mcp") {
        "mcp"
    } else if event_type.contains("subagent") || event_type.contains("sub_agent") {
        "agent"
    } else {
        "tool"
    };
    let mut details = Vec::new();
    for (keys, label) in [
        (&["server", "server_label", "server_name"][..], "server"),
        (&["tool", "tool_name"][..], "tool"),
        (&["name"][..], "name"),
        (&["status"][..], "status"),
        (&["phase"][..], "phase"),
    ] {
        if let Some(value) = keys.iter().find_map(|key| {
            payload
                .get(*key)
                .and_then(serde_json::Value::as_str)
                .and_then(transcript_safe_operation_value)
        }) {
            details.push(format!("{label}={value}"));
        }
    }
    if !details.iter().any(|detail| detail.starts_with("status="))
        && let Some(status) = event_type
            .strip_prefix("subagent_")
            .or_else(|| event_type.strip_prefix("sub_agent_"))
    {
        details.push(format!("status={status}"));
    }
    let text = if details.is_empty() {
        event_type.replace('_', " ")
    } else {
        details.join(" ")
    };
    Some(TranscriptEvent {
        timestamp,
        source: source.to_string(),
        text,
    })
}

fn transcript_safe_operation_value(value: &str) -> Option<String> {
    let value = redaction::redaction_redact_secret_like_text(value);
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    let mut bounded = value.chars().take(192).collect::<String>();
    if value.chars().nth(192).is_some() {
        bounded.push('…');
    }
    Some(bounded)
}

fn transcript_text_from_reasoning(payload: &serde_json::Value) -> Option<String> {
    let summary = payload.get("summary")?;
    let parts = match summary {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(reasoning_summary_text)
            .collect::<Vec<_>>(),
        _ => reasoning_summary_text(summary).into_iter().collect(),
    };
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn reasoning_summary_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
        serde_json::Value::Object(_) => value
            .get("text")
            .or_else(|| value.get("summary_text"))
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(str::to_string),
        _ => None,
    }
}
