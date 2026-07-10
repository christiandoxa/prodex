use super::log_follow::{FollowedLog, collect_new_followed_lines};
use super::log_transcript::TranscriptEvent;
use crate::app_commands::log_format::{
    current_log_width, local_log_timestamp, render_log_block, render_text_body,
};
use crate::app_commands::log_upstream;
use crate::app_commands::log_upstream_payload;
use crate::app_commands::log_upstream_payload::UpstreamPayloadEvent;
use anyhow::{Context, Result};
use prodex_app_reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) enum LogStreamItem {
    Transcript(TranscriptEvent),
    TokenUsage(InfoTokenUsageEvent),
    UpstreamPayload(UpstreamPayloadEvent),
}

pub(crate) fn read_new_runtime_log_events(
    path: &Path,
    state: &mut FollowedLog,
    json: bool,
) -> Result<()> {
    for event in collect_new_runtime_log_stream_items(path, state)? {
        match event {
            LogStreamItem::TokenUsage(event) => print_token_usage_event(&event, json)?,
            LogStreamItem::UpstreamPayload(event) if !json => print_upstream_payload_event(&event)?,
            LogStreamItem::Transcript(_) => {}
            LogStreamItem::UpstreamPayload(_) => {}
        }
    }
    Ok(())
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
        if let Some(event) = log_upstream_payload::upstream_payload_event_from_runtime_line(&line) {
            items.push(LogStreamItem::UpstreamPayload(event));
        }
        if let Some(event) = info_token_usage_event_from_line(&line) {
            items.push(LogStreamItem::TokenUsage(local_token_usage_event(event)));
        }
    }
    Ok(items)
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
