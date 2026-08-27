use chrono::{DateTime, Local};
use std::borrow::Cow;

pub(super) fn current_log_width() -> usize {
    terminal_ui::current_cli_width()
}

pub(super) fn local_log_timestamp(timestamp: &str) -> String {
    parse_log_timestamp(timestamp)
        .map(|datetime| {
            datetime
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S%.3f %:z")
                .to_string()
        })
        .unwrap_or_else(|| timestamp.to_string())
}

pub(super) fn render_log_block(
    timestamp: &str,
    title: &str,
    meta: &[(&str, String)],
    body: &[String],
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(render_header(timestamp, title, width));
    if !meta.is_empty() {
        lines.extend(render_meta(meta, width));
    }
    if !body.is_empty() {
        lines.extend(render_body(body, width));
    }
    lines
}

pub(super) fn render_text_body(text: &str, width: usize) -> Vec<String> {
    let body_width = width.saturating_sub(4).max(20);
    terminal_ui::wrap_text(text, body_width)
}

pub(super) fn human_event_name(event: &str) -> Cow<'_, str> {
    let name = match event {
        "request_captured" => "received request",
        "route_decision" => "route decided",
        "selection_plan" => "route planned",
        "selection_pick" => "profile picked",
        "selection_keep_affinity" => "owner kept",
        "selection_keep_current" => "current profile kept",
        "selection_skip_current" => "profile skipped",
        "selection_skip_affinity" => "affinity skipped",
        "selection_skip_sync_probe" => "quota probe skipped",
        "local_selection_blocked" => "route blocked",
        "profile_commit" => "profile committed",
        "route_affinity_recompute" => "affinity recomputed",
        "route_affinity_recompute_result" => "affinity resolved",
        "previous_response_owner" => "continuation owner",
        "previous_response_not_found" => "continuation missing",
        "previous_response_negative_cache" => "continuation cached missing",
        "previous_response_fresh_fallback" => "fresh fallback",
        "previous_response_fresh_fallback_blocked" => "fallback blocked",
        "previous_response_turn_state_rehydrated" => "turn state restored",
        "session_rotation_release_affinity" => "session affinity released",
        "binding_prompt_cache" => "prompt cache bound",
        "upgrade" | "upgraded" => "request upgraded",
        "profile_quota_exhausted" | "quota_exhausted" => "quota exhausted",
        "quota_blocked" => "quota blocked",
        "quota_critical_floor_before_send" => "quota floor blocked",
        "profile_quota_quarantine" => "quota quarantine",
        "profile_probe_refresh_start" => "quota refresh started",
        "profile_probe_refresh_ok" => "quota refreshed",
        "upstream_usage_limit_passthrough" => "upstream limit passed through",
        "upstream_overload_passthrough" => "upstream overload passed through",
        "profile_retry_backoff" => "retry backoff",
        "compact_retryable_failure" => "compaction retry",
        "compact_overload_conservative_retry" => "compaction retry (overload)",
        "profile_transport_backoff" => "transport backoff",
        "rotation_waiting_for_recovery" => "waiting for recovery",
        "profile_circuit_open" => "circuit open",
        "profile_circuit_half_open_probe" => "circuit probe",
        "profile_transport_failure" => "transport failed",
        "profile_health" => "health penalty",
        "profile_bad_pairing" => "affinity penalty",
        "upstream_start" | "upstream_async_start" => "upstream request",
        "upstream_response" | "upstream_async_response" => "upstream response",
        "upstream_connect_start" => "upstream connecting",
        "upstream_connect_ok" => "upstream connected",
        "upstream_connect_error" => "upstream connect failed",
        "first_upstream_chunk" => "first upstream chunk",
        "first_local_chunk" => "first local chunk",
        "stream_complete" => "stream complete",
        "buffered_response_complete" => "response complete",
        "terminal_event" => "terminal event",
        "runtime_proxy_queue_overloaded" => "proxy queue full",
        "runtime_proxy_active_limit_reached" => "proxy busy",
        "runtime_proxy_lane_limit_reached" => "lane full",
        "profile_inflight_saturated" => "profile busy",
        "smart_context_autopilot" => "Smart Context",
        "smart_context_prepare_error" => "Smart Context failed",
        "smart_context_prepare_fallback" => "Smart Context fallback",
        "smart_context_disabled" => "Smart Context disabled",
        "local_rewrite_request_detail" => "provider request",
        "local_rewrite_provider_model_fallback" => "model fallback",
        "local_rewrite_provider_auth_failure" => "provider auth failed",
        "upstream_read_error" => "upstream read failed",
        "upstream_send_error" => "upstream send failed",
        "upstream_stream_error" => "upstream stream failed",
        "upstream_close_before_completed" => "upstream closed early",
        "upstream_connection_closed" => "upstream disconnected",
        "stream_read_error" => "stream read failed",
        "local_writer_error" => "terminal write failed",
        "invalid_previous_response_id" => "continuation invalid",
        "session_error" => "session failed",
        "local_connection_closed" => "local connection closed",
        "profile_probe_refresh_error" => "quota refresh failed",
        "smart_context_token_calibration_save_error" => "Smart Context calibration failed",
        _ if event.contains("compact") || event.contains("compaction") => "compaction",
        _ if event.contains("mcp") || event.starts_with("expose_") => "MCP",
        _ if event.contains("sub_agent") || event.contains("subagent") => "sub-agent",
        _ => return Cow::Owned(event.replace('_', " ")),
    };
    Cow::Borrowed(name)
}

fn parse_log_timestamp(timestamp: &str) -> Option<DateTime<chrono::FixedOffset>> {
    DateTime::parse_from_rfc3339(timestamp)
        .or_else(|_| DateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S%.f %:z"))
        .or_else(|_| DateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S %:z"))
        .ok()
}

fn render_header(timestamp: &str, title: &str, width: usize) -> String {
    let prefix = format!("[{timestamp}] {title}");
    if terminal_ui::text_width(&prefix) >= width.saturating_sub(1) {
        return prefix;
    }
    let fill = width.saturating_sub(terminal_ui::text_width(&prefix) + 1);
    format!("{prefix} {}", "-".repeat(fill))
}

fn render_meta(meta: &[(&str, String)], width: usize) -> Vec<String> {
    let text = meta
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("  ");
    terminal_ui::wrap_text(&text, width.saturating_sub(2).max(20))
        .into_iter()
        .map(|line| format!("  {line}"))
        .collect()
}

fn render_body(body: &[String], width: usize) -> Vec<String> {
    let body_width = width.saturating_sub(4).max(20);
    let mut lines = Vec::new();
    for block in body {
        for line in terminal_ui::wrap_text(block, body_width) {
            lines.push(format!("  | {line}"));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Local};

    #[test]
    fn renders_block_with_header_meta_and_body() {
        let lines = render_log_block(
            "2026-06-22T01:00:00Z",
            "stream assistant",
            &[
                ("profile", "main".to_string()),
                ("request", "7".to_string()),
            ],
            &["hello terminal".to_string()],
            72,
        );
        let rendered = lines.join("\n");

        assert!(rendered.contains("[2026-06-22T01:00:00Z] stream assistant"));
        assert!(rendered.contains("profile=main"));
        assert!(rendered.contains("request=7"));
        assert!(rendered.contains("| hello terminal"));
    }

    #[test]
    fn local_log_timestamp_converts_rfc3339_to_local_time() {
        let input = "2026-06-20T01:00:00Z";
        let expected = DateTime::parse_from_rfc3339(input)
            .unwrap()
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S%.3f %:z")
            .to_string();

        assert_eq!(local_log_timestamp(input), expected);
        assert_ne!(local_log_timestamp(input), input);
    }

    #[test]
    fn local_log_timestamp_keeps_unknown_values() {
        assert_eq!(local_log_timestamp("-"), "-");
    }
}
