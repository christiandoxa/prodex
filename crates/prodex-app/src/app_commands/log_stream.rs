use super::log_follow::{FollowedLog, collect_new_followed_lines};
use super::log_load::{LogLoadAggregate, LogLoadObservation, is_routine_load_event};
use super::log_transcript::TranscriptEvent;
use crate::app_commands::log_format::{
    human_event_name, local_log_timestamp, render_log_block, render_text_body,
};
use crate::app_commands::log_throughput::OutputThroughput;
use crate::app_commands::log_tui::format_output_tokens_per_second;
use crate::app_commands::log_upstream;
use crate::app_commands::log_upstream_payload;
use crate::app_commands::log_upstream_payload::UpstreamPayloadEvent;
use crate::app_commands::log_upstream_payload::parse_runtime_log_line;
use crate::reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use anyhow::{Context, Result};
use prodex_mojo_core::json::LogTokenPresentation;
use prodex_mojo_core::observability::{OperationalDetailFormat, operational_detail_spec};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

#[path = "log_event_source.rs"]
mod event_source;
#[path = "log_stream/operational.rs"]
mod operational;
use event_source::operational_event_plan;

#[path = "log_stream/token_presentation.rs"]
mod token_presentation;
pub(super) use token_presentation::{TokenLogTone, project_token_log};

#[derive(Debug, Clone)]
pub(crate) enum LogStreamItem {
    Transcript(TranscriptEvent),
    LoadObservation(LogLoadObservation),
    LoadAggregate(LogLoadAggregate),
    TokenUsage(InfoTokenUsageEvent),
    UpstreamPayload(UpstreamPayloadEvent),
}

pub(crate) fn print_log_stream_item(event: &LogStreamItem, json: bool) -> Result<()> {
    if !json && matches!(event, LogStreamItem::Transcript(event) if event.source == "load") {
        return Ok(());
    }
    if json {
        println!("{}", log_stream_item_json(event)?);
        return io::stdout()
            .flush()
            .context("failed to flush JSON log output");
    }
    match event {
        LogStreamItem::Transcript(event) => print_transcript_event(event),
        LogStreamItem::LoadObservation(event) if !is_routine_load_event(&event.event_name) => {
            print_transcript_event(&event.event)
        }
        LogStreamItem::LoadAggregate(event)
            if !event
                .key
                .split('\u{1f}')
                .next()
                .is_some_and(is_routine_load_event) =>
        {
            print_transcript_event(&event.as_transcript())
        }
        LogStreamItem::LoadObservation(_) | LogStreamItem::LoadAggregate(_) => Ok(()),
        LogStreamItem::TokenUsage(event) => print_token_usage_event(event, false),
        LogStreamItem::UpstreamPayload(event) => print_upstream_payload_event(event),
    }
}

pub(crate) fn log_stream_item_json(event: &LogStreamItem) -> Result<String> {
    match event {
        LogStreamItem::Transcript(event) => serde_json::to_string(event),
        LogStreamItem::LoadObservation(event) => serde_json::to_string(&event.event),
        LogStreamItem::LoadAggregate(event) => serde_json::to_string(&event.as_transcript()),
        LogStreamItem::TokenUsage(event) => {
            serde_json::to_string(&project_token_log::<serde_json::Value>(
                event,
                LogTokenPresentation::Envelope,
            )?)
        }
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
    for event in collect_new_runtime_log_stream_items(path, state, !json)? {
        if let LogStreamItem::TokenUsage(event) = event {
            print_token_usage_event(&event, json)?;
        }
    }
    Ok(())
}

pub(crate) fn collect_new_runtime_log_stream_items(
    path: &Path,
    state: &mut FollowedLog,
    include_operational_insights: bool,
) -> Result<Vec<LogStreamItem>> {
    collect_new_runtime_log_stream_items_with_throughput(
        path,
        state,
        include_operational_insights,
        None,
    )
}

pub(crate) fn collect_new_runtime_log_stream_items_with_throughput(
    path: &Path,
    state: &mut FollowedLog,
    include_operational_insights: bool,
    throughput: Option<&mut OutputThroughput>,
) -> Result<Vec<LogStreamItem>> {
    collect_new_runtime_log_stream_items_internal(
        path,
        state,
        include_operational_insights,
        throughput,
        false,
    )
}

pub(crate) fn collect_new_runtime_log_stream_items_for_tui_with_throughput(
    path: &Path,
    state: &mut FollowedLog,
    include_operational_insights: bool,
    throughput: Option<&mut OutputThroughput>,
) -> Result<Vec<LogStreamItem>> {
    collect_new_runtime_log_stream_items_internal(
        path,
        state,
        include_operational_insights,
        throughput,
        true,
    )
}

fn collect_new_runtime_log_stream_items_internal(
    path: &Path,
    state: &mut FollowedLog,
    include_operational_insights: bool,
    mut throughput: Option<&mut OutputThroughput>,
    coalesce_load: bool,
) -> Result<Vec<LogStreamItem>> {
    let mut items = Vec::new();
    for line in collect_new_followed_lines(path, state)? {
        items.extend(collect_runtime_log_line(
            path,
            &line,
            include_operational_insights,
            throughput.as_deref_mut(),
            coalesce_load,
        )?);
    }
    Ok(items)
}

pub(crate) fn collect_runtime_log_line(
    path: &Path,
    line: &str,
    include_operational_insights: bool,
    mut throughput: Option<&mut OutputThroughput>,
    coalesce_load: bool,
) -> Result<Vec<LogStreamItem>> {
    let mut items = operational::log_items(line, include_operational_insights, coalesce_load)?;
    if let Some(event) = stream_payload_event_from_runtime_line(line) {
        items.push(LogStreamItem::Transcript(event));
    }
    if let Some(event) = log_upstream_payload::upstream_payload_event_from_runtime_line(line) {
        items.push(LogStreamItem::UpstreamPayload(event));
    }
    operational::observe_token_usage_progress(path, line, throughput.as_deref_mut());
    if let Some(event) = info_token_usage_event_from_line(line) {
        let event = local_token_usage_event(event);
        if let Some(throughput) = throughput {
            throughput.observe_token_usage(path, &event, Instant::now());
            throughput.finish(path, &event);
        }
        items.push(LogStreamItem::TokenUsage(event));
    }
    Ok(items)
}

struct ParsedOperationalEvent {
    transcript: TranscriptEvent,
    load: Option<LogLoadObservation>,
}

fn operational_event_from_runtime_line(line: &str) -> Result<Option<ParsedOperationalEvent>> {
    let Some(parsed) = parse_runtime_log_line(line) else {
        return Ok(None);
    };
    let Some(event) = parsed.event.as_deref() else {
        return Ok(None);
    };
    let Some(event_plan) = operational_event_plan(event, &parsed.fields)? else {
        return Ok(None);
    };
    let source = event_plan.source;
    let request = parsed
        .fields
        .get("request")
        .and_then(|value| value.parse().ok());
    let correlation = request
        .map(short_request_id)
        .unwrap_or_else(|| "-".to_string());
    let summary = operational_event_summary(event, source, &parsed.fields);
    let transcript = TranscriptEvent {
        timestamp: local_log_timestamp(&parsed.timestamp),
        source: source.to_string(),
        text: format!("{correlation}  {summary}"),
    };
    let load = event_plan.load_fields.map(|fields| LogLoadObservation {
        event: transcript.clone(),
        event_name: event.to_string(),
        fields,
        run_id: request.map(short_request_id),
    });
    Ok(Some(ParsedOperationalEvent { transcript, load }))
}

fn short_request_id(request: u64) -> String {
    format!("r{:04x}", request & 0xffff)
}

fn operational_event_summary(
    event: &str,
    source: &str,
    fields: &BTreeMap<String, String>,
) -> String {
    let plan = prodex_mojo_core::observability::operational_event_detail_plan(
        source,
        event == "first_local_chunk",
    )
    .unwrap_or_else(|error| panic!("Mojo operational detail plan failed: {error:?}"));
    let acquired: Vec<_> = plan
        .into_iter()
        .map(|detail| {
            let spec =
                operational_detail_spec(detail).expect("Mojo operational detail metadata failed");
            let original = fields.get(&spec.key).map(String::as_str).unwrap_or("");
            let prepared = match spec.format {
                OperationalDetailFormat::Plain | OperationalDetailFormat::Percent => {
                    runtime_proxy_crate::runtime_proxy_redact_log_field_value(&spec.key, original)
                }
                OperationalDetailFormat::Endpoint => safe_endpoint(original),
            };
            (spec, original, prepared)
        })
        .collect();
    let details: Vec<_> = acquired
        .iter()
        .map(
            |(spec, original, prepared)| prodex_mojo_core::observability::OperationalDetailText {
                label: &spec.label,
                format: spec.format,
                original,
                prepared,
            },
        )
        .collect();
    prodex_mojo_core::observability::render_operational_details(&human_event_name(event), &details)
        .expect("Mojo operational detail rendering failed")
}

fn safe_endpoint(url: &str) -> String {
    if url.starts_with('/') {
        return url.split(['?', '#']).next().unwrap_or(url).to_string();
    }
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| (!url.path().is_empty()).then(|| url.path().to_string()))
        .unwrap_or_else(|| "upstream".to_string())
}

pub(crate) fn stream_payload_event_from_runtime_line(line: &str) -> Option<TranscriptEvent> {
    if !line.contains("stream_payload") {
        return None;
    }
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
        let fields: Vec<(String, String)> =
            project_token_log(event, LogTokenPresentation::Metadata)?;
        let meta = fields
            .iter()
            .map(|(label, value)| (label.as_str(), value.clone()))
            .collect::<Vec<_>>();
        for line in render_log_block(
            &event.timestamp,
            "TOKENS",
            &meta,
            &[],
            terminal_ui::current_cli_width(),
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
    let width = terminal_ui::current_cli_width();
    let body = render_text_body(&event.text, width);
    for line in render_log_block(
        &event.timestamp,
        &log_event_label(&event.source),
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

pub(crate) fn log_event_label(source: &str) -> String {
    prodex_mojo_core::observability::render_log_event_label(source)
        .expect("Mojo log event-label renderer failed")
}

pub(crate) fn print_upstream_payload_event(event: &UpstreamPayloadEvent) -> Result<()> {
    log_upstream::print_upstream_payload_event(event, false)
}

#[cfg(test)]
#[path = "log_stream/summary_tests.rs"]
mod summary_tests;

#[cfg(test)]
#[path = "log_stream/selection_tests.rs"]
mod selection_tests;

#[cfg(test)]
#[path = "log_stream/detail_tests.rs"]
mod detail_tests;

#[cfg(test)]
#[path = "log_stream/label_tests.rs"]
mod label_tests;
