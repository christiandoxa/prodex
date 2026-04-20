use std::collections::BTreeMap;
use std::path::Path;

use super::*;

fn runtime_doctor_parse_broker_artifact(line: &str) -> BTreeMap<String, String> {
    line.split_whitespace()
        .filter_map(|token| token.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn runtime_doctor_runtime_broker_issue_lines(summary: &RuntimeDoctorSummary) -> Vec<String> {
    summary
        .runtime_broker_identities
        .iter()
        .filter_map(|line| {
            let artifact = runtime_doctor_parse_broker_artifact(line);
            let broker_key = artifact.get("broker_key")?;
            let status = artifact.get("status").map(String::as_str).unwrap_or("unknown");
            let pid = artifact.get("pid").map(String::as_str).unwrap_or("-");
            let stale_leases = artifact
                .get("stale_leases")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            let issue = match status {
                "dead_pid" => Some(format!(
                    "{broker_key}: registry points to dead pid {pid}; run prodex cleanup or restart prodex run"
                )),
                "health_timeout" => Some(format!(
                    "{broker_key}: pid {pid} health probe timed out; check local listener then restart prodex run if it stays stuck"
                )),
                "health_unreachable" => Some(format!(
                    "{broker_key}: pid {pid} health probe unreachable; check local listener then restart prodex run if needed"
                )),
                "binary_mismatch" => Some(format!(
                    "{broker_key}: pid {pid} runs different prodex binary; restart active prodex/codex sessions"
                )),
                _ => None,
            };
            match (issue, stale_leases) {
                (Some(issue), leases) if leases > 0 => Some(format!(
                    "{issue}; {leases} stale lease(s) remain, run prodex cleanup after old terminals exit"
                )),
                (Some(issue), _) => Some(issue),
                (None, leases) if leases > 0 => Some(format!(
                    "{broker_key}: {leases} stale lease(s) remain; run prodex cleanup after old terminals exit"
                )),
                (None, _) => None,
            }
        })
        .collect()
}

fn runtime_doctor_json_entry<T: serde::Serialize>(
    key: &str,
    value: T,
) -> (String, serde_json::Value) {
    (
        key.to_string(),
        serde_json::to_value(value).expect("runtime doctor serialization should always succeed"),
    )
}

fn runtime_doctor_json_map<K, V, I>(entries: I) -> serde_json::Map<String, serde_json::Value>
where
    K: Into<String>,
    V: serde::Serialize,
    I: IntoIterator<Item = (K, V)>,
{
    entries
        .into_iter()
        .map(|(key, value)| {
            (
                key.into(),
                serde_json::to_value(value)
                    .expect("runtime doctor serialization should always succeed"),
            )
        })
        .collect()
}

fn runtime_doctor_profile_json(profile: &RuntimeDoctorProfileSummary) -> serde_json::Value {
    serde_json::json!({
        "profile": profile.profile,
        "quota_freshness": profile.quota_freshness,
        "quota_age_seconds": profile.quota_age_seconds,
        "retry_backoff_until": profile.retry_backoff_until,
        "transport_backoff_until": profile.transport_backoff_until,
        "routes": profile.routes.iter().map(|route| {
            serde_json::json!({
                "route": route.route,
                "circuit_state": route.circuit_state,
                "circuit_until": route.circuit_until,
                "transport_backoff_until": route.transport_backoff_until,
                "health_score": route.health_score,
                "bad_pairing_score": route.bad_pairing_score,
                "performance_score": route.performance_score,
                "quota_band": route.quota_band,
                "five_hour_status": route.five_hour_status,
                "weekly_status": route.weekly_status,
            })
        }).collect::<Vec<_>>(),
    })
}

pub(crate) fn runtime_doctor_json_value(summary: &RuntimeDoctorSummary) -> serde_json::Value {
    serde_json::Value::Object(
        [
            runtime_doctor_json_entry(
                "log_path",
                summary
                    .log_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
            ),
            runtime_doctor_json_entry("pointer_exists", summary.pointer_exists),
            runtime_doctor_json_entry("log_exists", summary.log_exists),
            runtime_doctor_json_entry("line_count", summary.line_count),
            runtime_doctor_json_entry("first_timestamp", summary.first_timestamp.clone()),
            runtime_doctor_json_entry("last_timestamp", summary.last_timestamp.clone()),
            runtime_doctor_json_entry("compat_warning_count", summary.compat_warning_count),
            runtime_doctor_json_entry("top_client_family", summary.top_client_family.clone()),
            runtime_doctor_json_entry("top_client", summary.top_client.clone()),
            runtime_doctor_json_entry("top_tool_surface", summary.top_tool_surface.clone()),
            runtime_doctor_json_entry("top_compat_warning", summary.top_compat_warning.clone()),
            runtime_doctor_json_entry(
                "marker_counts",
                runtime_doctor_json_map(
                    summary
                        .marker_counts
                        .iter()
                        .map(|(marker, count)| ((*marker).to_string(), *count)),
                ),
            ),
            runtime_doctor_json_entry(
                "marker_last_fields",
                runtime_doctor_json_map(summary.marker_last_fields.iter().map(
                    |(marker, fields)| {
                        (
                            (*marker).to_string(),
                            runtime_doctor_json_map(fields.clone()),
                        )
                    },
                )),
            ),
            runtime_doctor_json_entry(
                "facet_counts",
                runtime_doctor_json_map(summary.facet_counts.iter().map(|(facet, counts)| {
                    (facet.clone(), runtime_doctor_json_map(counts.clone()))
                })),
            ),
            runtime_doctor_json_entry(
                "previous_response_not_found_by_route",
                runtime_doctor_json_map(summary.previous_response_not_found_by_route.clone()),
            ),
            runtime_doctor_json_entry(
                "previous_response_not_found_by_transport",
                runtime_doctor_json_map(summary.previous_response_not_found_by_transport.clone()),
            ),
            runtime_doctor_json_entry(
                "chain_retried_owner_by_reason",
                runtime_doctor_json_map(summary.chain_retried_owner_by_reason.clone()),
            ),
            runtime_doctor_json_entry(
                "chain_dead_upstream_confirmed_by_reason",
                runtime_doctor_json_map(summary.chain_dead_upstream_confirmed_by_reason.clone()),
            ),
            runtime_doctor_json_entry(
                "stale_continuation_by_reason",
                runtime_doctor_json_map(summary.stale_continuation_by_reason.clone()),
            ),
            runtime_doctor_json_entry("latest_chain_event", summary.latest_chain_event.clone()),
            runtime_doctor_json_entry(
                "latest_stale_continuation_reason",
                summary.latest_stale_continuation_reason.clone(),
            ),
            runtime_doctor_json_entry("last_marker_line", summary.last_marker_line.clone()),
            runtime_doctor_json_entry("selection_pressure", summary.selection_pressure.clone()),
            runtime_doctor_json_entry("transport_pressure", summary.transport_pressure.clone()),
            runtime_doctor_json_entry("persistence_pressure", summary.persistence_pressure.clone()),
            runtime_doctor_json_entry(
                "quota_freshness_pressure",
                summary.quota_freshness_pressure.clone(),
            ),
            runtime_doctor_json_entry(
                "startup_audit_pressure",
                summary.startup_audit_pressure.clone(),
            ),
            runtime_doctor_json_entry("persisted_retry_backoffs", summary.persisted_retry_backoffs),
            runtime_doctor_json_entry(
                "persisted_transport_backoffs",
                summary.persisted_transport_backoffs,
            ),
            runtime_doctor_json_entry("persisted_route_circuits", summary.persisted_route_circuits),
            runtime_doctor_json_entry(
                "persisted_usage_snapshots",
                summary.persisted_usage_snapshots,
            ),
            runtime_doctor_json_entry(
                "persisted_response_bindings",
                summary.persisted_response_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_session_bindings",
                summary.persisted_session_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_turn_state_bindings",
                summary.persisted_turn_state_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_session_id_bindings",
                summary.persisted_session_id_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_verified_continuations",
                summary.persisted_verified_continuations,
            ),
            runtime_doctor_json_entry(
                "persisted_warm_continuations",
                summary.persisted_warm_continuations,
            ),
            runtime_doctor_json_entry(
                "persisted_suspect_continuations",
                summary.persisted_suspect_continuations,
            ),
            runtime_doctor_json_entry(
                "persisted_dead_continuations",
                summary.persisted_dead_continuations,
            ),
            runtime_doctor_json_entry(
                "persisted_continuation_journal_response_bindings",
                summary.persisted_continuation_journal_response_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_continuation_journal_session_bindings",
                summary.persisted_continuation_journal_session_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_continuation_journal_turn_state_bindings",
                summary.persisted_continuation_journal_turn_state_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_continuation_journal_session_id_bindings",
                summary.persisted_continuation_journal_session_id_bindings,
            ),
            runtime_doctor_json_entry(
                "persisted_turn_state_coverage_percent",
                summary.persisted_turn_state_coverage_percent,
            ),
            runtime_doctor_json_entry("state_save_queue_backlog", summary.state_save_queue_backlog),
            runtime_doctor_json_entry("state_save_lag_ms", summary.state_save_lag_ms),
            runtime_doctor_json_entry(
                "continuation_journal_save_backlog",
                summary.continuation_journal_save_backlog,
            ),
            runtime_doctor_json_entry(
                "continuation_journal_save_lag_ms",
                summary.continuation_journal_save_lag_ms,
            ),
            runtime_doctor_json_entry(
                "profile_probe_refresh_backlog",
                summary.profile_probe_refresh_backlog,
            ),
            runtime_doctor_json_entry(
                "profile_probe_refresh_lag_ms",
                summary.profile_probe_refresh_lag_ms,
            ),
            runtime_doctor_json_entry(
                "continuation_journal_saved_at",
                summary.continuation_journal_saved_at,
            ),
            runtime_doctor_json_entry(
                "suspect_continuation_bindings",
                summary.suspect_continuation_bindings.clone(),
            ),
            runtime_doctor_json_entry(
                "stale_persisted_usage_snapshots",
                summary.stale_persisted_usage_snapshots,
            ),
            runtime_doctor_json_entry("recovered_state_file", summary.recovered_state_file),
            runtime_doctor_json_entry(
                "recovered_continuations_file",
                summary.recovered_continuations_file,
            ),
            runtime_doctor_json_entry(
                "recovered_continuation_journal_file",
                summary.recovered_continuation_journal_file,
            ),
            runtime_doctor_json_entry("recovered_scores_file", summary.recovered_scores_file),
            runtime_doctor_json_entry(
                "recovered_usage_snapshots_file",
                summary.recovered_usage_snapshots_file,
            ),
            runtime_doctor_json_entry("recovered_backoffs_file", summary.recovered_backoffs_file),
            runtime_doctor_json_entry(
                "last_good_backups_present",
                summary.last_good_backups_present,
            ),
            runtime_doctor_json_entry("degraded_routes", summary.degraded_routes.clone()),
            runtime_doctor_json_entry("orphan_managed_dirs", summary.orphan_managed_dirs.clone()),
            runtime_doctor_json_entry(
                "prodex_binary_identities",
                summary.prodex_binary_identities.clone(),
            ),
            runtime_doctor_json_entry(
                "runtime_broker_identities",
                summary.runtime_broker_identities.clone(),
            ),
            runtime_doctor_json_entry(
                "runtime_broker_artifacts",
                summary
                    .runtime_broker_identities
                    .iter()
                    .map(|line| runtime_doctor_json_map(runtime_doctor_parse_broker_artifact(line)))
                    .collect::<Vec<_>>(),
            ),
            runtime_doctor_json_entry("prodex_binary_mismatch", summary.prodex_binary_mismatch),
            runtime_doctor_json_entry("runtime_broker_mismatch", summary.runtime_broker_mismatch),
            runtime_doctor_json_entry(
                "failure_class_counts",
                runtime_doctor_json_map(summary.failure_class_counts.clone()),
            ),
            runtime_doctor_json_entry(
                "profiles",
                summary
                    .profiles
                    .iter()
                    .map(runtime_doctor_profile_json)
                    .collect::<Vec<_>>(),
            ),
            runtime_doctor_json_entry("diagnosis", summary.diagnosis.clone()),
        ]
        .into_iter()
        .collect(),
    )
}

pub(crate) fn runtime_doctor_fields() -> Vec<(String, String)> {
    let pointer_path = runtime_proxy_latest_log_pointer_path();
    let summary = state::collect_runtime_doctor_summary();
    runtime_doctor_fields_for_summary(&summary, &pointer_path)
}

fn runtime_doctor_format_option<T: ToString>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn runtime_doctor_fields_for_summary(
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
    for (label, marker) in RUNTIME_DOCTOR_COUNT_FIELD_ROWS {
        fields.push(
            *label,
            diagnosis::runtime_doctor_marker_count(summary, marker).to_string(),
        );
        if *marker == "runtime_proxy_overload_backoff" {
            fields.push(
                "Connect failures",
                (diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_timeout")
                    + diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_error"))
                .to_string(),
            );
        }
        if *marker == "previous_response_not_found" {
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
        if *marker == "stale_continuation" {
            fields
                .push(
                    "Chain retry reasons",
                    diagnosis::runtime_doctor_count_breakdown(
                        &summary.chain_retried_owner_by_reason,
                    ),
                )
                .push(
                    "Chain dead reasons",
                    diagnosis::runtime_doctor_count_breakdown(
                        &summary.chain_dead_upstream_confirmed_by_reason,
                    ),
                )
                .push(
                    "Stale reasons",
                    diagnosis::runtime_doctor_count_breakdown(
                        &summary.stale_continuation_by_reason,
                    ),
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
        if *marker == "local_writer_error" {
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
        if *marker == "runtime_proxy_startup_audit" {
            fields.push("Startup pressure", summary.startup_audit_pressure.clone());
        }
        if *marker == "compat_request_surface" {
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
            "Last marker",
            summary
                .last_marker_line
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        )
        .push("Diagnosis", summary.diagnosis.clone());
    fields.build()
}
