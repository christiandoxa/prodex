use terminal_ui::FieldRowsBuilder;

use crate::{RuntimeDoctorSummary, diagnosis};

fn runtime_doctor_format_option<T: ToString>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub(super) fn runtime_doctor_push_marker_detail_rows(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    if marker == "runtime_proxy_active_limit_reached" {
        fields.push(
            "Active next step",
            diagnosis::runtime_doctor_active_pressure_next_step(summary),
        );
    }
    if marker == "runtime_proxy_lane_limit_reached" {
        fields.push(
            "Lane next step",
            diagnosis::runtime_doctor_lane_pressure_next_step(summary),
        );
    }
    if marker == "profile_inflight_saturated"
        && diagnosis::runtime_doctor_marker_count(summary, "profile_inflight_saturated") > 0
    {
        fields
            .push(
                "In-flight profile",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("profile"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "In-flight hard limit",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("hard_limit"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "In-flight next step",
                diagnosis::runtime_doctor_profile_inflight_saturated_next_step(summary),
            );
    }
    if marker == "runtime_proxy_overload_backoff" {
        fields.push(
            "Connect failures",
            (diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_timeout")
                + diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_error"))
            .to_string(),
        );
    }
    if let Some(marker_fields) = summary.marker_last_fields.get(marker) {
        if marker == "local_rewrite_provider_model_fallback" {
            fields
                .push(
                    "Provider fallback",
                    format!(
                        "{} {} -> {}",
                        marker_fields
                            .get("provider")
                            .map(String::as_str)
                            .unwrap_or("provider"),
                        marker_fields
                            .get("from_model")
                            .map(String::as_str)
                            .unwrap_or("-"),
                        marker_fields
                            .get("to_model")
                            .map(String::as_str)
                            .unwrap_or("-")
                    ),
                )
                .push(
                    "Provider fallback class",
                    marker_fields
                        .get("class")
                        .cloned()
                        .unwrap_or_else(|| "-".to_string()),
                );
        }
        if marker == "local_rewrite_provider_auth_failure" {
            fields
                .push(
                    "Provider auth scope",
                    format!(
                        "{} profile={} status={}",
                        marker_fields
                            .get("provider")
                            .map(String::as_str)
                            .unwrap_or("provider"),
                        marker_fields
                            .get("profile")
                            .map(String::as_str)
                            .unwrap_or("-"),
                        marker_fields
                            .get("status")
                            .map(String::as_str)
                            .unwrap_or("-")
                    ),
                )
                .push(
                    "Provider auth error",
                    marker_fields
                        .get("class")
                        .cloned()
                        .unwrap_or_else(|| "-".to_string()),
                );
        }
        if marker == "local_rewrite_gemini_quota_rotate"
            || marker == "local_rewrite_gemini_rate_limit_retry"
        {
            fields.push(
                "Gemini retry scope",
                format!(
                    "profile={} status={} reason={} retry={} delay_ms={}",
                    marker_fields
                        .get("profile")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("status")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("reason")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("retry")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("delay_ms")
                        .map(String::as_str)
                        .unwrap_or("-")
                ),
            );
        }
        if marker == "local_rewrite_gemini_invalid_stream_retry"
            || marker == "local_rewrite_gemini_invalid_stream_model_fallback"
        {
            fields.push(
                "Gemini stream retry",
                format!(
                    "profile={} model={} from={} to={} reason={}",
                    marker_fields
                        .get("profile")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("model")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("from_model")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("to_model")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("reason")
                        .map(String::as_str)
                        .unwrap_or("-")
                ),
            );
        }
        if marker == "local_rewrite_gemini_compact_semantic"
            || marker == "local_rewrite_gemini_compact_fallback"
        {
            fields.push(
                "Gemini compact",
                format!(
                    "mode={} profile={} request={} body_bytes={} reason={}",
                    if marker == "local_rewrite_gemini_compact_semantic" {
                        "semantic"
                    } else {
                        "local-fallback"
                    },
                    marker_fields
                        .get("profile")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("request")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("body_bytes")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("reason")
                        .map(String::as_str)
                        .unwrap_or("-")
                ),
            );
        }
        if marker == "local_rewrite_gemini_live_error"
            || marker == "local_rewrite_gemini_live_sidecar_error"
            || marker == "local_rewrite_gemini_live_sidecar_session_error"
        {
            fields.push(
                "Gemini Live error",
                format!(
                    "request={} profile={} error={}",
                    marker_fields
                        .get("request")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("profile")
                        .map(String::as_str)
                        .unwrap_or("-"),
                    marker_fields
                        .get("error")
                        .map(String::as_str)
                        .unwrap_or("-")
                ),
            );
        }
    }
    let has_ws_overflow_rejected =
        diagnosis::runtime_doctor_marker_count(summary, "websocket_connect_overflow_rejected") > 0;
    let has_ws_overflow_reject =
        diagnosis::runtime_doctor_marker_count(summary, "websocket_connect_overflow_reject") > 0;
    let has_ws_overflow_enqueue =
        diagnosis::runtime_doctor_marker_count(summary, "websocket_connect_overflow_enqueue") > 0;
    let should_render_ws_overflow_detail = marker == "websocket_connect_overflow_rejected"
        || marker == "websocket_connect_overflow_reject" && !has_ws_overflow_rejected
        || marker == "websocket_connect_overflow_enqueue"
            && !has_ws_overflow_rejected
            && !has_ws_overflow_reject
        || marker == "websocket_connect_overflow_dispatch"
            && !has_ws_overflow_rejected
            && !has_ws_overflow_reject
            && !has_ws_overflow_enqueue;
    if should_render_ws_overflow_detail {
        fields
            .push(
                "WS overflow reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "WS overflow pending",
                summary
                    .marker_last_fields
                    .get(marker)
                    .map(|fields| {
                        format!(
                            "{}/{}",
                            fields
                                .get("overflow_pending")
                                .map(String::as_str)
                                .unwrap_or("-"),
                            fields
                                .get("overflow_max_pending")
                                .map(String::as_str)
                                .unwrap_or("-")
                        )
                    })
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "WS overflow next step",
                diagnosis::runtime_doctor_websocket_connect_overflow_next_step(summary),
            );
    }
    if marker == "profile_auth_recovery_failed"
        || marker == "profile_auth_recovered"
            && diagnosis::runtime_doctor_marker_count(summary, "profile_auth_recovery_failed") == 0
    {
        fields
            .push(
                "Auth recovery profile",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("profile"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Auth recovery route",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("route"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Auth recovery source",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("source"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Auth recovery error",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("error"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Auth recovery next step",
                diagnosis::runtime_doctor_profile_auth_recovery_next_step(summary),
            );
    }
    if marker == "previous_response_not_found" {
        fields
            .push(
                "Prev not found routes",
                diagnosis::runtime_doctor_count_breakdown(
                    &summary.previous_response_not_found_by_route,
                ),
            )
            .push(
                "Prev not found xport",
                diagnosis::runtime_doctor_count_breakdown(
                    &summary.previous_response_not_found_by_transport,
                ),
            );
    }
    if marker == "previous_response_fresh_fallback" {
        fields
            .push(
                "Legacy fallback shape",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("request_shape"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Legacy fallback reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Legacy fallback note",
                "Current runtime should fail closed; restart active prodex/codex sessions if this marker came from a live broker.",
            );
    }
    if marker == "previous_response_fresh_fallback_blocked" {
        fields
            .push(
                "Continuation shape",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("request_shape"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Continuation reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Fail-closed shapes",
                diagnosis::runtime_doctor_count_breakdown(
                    &summary.previous_response_fresh_fallback_blocked_by_request_shape,
                ),
            )
            .push(
                "Continuation next step",
                diagnosis::runtime_doctor_previous_response_fail_closed_next_step(summary),
            );
    }
    if marker == "stale_continuation" {
        fields
            .push(
                "Chain retry reasons",
                diagnosis::runtime_doctor_count_breakdown(&summary.chain_retried_owner_by_reason),
            )
            .push(
                "Chain dead reasons",
                diagnosis::runtime_doctor_count_breakdown(
                    &summary.chain_dead_upstream_confirmed_by_reason,
                ),
            )
            .push(
                "Stale reasons",
                diagnosis::runtime_doctor_count_breakdown(&summary.stale_continuation_by_reason),
            )
            .push(
                "Latest stale reason",
                summary
                    .latest_stale_continuation_reason
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Latest chain event",
                summary
                    .latest_chain_event
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            );
    }
    if marker == "local_writer_error" {
        fields
            .push(
                "State save backlog",
                runtime_doctor_format_option(summary.state_save_queue_backlog),
            )
            .push(
                "State save lag",
                runtime_doctor_format_option(summary.state_save_lag_ms),
            )
            .push(
                "Cont journal backlog",
                runtime_doctor_format_option(summary.continuation_journal_save_backlog),
            )
            .push(
                "Cont journal lag",
                runtime_doctor_format_option(summary.continuation_journal_save_lag_ms),
            )
            .push(
                "Probe backlog",
                runtime_doctor_format_option(summary.profile_probe_refresh_backlog),
            )
            .push(
                "Probe lag",
                runtime_doctor_format_option(summary.profile_probe_refresh_lag_ms),
            );
    }
    if marker == "state_save_queue_backpressure"
        && diagnosis::runtime_doctor_marker_count(summary, "state_save_queue_backpressure") > 0
    {
        fields
            .push(
                "State pressure reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "State pressure backlog",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("backlog"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Persistence next step",
                diagnosis::runtime_doctor_persistence_backpressure_next_step(summary),
            );
    }
    if marker == "continuation_journal_queue_backpressure"
        && diagnosis::runtime_doctor_marker_count(
            summary,
            "continuation_journal_queue_backpressure",
        ) > 0
    {
        fields
            .push(
                "Cont journal pressure reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Cont journal pressure backlog",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("backlog"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            );
    }
    if marker == "selection_skip_sync_probe"
        && diagnosis::runtime_doctor_marker_count(summary, "selection_skip_sync_probe") > 0
    {
        let deferred = summary
            .marker_last_fields
            .get(marker)
            .and_then(|fields| {
                fields
                    .get("cold_start_jobs")
                    .map(|count| format!("{count} job(s)"))
                    .or_else(|| {
                        fields
                            .get("cold_start_profiles")
                            .map(|count| format!("{count} profile(s)"))
                    })
            })
            .unwrap_or_else(|| "-".to_string());
        fields
            .push(
                "Sync-probe route",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("route"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Sync-probe reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push("Sync-probe deferred", deferred)
            .push(
                "Sync-probe next step",
                diagnosis::runtime_doctor_sync_probe_skip_next_step(summary),
            );
    }
    if marker == "selection_plan" && diagnosis::runtime_doctor_marker_count(summary, marker) > 0 {
        let last_fields = summary.marker_last_fields.get(marker);
        let field = |name: &str| {
            last_fields
                .and_then(|fields| fields.get(name))
                .cloned()
                .unwrap_or_else(|| "-".to_string())
        };
        fields
            .push("Selection route", field("route"))
            .push("Ready candidates", field("ready"))
            .push("Fallback candidates", field("fallback"))
            .push("Excluded profiles", field("excluded_count"))
            .push("Cold-start jobs", field("cold_start_jobs"))
            .push("Sync-probe jobs", field("sync_probe_jobs"))
            .push("Sync-probe mode", field("sync_probe_mode"))
            .push("Pressure mode", field("pressure_mode"));
    }
    if marker == "profile_probe_refresh_backpressure"
        && diagnosis::runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure") > 0
    {
        fields
            .push(
                "Probe pressure profile",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("profile"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Probe pressure backlog",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("backlog"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Probe next step",
                diagnosis::runtime_doctor_probe_refresh_backpressure_next_step(summary),
            );
    }
    if marker == "runtime_proxy_startup_audit" {
        fields.push("Startup pressure", summary.startup_audit_pressure.clone());
    }
    if marker == "compact_final_failure" {
        fields
            .push(
                "Compact exit",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("exit"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Compact reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Compact last fail",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("last_failure"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Compact next step",
                diagnosis::runtime_doctor_compact_final_failure_next_step(summary),
            );
    }
    if marker == "profile_health" {
        fields
            .push(
                "Health route",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("route"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Health profile",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("profile"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Health score",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("score"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Health reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Health next step",
                diagnosis::runtime_doctor_route_health_next_step(summary),
            );
    }
    if marker == "compat_request_surface" {
        fields
            .push("Compat warnings", summary.compat_warning_count.to_string())
            .push(
                "Client family",
                summary
                    .top_client_family
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Top client",
                summary
                    .top_client
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Tool surface",
                summary
                    .top_tool_surface
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Compat warning",
                summary
                    .top_compat_warning
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Hot lane",
                diagnosis::runtime_doctor_top_facet(summary, "lane")
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Hot route",
                diagnosis::runtime_doctor_top_facet(summary, "route")
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Hot profile",
                diagnosis::runtime_doctor_top_facet(summary, "profile")
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Hot reason",
                diagnosis::runtime_doctor_top_facet(summary, "reason")
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Quota source",
                diagnosis::runtime_doctor_top_facet(summary, "quota_source")
                    .unwrap_or_else(|| "-".to_string()),
            );
    }
}
