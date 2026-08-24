use super::LOG_SNAPSHOT_TAIL_BYTES;
use super::log_follow::{FollowedLog, collect_new_followed_lines};
use super::log_transcript::TranscriptEvent;
use crate::app_commands::collect_recent_runtime_log_paths;
use crate::app_commands::log_format::{
    current_log_width, local_log_timestamp, render_log_block, render_text_body,
};
use crate::app_commands::log_upstream;
use crate::app_commands::log_upstream_payload;
use crate::app_commands::log_upstream_payload::UpstreamPayloadEvent;
use crate::app_commands::log_upstream_payload::parse_runtime_log_line;
use crate::reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use anyhow::{Context, Result};
use prodex_runtime_doctor::read_runtime_log_tail;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) enum LogStreamItem {
    Transcript(TranscriptEvent),
    TokenUsage(InfoTokenUsageEvent),
    UpstreamPayload(UpstreamPayloadEvent),
}

pub(crate) fn print_log_stream_item(event: &LogStreamItem, json: bool) -> Result<()> {
    if json {
        println!("{}", log_stream_item_json(event)?);
        return io::stdout()
            .flush()
            .context("failed to flush JSON log output");
    }
    match event {
        LogStreamItem::Transcript(event) => print_transcript_event(event),
        LogStreamItem::TokenUsage(event) => print_token_usage_event(event, false),
        LogStreamItem::UpstreamPayload(event) => print_upstream_payload_event(event),
    }
}

pub(crate) fn log_stream_item_json(event: &LogStreamItem) -> Result<String> {
    match event {
        LogStreamItem::Transcript(event) => serde_json::to_string(event),
        LogStreamItem::TokenUsage(event) => serde_json::to_string(event),
        LogStreamItem::UpstreamPayload(event) => serde_json::to_string(event),
    }
    .context("failed to serialize JSON log event")
}

#[cfg(test)]
pub(crate) fn read_new_token_usage_events(
    path: &Path,
    state: &mut FollowedLog,
    json: bool,
) -> Result<()> {
    for event in collect_new_runtime_log_stream_items(path, state)? {
        if let LogStreamItem::TokenUsage(event) = event {
            print_token_usage_event(&event, json)?;
        }
    }
    Ok(())
}

pub(crate) fn collect_new_runtime_log_stream_items(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<LogStreamItem>> {
    let mut items = Vec::new();
    for line in collect_new_followed_lines(path, state)? {
        if let Some(event) = stream_payload_event_from_runtime_line(&line) {
            items.push(LogStreamItem::Transcript(event));
        }
        if let Some(event) = log_upstream_payload::upstream_payload_event_from_runtime_line(&line) {
            items.push(LogStreamItem::UpstreamPayload(event));
        }
        if let Some(event) = info_token_usage_event_from_line(&line) {
            items.push(LogStreamItem::TokenUsage(local_token_usage_event(event)));
        }
    }
    Ok(items)
}

pub(crate) fn latest_runtime_stream_payload_event() -> Option<TranscriptEvent> {
    let mut latest = None;
    for path in collect_recent_runtime_log_paths(32) {
        let Ok(tail) = read_runtime_log_tail(&path, LOG_SNAPSHOT_TAIL_BYTES) else {
            continue;
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            let Some(event) = stream_payload_event_from_runtime_line(line) else {
                continue;
            };
            if latest
                .as_ref()
                .is_none_or(|current: &TranscriptEvent| event.timestamp >= current.timestamp)
            {
                latest = Some(event);
            }
        }
    }
    latest
}

pub(crate) fn stream_payload_event_from_runtime_line(line: &str) -> Option<TranscriptEvent> {
    let parsed = parse_runtime_log_line(line)?;
    if parsed.event.as_deref() != Some("stream_payload") {
        return None;
    }
    let source = parsed.fields.get("source")?.clone();
    let text = parsed
        .fields
        .get("stream")
        .or_else(|| parsed.fields.get("message"))
        .cloned()?;
    (!source.trim().is_empty() && !text.trim().is_empty()).then(|| TranscriptEvent {
        timestamp: local_log_timestamp(&parsed.timestamp),
        source,
        text,
    })
}

pub(crate) fn print_token_usage_event(event: &InfoTokenUsageEvent, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
    } else {
        let request = event
            .request
            .map(|request| request.to_string())
            .unwrap_or_else(|| "-".to_string());
        let meta = [
            ("profile", event.profile.clone()),
            ("request", request),
            ("transport", event.transport.clone()),
            ("source", event.source.clone()),
            ("sent", event.input_tokens.to_string()),
            ("cached", event.cached_input_tokens.to_string()),
            ("received", event.output_tokens.to_string()),
            ("reasoning", event.reasoning_tokens.to_string()),
        ];
        for line in render_log_block(
            &event.timestamp,
            "stream usage",
            &meta,
            &[],
            current_log_width(),
        ) {
            println!("{line}");
        }
    }
    io::stdout()
        .flush()
        .context("failed to flush token log output")
}

pub(crate) fn local_token_usage_event(mut event: InfoTokenUsageEvent) -> InfoTokenUsageEvent {
    event.timestamp = local_log_timestamp(&event.timestamp);
    event
}

pub(crate) fn print_transcript_event(event: &TranscriptEvent) -> Result<()> {
    let width = current_log_width();
    let body = render_text_body(&event.text, width);
    for line in render_log_block(
        &event.timestamp,
        &format!("stream {}", event.source),
        &[],
        &body,
        width,
    ) {
        println!("{line}");
    }
    io::stdout()
        .flush()
        .context("failed to flush transcript log output")
}

pub(crate) fn print_upstream_payload_event(event: &UpstreamPayloadEvent) -> Result<()> {
    log_upstream::print_upstream_payload_event(event, false)
}
