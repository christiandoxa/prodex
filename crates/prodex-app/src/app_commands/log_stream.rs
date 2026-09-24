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
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

#[path = "log_event_source.rs"]
mod event_source;
#[path = "log_stream/operational.rs"]
mod operational;
use event_source::operational_event_plan;

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
        LogStreamItem::TokenUsage(event) => serde_json::to_string(&token_usage_json_value(event)),
        LogStreamItem::UpstreamPayload(event) => serde_json::to_string(event),
    }
    .context("failed to serialize JSON log event")
}

fn token_usage_json_value(event: &InfoTokenUsageEvent) -> serde_json::Value {
    let mut value = serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({}));
    value["event"] = serde_json::Value::String("token_usage".to_string());
    let mut fields = serde_json::Map::new();
    if let Some(request) = event.request {
        fields.insert(
            "request".to_string(),
            serde_json::json!(request.to_string()),
        );
    }
    fields.insert("transport".to_string(), serde_json::json!(event.transport));
    fields.insert("profile".to_string(), serde_json::json!(event.profile));
    fields.insert("source".to_string(), serde_json::json!(event.source));
    fields.insert(
        "input_tokens".to_string(),
        serde_json::json!(event.input_tokens),
    );
    fields.insert(
        "cached_input_tokens".to_string(),
        serde_json::json!(event.cached_input_tokens),
    );
    fields.insert(
        "output_tokens".to_string(),
        serde_json::json!(event.output_tokens),
    );
    fields.insert(
        "reasoning_tokens".to_string(),
        serde_json::json!(event.reasoning_tokens),
    );
    if let Some(generation_ms) = event.generation_ms {
        fields.insert(
            "generation_ms".to_string(),
            serde_json::json!(generation_ms),
        );
    }
    if let Some(rate) = event.output_tokens_per_second {
        fields.insert(
            "output_tokens_per_second".to_string(),
            serde_json::json!(rate),
        );
    }
    value["fields"] = serde_json::Value::Object(fields);
    value
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
    if matches!(
        event,
        "stream_payload" | "upstream_payload" | "token_usage" | "token_usage_progress"
    ) {
        return Ok(None);
    }
    let event_plan = operational_event_plan(event, &parsed.fields)?;
    if !event_plan.interesting {
        return Ok(None);
    }
    let Some(source) = event_plan.source else {
        return Ok(None);
    };
    let source = if source == "load" && !is_routine_load_event(event) {
        "error"
    } else {
        source
    };
    if source == "route"
        && event == "profile_commit"
        && parsed.fields.get("switched").map(String::as_str) != Some("true")
    {
        return Ok(None);
    }
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
    let load = (source == "load").then(|| LogLoadObservation {
        event: transcript.clone(),
        event_name: event.to_string(),
        fields: parsed
            .fields
            .iter()
            .filter(|(key, _)| {
                matches!(
                    key.as_str(),
                    "profile"
                        | "route"
                        | "lane"
                        | "transport"
                        | "context"
                        | "provider"
                        | "model"
                        | "path"
                        | "limit"
                        | "hard_limit"
                        | "reason"
                )
            })
            .map(|(key, value)| {
                (
                    key.clone(),
                    if key == "path" {
                        safe_endpoint(value)
                    } else {
                        value.clone()
                    },
                )
            })
            .collect(),
        run_id: request.map(short_request_id),
    });
    Ok(Some(ParsedOperationalEvent { transcript, load }))
}

fn short_request_id(request: u64) -> String {
    format!("r{:04x}", request & 0xffff)
}

#[cfg(any(not(feature = "mojo-core"), test))]
#[path = "log_stream/summary_oracle.rs"]
mod summary_oracle;

#[cfg(feature = "mojo-core")]
#[derive(Clone, Copy)]
enum OperationalDetailFormat {
    Plain,
    Percent,
    Endpoint,
}

#[cfg(feature = "mojo-core")]
const OPERATIONAL_DETAIL_SPECS: [(&str, &str, OperationalDetailFormat); 62] = [
    ("profile", "profile", OperationalDetailFormat::Plain),
    ("route", "route", OperationalDetailFormat::Plain),
    ("provider", "provider", OperationalDetailFormat::Plain),
    ("model", "model", OperationalDetailFormat::Plain),
    ("from_model", "from", OperationalDetailFormat::Plain),
    ("to_model", "to", OperationalDetailFormat::Plain),
    ("effort", "effort", OperationalDetailFormat::Plain),
    ("transport", "transport", OperationalDetailFormat::Plain),
    ("method", "method", OperationalDetailFormat::Plain),
    ("command", "command", OperationalDetailFormat::Plain),
    ("cwd", "cwd", OperationalDetailFormat::Plain),
    ("arg_count", "args", OperationalDetailFormat::Plain),
    ("env_count", "env", OperationalDetailFormat::Plain),
    ("stdin_bytes", "stdin_bytes", OperationalDetailFormat::Plain),
    ("timeout_ms", "timeout_ms", OperationalDetailFormat::Plain),
    ("path", "path", OperationalDetailFormat::Endpoint),
    ("url", "path", OperationalDetailFormat::Endpoint),
    ("tool_surface", "tools", OperationalDetailFormat::Plain),
    (
        "continuation",
        "continuation",
        OperationalDetailFormat::Plain,
    ),
    ("status", "status", OperationalDetailFormat::Plain),
    ("class", "class", OperationalDetailFormat::Plain),
    ("event_type", "event", OperationalDetailFormat::Plain),
    ("state", "state", OperationalDetailFormat::Plain),
    ("code", "code", OperationalDetailFormat::Plain),
    ("reason", "reason", OperationalDetailFormat::Plain),
    ("elapsed_ms", "latency_ms", OperationalDetailFormat::Plain),
    ("duration_ms", "duration_ms", OperationalDetailFormat::Plain),
    ("exit_code", "exit", OperationalDetailFormat::Plain),
    ("exit_status", "exit", OperationalDetailFormat::Plain),
    ("outcome", "outcome", OperationalDetailFormat::Plain),
    ("active", "active", OperationalDetailFormat::Plain),
    ("limit", "limit", OperationalDetailFormat::Plain),
    ("count", "count", OperationalDetailFormat::Plain),
    ("dropped", "dropped", OperationalDetailFormat::Plain),
    ("quota_band", "band", OperationalDetailFormat::Plain),
    (
        "five_hour_remaining",
        "5h",
        OperationalDetailFormat::Percent,
    ),
    ("weekly_remaining", "week", OperationalDetailFormat::Percent),
    ("until", "until", OperationalDetailFormat::Plain),
    ("attempt", "attempt", OperationalDetailFormat::Plain),
    ("retry_index", "retry", OperationalDetailFormat::Plain),
    ("seconds", "backoff_s", OperationalDetailFormat::Plain),
    ("score", "score", OperationalDetailFormat::Plain),
    ("delta", "delta", OperationalDetailFormat::Plain),
    ("chunks", "chunks", OperationalDetailFormat::Plain),
    ("bytes", "bytes", OperationalDetailFormat::Plain),
    ("elapsed_ms", "ttft_ms", OperationalDetailFormat::Plain),
    ("decision", "decision", OperationalDetailFormat::Plain),
    ("tier", "tier", OperationalDetailFormat::Plain),
    ("rewrite_kind", "rewrite", OperationalDetailFormat::Plain),
    (
        "tokens_before",
        "tokens_before",
        OperationalDetailFormat::Plain,
    ),
    (
        "tokens_after",
        "tokens_after",
        OperationalDetailFormat::Plain,
    ),
    (
        "body_bytes_saved",
        "bytes_saved",
        OperationalDetailFormat::Plain,
    ),
    (
        "rewrite_ratio_percent",
        "rewrite",
        OperationalDetailFormat::Percent,
    ),
    (
        "tool_outputs_condensed",
        "tools_condensed",
        OperationalDetailFormat::Plain,
    ),
    (
        "rehydrated_refs",
        "rehydrated",
        OperationalDetailFormat::Plain,
    ),
    ("pressure_band", "pressure", OperationalDetailFormat::Plain),
    ("self_check", "check", OperationalDetailFormat::Plain),
    ("exit", "exit", OperationalDetailFormat::Plain),
    ("attempts", "attempts", OperationalDetailFormat::Plain),
    ("lane", "lane", OperationalDetailFormat::Plain),
    ("hard_limit", "limit", OperationalDetailFormat::Plain),
    ("stage", "stage", OperationalDetailFormat::Plain),
];

fn operational_event_summary(
    event: &str,
    source: &str,
    fields: &BTreeMap<String, String>,
) -> String {
    #[cfg(feature = "mojo-core")]
    {
        let plan = prodex_mojo_core::observability::operational_event_detail_plan(
            source,
            event == "first_local_chunk",
        )
        .unwrap_or_else(|error| panic!("Mojo operational detail plan failed: {error:?}"));
        let mut details = Vec::new();
        for detail in plan {
            let (key, label, format) = OPERATIONAL_DETAIL_SPECS
                .get(usize::try_from(detail).expect("validated Mojo detail index"))
                .copied()
                .expect("validated Mojo operational detail");
            match format {
                OperationalDetailFormat::Plain => add_log_detail(&mut details, fields, key, label),
                OperationalDetailFormat::Percent => {
                    add_log_percent_detail(&mut details, fields, key, label)
                }
                OperationalDetailFormat::Endpoint => {
                    add_log_endpoint_detail(&mut details, fields, key, label)
                }
            }
        }
        join_log_details(&human_event_name(event), details)
    }

    #[cfg(not(feature = "mojo-core"))]
    {
        summary_oracle::operational_event_summary(event, source, fields)
    }
}

fn display_log_field<'a>(fields: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    fields
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty() && value.chars().all(|character| !character.is_control()))
}

fn add_log_detail(
    details: &mut Vec<String>,
    fields: &BTreeMap<String, String>,
    key: &str,
    label: &str,
) {
    if let Some(value) = display_log_field(fields, key) {
        let value = runtime_proxy_crate::runtime_proxy_redact_log_field_value(key, value);
        if !value.is_empty() && value != "-" {
            details.push(format!("{label}={}", bounded_log_value(&value, 192)));
        }
    }
}

fn add_log_percent_detail(
    details: &mut Vec<String>,
    fields: &BTreeMap<String, String>,
    key: &str,
    label: &str,
) {
    if let Some(value) = display_log_field(fields, key) {
        let value = runtime_proxy_crate::runtime_proxy_redact_log_field_value(key, value);
        details.push(format!("{label}={}%", bounded_log_value(&value, 32)));
    }
}

fn add_log_endpoint_detail(
    details: &mut Vec<String>,
    fields: &BTreeMap<String, String>,
    key: &str,
    label: &str,
) {
    if let Some(value) = display_log_field(fields, key) {
        details.push(format!(
            "{label}={}",
            bounded_log_value(&safe_endpoint(value), 192)
        ));
    }
}

fn join_log_details(event: &str, details: Vec<String>) -> String {
    if details.is_empty() {
        event.to_string()
    } else {
        format!("{event}  {}", details.join(" · "))
    }
}

fn bounded_log_value(value: &str, max_chars: usize) -> String {
    let mut bounded = value.chars().take(max_chars).collect::<String>();
    if value.chars().nth(max_chars).is_some() {
        bounded.push('…');
    }
    bounded
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
        let request = event
            .request
            .map(|request| request.to_string())
            .unwrap_or_else(|| "-".to_string());
        let meta = [
            ("profile", event.profile.clone()),
            ("request", request),
            ("transport", event.transport.clone()),
            ("source", event.source.clone()),
            ("input", event.input_tokens.to_string()),
            ("cache", event.cached_input_tokens.to_string()),
            ("output", event.output_tokens.to_string()),
            ("reasoning", event.reasoning_tokens.to_string()),
            (
                "generation",
                event
                    .generation_ms
                    .map(|duration| format!("{duration}ms"))
                    .unwrap_or_else(|| "unavailable".to_string()),
            ),
            (
                "avg_output",
                event
                    .generation_ms
                    .filter(|duration| *duration > 0)
                    .and(event.output_tokens_per_second)
                    .map(|rate| format_output_tokens_per_second(Some(rate)))
                    .unwrap_or_else(|| format_output_tokens_per_second(None)),
            ),
        ];
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
    match source {
        "request" => "REQUEST".to_string(),
        "route" => "ROUTE".to_string(),
        "quota" => "QUOTA".to_string(),
        "retry" => "RETRY".to_string(),
        "backoff" => "BACKOFF".to_string(),
        "health" => "HEALTH".to_string(),
        "upstream" => "UPSTREAM".to_string(),
        "stream" => "STREAM".to_string(),
        "response" => "RESPONSE".to_string(),
        "tokens" => "TOKENS".to_string(),
        "smart" => "SMART".to_string(),
        "compact" => "COMPACT".to_string(),
        "model" => "MODEL".to_string(),
        "tool" => "TOOL".to_string(),
        "agent" => "AGENT".to_string(),
        "mcp" => "MCP".to_string(),
        "hook" => "HOOK".to_string(),
        "load" => "LOAD".to_string(),
        "event" => "EVENT".to_string(),
        "ws" => "WEBSOCKET".to_string(),
        "terminal" => "TERMINAL".to_string(),
        "error" => "ERROR".to_string(),
        "user" => "USER".to_string(),
        "assistant" => "ASSISTANT".to_string(),
        "reasoning" => "REASONING".to_string(),
        "turn-context" => "MODEL".to_string(),
        "session-context" => "SESSION".to_string(),
        "prompt-engineering" => "PROMPT".to_string(),
        "tool-output" => "TOOL RESULT".to_string(),
        source if source.starts_with("tool-call:") => "TOOL CALL".to_string(),
        _ => format!("stream {source}"),
    }
}

pub(crate) fn print_upstream_payload_event(event: &UpstreamPayloadEvent) -> Result<()> {
    log_upstream::print_upstream_payload_event(event, false)
}

#[cfg(test)]
#[path = "log_stream/summary_tests.rs"]
mod summary_tests;
