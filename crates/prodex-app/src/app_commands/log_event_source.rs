#[cfg(feature = "mojo-core")]
use super::is_routine_load_event;
use std::collections::BTreeMap;

#[cfg(feature = "mojo-core")]
pub(super) fn operational_event_source(
    event: &str,
    _fields: &BTreeMap<String, String>,
) -> anyhow::Result<Option<&'static str>> {
    let (category, _) = prodex_mojo_core::log::classify_log_event(event)
        .map_err(|error| anyhow::anyhow!("Mojo log event classifier failed: {error:?}"))?;
    Ok(category
        .source()
        .or_else(|| is_routine_load_event(event).then_some("load"))
        .or(Some("event")))
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn operational_event_source(
    event: &str,
    fields: &BTreeMap<String, String>,
) -> anyhow::Result<Option<&'static str>> {
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
        "runtime_log_gap" | "runtime_proxy_async_log_dropped" => Some("error"),
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
        _ => Some("event"),
    }
}
