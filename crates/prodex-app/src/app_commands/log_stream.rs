use super::log_follow::{FollowedLog, collect_new_followed_lines};
use super::log_transcript::TranscriptEvent;
use crate::app_commands::log_format::{
    current_log_width, human_event_name, local_log_timestamp, render_log_block, render_text_body,
};
use crate::app_commands::log_throughput::OutputThroughput;
use crate::app_commands::log_tui::format_output_tokens_per_second;
use crate::app_commands::log_upstream;
use crate::app_commands::log_upstream_payload;
use crate::app_commands::log_upstream_payload::UpstreamPayloadEvent;
use crate::app_commands::log_upstream_payload::parse_runtime_log_line;
use crate::reports::{
    InfoTokenUsageEvent, info_token_usage_event_from_line,
    info_token_usage_progress_event_from_line,
};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

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
    mut throughput: Option<&mut OutputThroughput>,
) -> Result<Vec<LogStreamItem>> {
    let mut items = Vec::new();
    for line in collect_new_followed_lines(path, state)? {
        items.extend(collect_runtime_log_line(
            path,
            &line,
            include_operational_insights,
            throughput.as_deref_mut(),
        )?);
    }
    Ok(items)
}

fn collect_runtime_log_line(
    path: &Path,
    line: &str,
    include_operational_insights: bool,
    mut throughput: Option<&mut OutputThroughput>,
) -> Result<Vec<LogStreamItem>> {
    let mut items = Vec::new();
    if include_operational_insights && let Some(event) = operational_event_from_runtime_line(line)?
    {
        items.push(LogStreamItem::Transcript(event));
    }
    if let Some(event) = stream_payload_event_from_runtime_line(line) {
        items.push(LogStreamItem::Transcript(event));
    }
    if let Some(event) = log_upstream_payload::upstream_payload_event_from_runtime_line(line) {
        items.push(LogStreamItem::UpstreamPayload(event));
    }
    if let Some(event) = info_token_usage_progress_event_from_line(line)
        && let Some(throughput) = throughput.as_deref_mut()
    {
        throughput.observe_token_usage(path, &event, Instant::now());
    }
    if let Some(event) = info_token_usage_event_from_line(line) {
        let event = local_token_usage_event(event);
        if let Some(throughput) = throughput {
            throughput.observe_token_usage(path, &event, Instant::now());
            if event.generation_ms.is_some() || event.output_tokens_per_second.is_some() {
                throughput.finish(path, &event);
            }
        }
        items.push(LogStreamItem::TokenUsage(event));
    }
    Ok(items)
}

fn operational_event_from_runtime_line(line: &str) -> Result<Option<TranscriptEvent>> {
    let Some(parsed) = parse_runtime_log_line(line) else {
        return Ok(None);
    };
    let Some(event) = parsed.event.as_deref() else {
        return Ok(None);
    };
    if !operational_event_is_interesting(event, &parsed.fields) {
        return Ok(None);
    }
    let Some(source) = operational_event_source(event, &parsed.fields)? else {
        return Ok(None);
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
    Ok(Some(TranscriptEvent {
        timestamp: local_log_timestamp(&parsed.timestamp),
        source: source.to_string(),
        text: format!("{correlation}  {summary}"),
    }))
}

fn operational_event_is_interesting(event: &str, fields: &BTreeMap<String, String>) -> bool {
    if event == "compat_request_surface" {
        let tool_surface = fields.get("tool_surface").map(String::as_str);
        return tool_surface.is_some_and(|value| value != "none")
            || fields
                .get("continuation")
                .is_some_and(|value| value != "none")
            || fields.get("family").is_some_and(|value| value != "codex");
    }
    if event == "smart_context_prepare_fallback" {
        return fields
            .get("decision")
            .is_none_or(|decision| decision != "pass_through");
    }
    true
}

#[cfg(feature = "mojo-core")]
fn operational_event_source(
    event: &str,
    _fields: &BTreeMap<String, String>,
) -> Result<Option<&'static str>> {
    let (category, _) = prodex_mojo_core::log::classify_log_event(event)
        .map_err(|error| anyhow::anyhow!("Mojo log event classifier failed: {error:?}"))?;
    Ok(category.source())
}

#[cfg(not(feature = "mojo-core"))]
fn operational_event_source(
    event: &str,
    fields: &BTreeMap<String, String>,
) -> Result<Option<&'static str>> {
    Ok(operational_event_source_rust(event, fields))
}

#[cfg(not(feature = "mojo-core"))]
fn operational_event_source_rust(
    event: &str,
    fields: &BTreeMap<String, String>,
) -> Option<&'static str> {
    match event {
        "request_captured" => Some("request"),
        "compat_request_surface" => {
            let tool_surface = fields.get("tool_surface").map(String::as_str);
            if tool_surface.is_some_and(|value| value.contains("mcp")) {
                Some("mcp")
            } else if tool_surface
                .is_some_and(|value| value.contains("sub_agent") || value.contains("subagent"))
            {
                Some("agent")
            } else if fields
                .get("continuation")
                .is_some_and(|value| value != "none")
                || tool_surface.is_some_and(|value| value != "none")
                || fields.get("family").is_some_and(|value| value != "codex")
            {
                Some("request")
            } else {
                None
            }
        }
        "route_decision"
        | "selection_plan"
        | "selection_pick"
        | "selection_keep_affinity"
        | "selection_keep_current"
        | "selection_skip_current"
        | "selection_skip_affinity"
        | "selection_skip_sync_probe"
        | "local_selection_blocked"
        | "route_affinity_recompute"
        | "route_affinity_recompute_result"
        | "profile_commit"
        | "previous_response_owner"
        | "previous_response_not_found"
        | "previous_response_negative_cache"
        | "previous_response_fresh_fallback"
        | "previous_response_fresh_fallback_blocked"
        | "previous_response_turn_state_rehydrated"
        | "session_rotation_release_affinity"
        | "binding_prompt_cache"
        | "upgrade"
        | "upgraded" => Some("route"),
        "profile_quota_exhausted"
        | "quota_exhausted"
        | "quota_blocked"
        | "quota_critical_floor_before_send"
        | "profile_quota_quarantine"
        | "profile_probe_refresh_start"
        | "profile_probe_refresh_ok"
        | "compact_pre_send_allow_quota_exhausted"
        | "upstream_usage_limit_passthrough"
        | "upstream_overload_passthrough" => Some("quota"),
        "profile_retry_backoff"
        | "compact_retryable_failure"
        | "compact_overload_conservative_retry"
        | "local_rewrite_gemini_quota_rotate"
        | "local_rewrite_gemini_rate_limit_retry"
        | "local_rewrite_gemini_invalid_stream_retry"
        | "websocket_reuse_owner_fresh_retry"
        | "websocket_reuse_nonreplayable_fresh_retry"
        | "websocket_reuse_locked_affinity_owner_fresh_retry" => Some("retry"),
        "profile_transport_backoff"
        | "rotation_waiting_for_recovery"
        | "profile_circuit_open"
        | "profile_circuit_half_open_probe"
        | "websocket_reuse_watchdog_timeout" => Some("backoff"),
        "profile_transport_failure" | "profile_health" | "profile_bad_pairing" => Some("health"),
        "profile_auth_recovery_failed" | "profile_auth_background_refresh_failed" => Some("error"),
        "profile_auth_recovered" => Some("model"),
        "profile_auth_backoff" => Some("backoff"),
        "upstream_start"
        | "upstream_response"
        | "upstream_async_start"
        | "upstream_async_response"
        | "upstream_connect_start"
        | "upstream_connect_ok"
        | "upstream_connect_error" => Some("upstream"),
        "first_upstream_chunk" | "first_local_chunk" | "stream_complete" | "committed" => {
            Some("stream")
        }
        "buffered_response_complete" => Some("response"),
        "terminal_event" => Some("terminal"),
        "local_rewrite_gemini_builtin_tool_fallback" => Some("tool"),
        "runtime_proxy_queue_overloaded"
        | "runtime_proxy_active_limit_reached"
        | "runtime_proxy_lane_limit_reached"
        | "runtime_proxy_overload_backoff"
        | "runtime_proxy_admission_wait_exhausted"
        | "runtime_proxy_queue_wait_exhausted"
        | "profile_inflight_saturated"
        | "websocket_dns_overflow_reject"
        | "websocket_connect_overflow_reject"
        | "websocket_connect_overflow_rejected" => Some("load"),
        "smart_context_autopilot" | "smart_context_prepare_error" | "smart_context_disabled" => {
            Some("smart")
        }
        "smart_context_prepare_fallback"
            if fields
                .get("decision")
                .is_some_and(|decision| decision != "pass_through") =>
        {
            Some("smart")
        }
        "local_rewrite_request_detail"
        | "local_rewrite_provider_model_fallback"
        | "local_rewrite_provider_auth_failure" => Some("model"),
        "websocket_precommit_frame_timeout"
        | "websocket_precommit_hold_timeout"
        | "websocket_dns_resolve_timeout"
        | "websocket_proxy_tunnel_failure"
        | "upstream_connect_timeout"
        | "upstream_connect_dns_error"
        | "upstream_tls_handshake_error" => Some("error"),
        event if event.contains("compact") || event.contains("compaction") => Some("compact"),
        event if event.contains("mcp") || event.starts_with("expose_") => Some("mcp"),
        event if event.contains("sub_agent") || event.contains("subagent") => Some("agent"),
        event if event.starts_with("local_rewrite_") && event.contains("retry") => Some("retry"),
        event if event.starts_with("local_rewrite_") && event.contains("error") => Some("error"),
        "upstream_read_error"
        | "upstream_send_error"
        | "upstream_stream_error"
        | "upstream_close_before_completed"
        | "upstream_connection_closed"
        | "stream_read_error"
        | "local_writer_error"
        | "invalid_previous_response_id"
        | "session_error"
        | "local_connection_closed"
        | "profile_probe_refresh_error"
        | "smart_context_token_calibration_save_error" => Some("error"),
        _ => None,
    }
}

fn short_request_id(request: u64) -> String {
    format!("r{:04x}", request & 0xffff)
}

fn operational_event_summary(
    event: &str,
    source: &str,
    fields: &BTreeMap<String, String>,
) -> String {
    let mut details = Vec::new();
    match source {
        "request" | "model" | "route" | "mcp" | "agent" | "tool" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "provider", "provider");
            add_log_detail(&mut details, fields, "model", "model");
            add_log_detail(&mut details, fields, "from_model", "from");
            add_log_detail(&mut details, fields, "to_model", "to");
            add_log_detail(&mut details, fields, "effort", "effort");
            add_log_detail(&mut details, fields, "transport", "transport");
            add_log_detail(&mut details, fields, "method", "method");
            add_log_endpoint_detail(&mut details, fields, "path", "path");
            add_log_endpoint_detail(&mut details, fields, "url", "path");
            add_log_detail(&mut details, fields, "tool_surface", "tools");
            add_log_detail(&mut details, fields, "continuation", "continuation");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "class", "class");
            add_log_detail(&mut details, fields, "event_type", "event");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
        }
        "quota" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "quota_band", "band");
            add_log_percent_detail(&mut details, fields, "five_hour_remaining", "5h");
            add_log_percent_detail(&mut details, fields, "weekly_remaining", "week");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "until", "until");
        }
        "retry" | "backoff" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "provider", "provider");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "class", "class");
            add_log_detail(&mut details, fields, "attempt", "attempt");
            add_log_detail(&mut details, fields, "retry_index", "retry");
            add_log_detail(&mut details, fields, "seconds", "backoff_s");
            add_log_detail(&mut details, fields, "until", "until");
        }
        "health" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "score", "score");
            add_log_detail(&mut details, fields, "delta", "delta");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "upstream" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "transport", "transport");
            add_log_detail(&mut details, fields, "method", "method");
            add_log_endpoint_detail(&mut details, fields, "url", "path");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "stream" | "response" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "transport", "transport");
            if event == "first_local_chunk" {
                add_log_detail(&mut details, fields, "elapsed_ms", "ttft_ms");
            } else {
                add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
            }
            add_log_detail(&mut details, fields, "chunks", "chunks");
            add_log_detail(&mut details, fields, "bytes", "bytes");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "event_type", "event");
        }
        "smart" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "decision", "decision");
            add_log_detail(&mut details, fields, "tier", "tier");
            add_log_detail(&mut details, fields, "rewrite_kind", "rewrite");
            add_log_detail(&mut details, fields, "tokens_before", "tokens_before");
            add_log_detail(&mut details, fields, "tokens_after", "tokens_after");
            add_log_detail(&mut details, fields, "body_bytes_saved", "bytes_saved");
            add_log_percent_detail(&mut details, fields, "rewrite_ratio_percent", "rewrite");
            add_log_detail(
                &mut details,
                fields,
                "tool_outputs_condensed",
                "tools_condensed",
            );
            add_log_detail(&mut details, fields, "rehydrated_refs", "rehydrated");
            add_log_detail(&mut details, fields, "pressure_band", "pressure");
            add_log_detail(&mut details, fields, "self_check", "check");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "compact" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "provider", "provider");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "decision", "decision");
            add_log_detail(&mut details, fields, "exit", "exit");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "attempts", "attempts");
            add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
        }
        "load" => {
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "lane", "lane");
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "active", "active");
            add_log_detail(&mut details, fields, "limit", "limit");
            add_log_detail(&mut details, fields, "hard_limit", "limit");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "terminal" | "error" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "transport", "transport");
            add_log_detail(&mut details, fields, "stage", "stage");
            add_log_detail(&mut details, fields, "event_type", "event");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "class", "class");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "outcome", "outcome");
        }
        _ => {}
    }
    join_log_details(&human_event_name(event), details)
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
                "avg_output",
                event
                    .output_tokens_per_second
                    .map(|rate| format_output_tokens_per_second(Some(rate)))
                    .unwrap_or_else(|| "- t/s".to_string()),
            ),
        ];
        for line in render_log_block(&event.timestamp, "TOKENS", &meta, &[], current_log_width()) {
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
