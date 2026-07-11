use super::log_transcript_text::{
    transcript_text_from_content, transcript_visible_message_text, transcript_visible_tool_output,
};
use super::*;
use crate::app_commands::log_format::local_log_timestamp;
use prodex_runtime_doctor::read_runtime_log_tail;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptEvent {
    pub(crate) timestamp: String,
    pub(crate) source: String,
    pub(crate) text: String,
}

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
            let event = local_transcript_event(event);
            if events.last().is_some_and(|last: &TranscriptEvent| {
                last.source == event.source && last.text == event.text
            }) {
                continue;
            }
            events.push(event);
        }
    }
    Ok(events)
}

pub(crate) fn local_transcript_event(mut event: TranscriptEvent) -> TranscriptEvent {
    event.timestamp = local_log_timestamp(&event.timestamp);
    event
}

pub(crate) fn latest_transcript_event() -> Result<Option<TranscriptEvent>> {
    let mut latest = None;
    for path in recent_session_log_paths()? {
        let tail = match read_runtime_log_tail(&path, SESSION_SNAPSHOT_TAIL_BYTES) {
            Ok(tail) => tail,
            Err(_) => continue,
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            for event in transcript_events_from_session_line(line) {
                let event = local_transcript_event(event);
                if latest
                    .as_ref()
                    .is_none_or(|current: &TranscriptEvent| event.timestamp >= current.timestamp)
                {
                    latest = Some(event);
                }
            }
        }
    }
    Ok(latest)
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

    match record_type {
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
    }
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
                .unwrap_or("tool");
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
                .unwrap_or("custom-tool");
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
        "reasoning" => transcript_text_from_reasoning(payload).map(|text| TranscriptEvent {
            timestamp,
            source: "reasoning".to_string(),
            text,
        }),
        _ => None,
    }
    .filter(|event| !event.text.trim().is_empty())
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
