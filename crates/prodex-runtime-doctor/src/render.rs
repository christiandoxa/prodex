use std::path::Path;

use chrono::{Local, TimeZone};
use terminal_ui::FieldRowsBuilder;

use super::*;

mod broker;
mod json;

pub use json::{runtime_doctor_json_value, runtime_doctor_json_value_with_policy_suggestions};

use broker::runtime_doctor_runtime_broker_issue_lines;

fn format_precise_reset_time(epoch: Option<i64>) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %:z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn runtime_doctor_format_option<T: ToString>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn runtime_doctor_binding_source_text(source: &RuntimeDoctorBindingSourceSummary) -> String {
    let top_profile = source
        .profiles
        .first()
        .map(|profile| format!("{}={}", profile.profile, profile.total_bindings))
        .unwrap_or_else(|| "-".to_string());
    format!(
        "r={} s={} t={} sid={} total={} profiles={} top={}",
        source.response_bindings,
        source.session_bindings,
        source.turn_state_bindings,
        source.session_id_bindings,
        source.total_bindings,
        source.profile_count,
        top_profile
    )
}

fn runtime_doctor_binding_state_text(summary: &RuntimeDoctorBindingStateSummary) -> String {
    let missing = summary.state.missing_profile_bindings
        + summary.runtime_continuations.missing_profile_bindings
        + summary.continuation_journal.missing_profile_bindings
        + summary.merged_continuations.missing_profile_bindings;
    format!(
        "active={} profiles={} selected={} | state {} | runtime {} | journal {} | merged {} | missing={}",
        summary.active_profile.as_deref().unwrap_or("-"),
        summary.profile_count,
        summary.last_run_selected_profiles,
        runtime_doctor_binding_source_text(&summary.state),
        runtime_doctor_binding_source_text(&summary.runtime_continuations),
        runtime_doctor_binding_source_text(&summary.continuation_journal),
        runtime_doctor_binding_source_text(&summary.merged_continuations),
        missing
    )
}

fn runtime_doctor_request_timeline_text(summary: &RuntimeDoctorSummary) -> String {
    let Some(request_id) = summary.latest_request_id.as_deref() else {
        return "-".to_string();
    };
    if summary.latest_request_timeline.is_empty() {
        return format!("request={request_id}");
    }
    let events = summary
        .latest_request_timeline
        .iter()
        .map(|event| {
            let mut text = format!("{}:{}", event.phase, event.marker);
            if !event.detail.is_empty() {
                text.push(' ');
                text.push_str(&event.detail);
            }
            text
        })
        .collect::<Vec<_>>()
        .join(" -> ");
    format!("request={request_id} {events}")
}

fn runtime_doctor_incident_explainer_text(incident: &RuntimeDoctorIncidentExplanation) -> String {
    let evidence = if incident.evidence.is_empty() {
        "-".to_string()
    } else {
        incident.evidence.join(", ")
    };
    format!(
        "{} Evidence: {evidence}. Next: {}",
        incident.cause, incident.next_action
    )
}

fn runtime_doctor_push_incident_explainer_rows(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
) {
    let incidents = diagnosis::runtime_doctor_incident_explainer(summary);
    if incidents.is_empty() {
        fields.push("Incident explainer", "-");
        return;
    }
    for (index, incident) in incidents.iter().take(6).enumerate() {
        fields.push(
            format!("Incident {}", index + 1),
            runtime_doctor_incident_explainer_text(incident),
        );
    }
    let hidden = incidents.len().saturating_sub(6);
    if hidden > 0 {
        fields.push("Incident hidden", hidden.to_string());
    }
}

fn runtime_doctor_push_marker_detail_rows(
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

fn runtime_doctor_push_summary_tail_rows(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    broker_issues: &[String],
    suspect_continuations: &str,
) {
    fields
        .push("Selection pressure", summary.selection_pressure.clone())
        .push("Transport pressure", summary.transport_pressure.clone())
        .push("Persistence pressure", summary.persistence_pressure.clone())
        .push("Quota freshness", summary.quota_freshness_pressure.clone())
        .push(
            "Failure classes",
            diagnosis::runtime_doctor_count_breakdown(&summary.failure_class_counts),
        )
        .push(
            "Persisted backoffs",
            format!(
                "retry={} transport={} circuits={}",
                summary.persisted_retry_backoffs,
                summary.persisted_transport_backoffs,
                summary.persisted_route_circuits
            ),
        )
        .push(
            "Persisted snapshots",
            format!(
                "{} total, {} stale",
                summary.persisted_usage_snapshots, summary.stale_persisted_usage_snapshots
            ),
        )
        .push(
            "Persisted continuations",
            format!(
                "responses={} sessions={} turns={} session_ids={} turn_coverage={}",
                summary.persisted_response_bindings,
                summary.persisted_session_bindings,
                summary.persisted_turn_state_bindings,
                summary.persisted_session_id_bindings,
                summary
                    .persisted_turn_state_coverage_percent
                    .map(|percent| format!("{percent}%"))
                    .unwrap_or_else(|| "-".to_string())
            ),
        )
        .push(
            "Binding state",
            runtime_doctor_binding_state_text(&summary.binding_state),
        )
        .push(
            "Continuation states",
            format!(
                "verified={} warm={} suspect={} dead={}",
                summary.persisted_verified_continuations,
                summary.persisted_warm_continuations,
                summary.persisted_suspect_continuations,
                summary.persisted_dead_continuations
            ),
        )
        .push(
            "Continuation journal",
            format!(
                "responses={} sessions={} turns={} session_ids={} saved_at={}",
                summary.persisted_continuation_journal_response_bindings,
                summary.persisted_continuation_journal_session_bindings,
                summary.persisted_continuation_journal_turn_state_bindings,
                summary.persisted_continuation_journal_session_id_bindings,
                summary
                    .continuation_journal_saved_at
                    .map(|epoch| format_precise_reset_time(Some(epoch)))
                    .unwrap_or_else(|| "-".to_string())
            ),
        )
        .push(
            "Recovered state",
            format!(
                "state={} continuations={} journal={} scores={} usage={} backoffs={} backups={}",
                summary.recovered_state_file,
                summary.recovered_continuations_file,
                summary.recovered_continuation_journal_file,
                summary.recovered_scores_file,
                summary.recovered_usage_snapshots_file,
                summary.recovered_backoffs_file,
                summary.last_good_backups_present
            ),
        )
        .push(
            "Degraded routes",
            if summary.degraded_routes.is_empty() {
                "-".to_string()
            } else {
                summary.degraded_routes.join(" | ")
            },
        )
        .push(
            "Orphan dirs",
            if summary.orphan_managed_dirs.is_empty() {
                "-".to_string()
            } else {
                summary.orphan_managed_dirs.join(", ")
            },
        )
        .push(
            "Prodex binaries",
            if summary.prodex_binary_identities.is_empty() {
                "-".to_string()
            } else {
                summary.prodex_binary_identities.join(" | ")
            },
        )
        .push(
            "Runtime brokers",
            if summary.runtime_broker_identities.is_empty() {
                "-".to_string()
            } else {
                summary.runtime_broker_identities.join(" | ")
            },
        )
        .push(
            "Broker issues",
            if broker_issues.is_empty() {
                "-".to_string()
            } else {
                broker_issues.join(" | ")
            },
        )
        .push(
            "Binary mismatch",
            format!(
                "installed={} broker={}",
                summary.prodex_binary_mismatch, summary.runtime_broker_mismatch
            ),
        )
        .push("Suspect continuations", suspect_continuations)
        .push(
            "Latest request timeline",
            runtime_doctor_request_timeline_text(summary),
        )
        .push(
            "Last marker",
            summary
                .last_marker_line
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        )
        .push("Diagnosis", summary.diagnosis.clone());
}

pub fn runtime_doctor_fields_for_summary(
    summary: &RuntimeDoctorSummary,
    pointer_path: &Path,
) -> Vec<(String, String)> {
    let latest_log = summary
        .log_path
        .as_ref()
        .map(|path| {
            format!(
                "{} ({})",
                path.display(),
                if summary.log_exists {
                    "exists"
                } else {
                    "missing"
                }
            )
        })
        .unwrap_or_else(|| "-".to_string());
    let suspect_continuations = if summary.suspect_continuation_bindings.is_empty() {
        "-".to_string()
    } else {
        format!(
            "count={} bindings={}",
            summary.persisted_suspect_continuations,
            summary.suspect_continuation_bindings.join(", ")
        )
    };
    let broker_issues = runtime_doctor_runtime_broker_issue_lines(summary);
    let mut fields = FieldRowsBuilder::new();
    fields
        .push(
            "Log pointer",
            format!(
                "{} ({})",
                pointer_path.display(),
                if summary.pointer_exists {
                    "exists"
                } else {
                    "missing"
                }
            ),
        )
        .push("Latest log", latest_log)
        .push("Log sample", format!("{} lines", summary.line_count));
    runtime_doctor_push_incident_explainer_rows(&mut fields, summary);
    for (label, marker) in RUNTIME_DOCTOR_COUNT_FIELD_ROWS {
        fields.push(
            *label,
            diagnosis::runtime_doctor_marker_count(summary, marker).to_string(),
        );
        runtime_doctor_push_marker_detail_rows(&mut fields, summary, marker);
    }
    runtime_doctor_push_summary_tail_rows(
        &mut fields,
        summary,
        &broker_issues,
        &suspect_continuations,
    );
    fields.build()
}

#[cfg(test)]
#[path = "../tests/src/render.rs"]
mod tests;
