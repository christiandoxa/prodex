use super::log_transcript_text::{
    transcript_text_from_content, transcript_visible_message_text, transcript_visible_tool_output,
};
use super::*;
use crate::app_commands::log_format::local_log_timestamp;
use prodex_mojo_core::json::{
    TranscriptContextKind, TranscriptOperationKind, TranscriptOperationValue,
};
use prodex_runtime_doctor::read_runtime_log_tail;
use std::path::Path;

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
                if crate::app_commands::log_format::log_latest_timestamp_wins(
                    latest
                        .as_ref()
                        .map(|current: &TranscriptEvent| current.timestamp.as_str()),
                    &event.timestamp,
                ) {
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
        "session_meta" => {
            context_transcript_events(timestamp, payload, TranscriptContextKind::SessionMetadata)
        }
        "turn_context" => {
            context_transcript_events(timestamp, payload, TranscriptContextKind::TurnContext)
        }
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

fn transcript_safe_text(text: &str) -> String {
    let redacted = redaction::redaction_redact_secret_like_text(text);
    prodex_mojo_core::log::normalize_transcript_safe_text(&redacted)
        .expect("compiled-in Mojo transcript safe-text normalizer failed")
}

fn context_transcript_events(
    timestamp: String,
    payload: &serde_json::Value,
    kind: TranscriptContextKind,
) -> Vec<TranscriptEvent> {
    let tree = crate::json_boundary::app_json_tree_without_wire(payload)
        .expect("transcript context exceeded the JSON ABI boundary");
    let projected = prodex_mojo_core::json::log_transcript_context(&tree.nodes, kind)
        .expect("Mojo transcript context projection returned invalid output");
    projected_transcript_events(timestamp, &projected)
}

fn operation_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
    kind: TranscriptOperationKind,
) -> Option<TranscriptEvent> {
    use prodex_mojo_core::json::JsonKind;
    let tree = crate::json_boundary::app_json_tree(payload)
        .expect("transcript operation exceeded the JSON ABI boundary");
    let raw = std::str::from_utf8(&tree.raw).expect("Serde emits UTF-8 JSON");
    let values = tree
        .nodes
        .iter()
        .map(|node| {
            // The ABI consumes root members; avoid copying overlapping subtree presentations.
            if node.parent != Some(0) {
                return TranscriptOperationValue::default();
            }
            // Serde owns number/container spelling; decoded strings use the existing sanitizer.
            let scalar = match node.kind {
                JsonKind::String => transcript_safe_operation_value(node.text).unwrap_or_default(),
                JsonKind::Null
                | JsonKind::False
                | JsonKind::True
                | JsonKind::Number
                | JsonKind::Array
                | JsonKind::Object => {
                    raw[node.raw_start..node.raw_start + node.raw_length].to_string()
                }
            };
            let output = match kind {
                TranscriptOperationKind::ShellCall => transcript_tool_name(node.text),
                TranscriptOperationKind::ShellOutput => {
                    transcript_visible_tool_output(&scalar).unwrap_or_default()
                }
                TranscriptOperationKind::ToolProtocol
                | TranscriptOperationKind::McpProtocol
                | TranscriptOperationKind::AgentProtocol
                | TranscriptOperationKind::TerminalStatus
                | TranscriptOperationKind::ErrorStatus => transcript_safe_text(&scalar),
            };
            TranscriptOperationValue { scalar, output }
        })
        .collect::<Vec<_>>();
    let projected =
        prodex_mojo_core::json::log_transcript_operation(&tree.nodes, raw, &values, kind)
            .expect("Mojo transcript operation projection returned invalid output");
    projected_transcript_events(timestamp, &projected).pop()
}

fn projected_transcript_events(timestamp: String, projected: &[u8]) -> Vec<TranscriptEvent> {
    let records: Vec<(String, String)> = serde_json::from_slice(projected)
        .expect("Mojo transcript projection returned invalid records");
    records
        .into_iter()
        .map(|(source, text)| TranscriptEvent {
            timestamp: timestamp.clone(),
            source,
            text,
        })
        .collect()
}

fn event_msg_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    use prodex_mojo_core::log::TranscriptEventKind;

    let event_type = payload.get("type").and_then(serde_json::Value::as_str)?;
    let status = payload.get("status").and_then(serde_json::Value::as_str);
    let plan = prodex_mojo_core::log::classify_transcript_event(event_type, status)
        .expect("Mojo transcript event classifier returned invalid output");
    match plan.kind {
        TranscriptEventKind::Protocol => {
            operation_transcript_event(timestamp, payload, plan.protocol_source.into())
        }
        TranscriptEventKind::StatusTerminal => {
            operation_transcript_event(timestamp, payload, TranscriptOperationKind::TerminalStatus)
        }
        TranscriptEventKind::StatusError => {
            operation_transcript_event(timestamp, payload, TranscriptOperationKind::ErrorStatus)
        }
        TranscriptEventKind::User
        | TranscriptEventKind::Assistant
        | TranscriptEventKind::Reasoning => {
            let (text_field, source) = match plan.kind {
                TranscriptEventKind::User => ("message", "user"),
                TranscriptEventKind::Assistant => ("message", "assistant"),
                TranscriptEventKind::Reasoning => ("text", "reasoning"),
                _ => unreachable!("matched transcript message event"),
            };
            let text = payload
                .get(text_field)
                .and_then(serde_json::Value::as_str)
                .and_then(transcript_visible_message_text)?;
            Some(TranscriptEvent {
                timestamp,
                source: source.to_string(),
                text: text.to_string(),
            })
        }
        TranscriptEventKind::Unknown => None,
    }
}

fn response_item_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    use prodex_mojo_core::log::TranscriptItemKind;

    let item_type = payload.get("type").and_then(serde_json::Value::as_str)?;
    let plan = prodex_mojo_core::log::classify_transcript_item(item_type)
        .expect("Mojo transcript response-item classifier returned invalid output");
    match plan.kind {
        TranscriptItemKind::Message => {
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
        TranscriptItemKind::FunctionCall => {
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
        TranscriptItemKind::FunctionOutput | TranscriptItemKind::CustomOutput => {
            let output = transcript_visible_tool_output(
                payload.get("output").and_then(serde_json::Value::as_str)?,
            )?;
            Some(TranscriptEvent {
                timestamp,
                source: "tool-output".to_string(),
                text: output,
            })
        }
        TranscriptItemKind::CustomCall => {
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
        TranscriptItemKind::ShellCall => {
            operation_transcript_event(timestamp, payload, TranscriptOperationKind::ShellCall)
        }
        TranscriptItemKind::ShellOutput => {
            operation_transcript_event(timestamp, payload, TranscriptOperationKind::ShellOutput)
        }
        TranscriptItemKind::Reasoning => {
            context_transcript_events(timestamp, payload, TranscriptContextKind::Reasoning).pop()
        }
        TranscriptItemKind::Protocol => {
            operation_transcript_event(timestamp, payload, plan.protocol_source.into())
        }
        TranscriptItemKind::Unknown => None,
    }
    .filter(|event| !event.text.trim().is_empty())
}

fn transcript_tool_name(value: &str) -> String {
    let value = redaction::redaction_redact_secret_like_text(value);
    prodex_mojo_core::log::sanitize_transcript_tool_name(&value)
        .expect("Mojo transcript tool-name sanitizer returned invalid output")
}

fn transcript_user_message_is_visible(payload: &serde_json::Value) -> bool {
    let tree = crate::json_boundary::app_json_tree_without_wire(payload)
        .expect("transcript visibility exceeded the JSON ABI boundary");
    prodex_mojo_core::json::session_prompt_write_json_check(
        &tree.nodes,
        prodex_mojo_core::json::SessionPromptWriteJsonCheck::TranscriptPayloadVisible,
    )
    .expect("compiled-in Mojo transcript visibility policy failed")
}

fn transcript_safe_operation_value(value: &str) -> Option<String> {
    let value = redaction::redaction_redact_secret_like_text(value);
    let (bounded, truncated) = prodex_mojo_core::log::transcript_operation_span(&value)
        .expect("Mojo transcript operation-value policy returned invalid output")?;
    let mut result = bounded.to_string();
    if truncated {
        result.push('…');
    }
    Some(result)
}

#[cfg(test)]
#[path = "log_transcript_visibility_tests.rs"]
mod visibility_tests;
