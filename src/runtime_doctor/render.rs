use std::collections::BTreeMap;
use std::path::Path;

use super::*;

#[derive(serde::Serialize)]
struct RuntimeDoctorBrokerArtifactJsonView {
    #[serde(flatten)]
    fields: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Serialize)]
struct RuntimeDoctorJsonView {
    log_path: Option<String>,
    pointer_exists: bool,
    log_exists: bool,
    line_count: usize,
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,
    compat_warning_count: usize,
    top_client_family: Option<String>,
    top_client: Option<String>,
    top_tool_surface: Option<String>,
    top_compat_warning: Option<String>,
    marker_counts: BTreeMap<String, usize>,
    marker_last_fields: BTreeMap<String, BTreeMap<String, String>>,
    facet_counts: BTreeMap<String, BTreeMap<String, usize>>,
    previous_response_not_found_by_route: BTreeMap<String, usize>,
    previous_response_not_found_by_transport: BTreeMap<String, usize>,
    chain_retried_owner_by_reason: BTreeMap<String, usize>,
    chain_dead_upstream_confirmed_by_reason: BTreeMap<String, usize>,
    stale_continuation_by_reason: BTreeMap<String, usize>,
    latest_chain_event: Option<String>,
    latest_stale_continuation_reason: Option<String>,
    last_marker_line: Option<String>,
    selection_pressure: String,
    transport_pressure: String,
    persistence_pressure: String,
    quota_freshness_pressure: String,
    startup_audit_pressure: String,
    persisted_retry_backoffs: usize,
    persisted_transport_backoffs: usize,
    persisted_route_circuits: usize,
    persisted_usage_snapshots: usize,
    persisted_response_bindings: usize,
    persisted_session_bindings: usize,
    persisted_turn_state_bindings: usize,
    persisted_session_id_bindings: usize,
    persisted_verified_continuations: usize,
    persisted_warm_continuations: usize,
    persisted_suspect_continuations: usize,
    persisted_dead_continuations: usize,
    persisted_continuation_journal_response_bindings: usize,
    persisted_continuation_journal_session_bindings: usize,
    persisted_continuation_journal_turn_state_bindings: usize,
    persisted_continuation_journal_session_id_bindings: usize,
    persisted_turn_state_coverage_percent: Option<u8>,
    state_save_queue_backlog: Option<usize>,
    state_save_lag_ms: Option<u64>,
    continuation_journal_save_backlog: Option<usize>,
    continuation_journal_save_lag_ms: Option<u64>,
    profile_probe_refresh_backlog: Option<usize>,
    profile_probe_refresh_lag_ms: Option<u64>,
    continuation_journal_saved_at: Option<i64>,
    suspect_continuation_bindings: Vec<String>,
    stale_persisted_usage_snapshots: usize,
    recovered_state_file: bool,
    recovered_continuations_file: bool,
    recovered_continuation_journal_file: bool,
    recovered_scores_file: bool,
    recovered_usage_snapshots_file: bool,
    recovered_backoffs_file: bool,
    last_good_backups_present: usize,
    degraded_routes: Vec<String>,
    orphan_managed_dirs: Vec<String>,
    prodex_binary_identities: Vec<String>,
    runtime_broker_identities: Vec<String>,
    runtime_broker_artifacts: Vec<RuntimeDoctorBrokerArtifactJsonView>,
    prodex_binary_mismatch: bool,
    runtime_broker_mismatch: bool,
    failure_class_counts: BTreeMap<String, usize>,
    profiles: Vec<RuntimeDoctorProfileSummary>,
    diagnosis: String,
}

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

fn runtime_doctor_broker_artifact_json_view(line: &str) -> RuntimeDoctorBrokerArtifactJsonView {
    let artifact = runtime_doctor_parse_broker_artifact(line);
    let mut fields = BTreeMap::new();
    for (key, value) in artifact {
        let json_value = if key == "stale_leases" {
            value
                .parse::<usize>()
                .map(serde_json::Value::from)
                .unwrap_or_else(|_| serde_json::Value::String(value))
        } else {
            serde_json::Value::String(value)
        };
        fields.insert(key, json_value);
    }
    RuntimeDoctorBrokerArtifactJsonView { fields }
}

impl From<&RuntimeDoctorSummary> for RuntimeDoctorJsonView {
    fn from(summary: &RuntimeDoctorSummary) -> Self {
        Self {
            log_path: summary
                .log_path
                .as_ref()
                .map(|path| path.display().to_string()),
            pointer_exists: summary.pointer_exists,
            log_exists: summary.log_exists,
            line_count: summary.line_count,
            first_timestamp: summary.first_timestamp.clone(),
            last_timestamp: summary.last_timestamp.clone(),
            compat_warning_count: summary.compat_warning_count,
            top_client_family: summary.top_client_family.clone(),
            top_client: summary.top_client.clone(),
            top_tool_surface: summary.top_tool_surface.clone(),
            top_compat_warning: summary.top_compat_warning.clone(),
            marker_counts: summary
                .marker_counts
                .iter()
                .map(|(marker, count)| ((*marker).to_string(), *count))
                .collect(),
            marker_last_fields: summary
                .marker_last_fields
                .iter()
                .map(|(marker, fields)| ((*marker).to_string(), fields.clone()))
                .collect(),
            facet_counts: summary.facet_counts.clone(),
            previous_response_not_found_by_route: summary
                .previous_response_not_found_by_route
                .clone(),
            previous_response_not_found_by_transport: summary
                .previous_response_not_found_by_transport
                .clone(),
            chain_retried_owner_by_reason: summary.chain_retried_owner_by_reason.clone(),
            chain_dead_upstream_confirmed_by_reason: summary
                .chain_dead_upstream_confirmed_by_reason
                .clone(),
            stale_continuation_by_reason: summary.stale_continuation_by_reason.clone(),
            latest_chain_event: summary.latest_chain_event.clone(),
            latest_stale_continuation_reason: summary.latest_stale_continuation_reason.clone(),
            last_marker_line: summary.last_marker_line.clone(),
            selection_pressure: summary.selection_pressure.clone(),
            transport_pressure: summary.transport_pressure.clone(),
            persistence_pressure: summary.persistence_pressure.clone(),
            quota_freshness_pressure: summary.quota_freshness_pressure.clone(),
            startup_audit_pressure: summary.startup_audit_pressure.clone(),
            persisted_retry_backoffs: summary.persisted_retry_backoffs,
            persisted_transport_backoffs: summary.persisted_transport_backoffs,
            persisted_route_circuits: summary.persisted_route_circuits,
            persisted_usage_snapshots: summary.persisted_usage_snapshots,
            persisted_response_bindings: summary.persisted_response_bindings,
            persisted_session_bindings: summary.persisted_session_bindings,
            persisted_turn_state_bindings: summary.persisted_turn_state_bindings,
            persisted_session_id_bindings: summary.persisted_session_id_bindings,
            persisted_verified_continuations: summary.persisted_verified_continuations,
            persisted_warm_continuations: summary.persisted_warm_continuations,
            persisted_suspect_continuations: summary.persisted_suspect_continuations,
            persisted_dead_continuations: summary.persisted_dead_continuations,
            persisted_continuation_journal_response_bindings: summary
                .persisted_continuation_journal_response_bindings,
            persisted_continuation_journal_session_bindings: summary
                .persisted_continuation_journal_session_bindings,
            persisted_continuation_journal_turn_state_bindings: summary
                .persisted_continuation_journal_turn_state_bindings,
            persisted_continuation_journal_session_id_bindings: summary
                .persisted_continuation_journal_session_id_bindings,
            persisted_turn_state_coverage_percent: summary.persisted_turn_state_coverage_percent,
            state_save_queue_backlog: summary.state_save_queue_backlog,
            state_save_lag_ms: summary.state_save_lag_ms,
            continuation_journal_save_backlog: summary.continuation_journal_save_backlog,
            continuation_journal_save_lag_ms: summary.continuation_journal_save_lag_ms,
            profile_probe_refresh_backlog: summary.profile_probe_refresh_backlog,
            profile_probe_refresh_lag_ms: summary.profile_probe_refresh_lag_ms,
            continuation_journal_saved_at: summary.continuation_journal_saved_at,
            suspect_continuation_bindings: summary.suspect_continuation_bindings.clone(),
            stale_persisted_usage_snapshots: summary.stale_persisted_usage_snapshots,
            recovered_state_file: summary.recovered_state_file,
            recovered_continuations_file: summary.recovered_continuations_file,
            recovered_continuation_journal_file: summary.recovered_continuation_journal_file,
            recovered_scores_file: summary.recovered_scores_file,
            recovered_usage_snapshots_file: summary.recovered_usage_snapshots_file,
            recovered_backoffs_file: summary.recovered_backoffs_file,
            last_good_backups_present: summary.last_good_backups_present,
            degraded_routes: summary.degraded_routes.clone(),
            orphan_managed_dirs: summary.orphan_managed_dirs.clone(),
            prodex_binary_identities: summary.prodex_binary_identities.clone(),
            runtime_broker_identities: summary.runtime_broker_identities.clone(),
            runtime_broker_artifacts: summary
                .runtime_broker_identities
                .iter()
                .map(|line| runtime_doctor_broker_artifact_json_view(line))
                .collect(),
            prodex_binary_mismatch: summary.prodex_binary_mismatch,
            runtime_broker_mismatch: summary.runtime_broker_mismatch,
            failure_class_counts: summary.failure_class_counts.clone(),
            profiles: summary.profiles.clone(),
            diagnosis: summary.diagnosis.clone(),
        }
    }
}

pub(crate) fn runtime_doctor_json_value(summary: &RuntimeDoctorSummary) -> serde_json::Value {
    serde_json::to_value(RuntimeDoctorJsonView::from(summary))
        .expect("runtime doctor serialization should always succeed")
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
    if marker == "runtime_proxy_overload_backoff" {
        fields.push(
            "Connect failures",
            (diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_timeout")
                + diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_error"))
            .to_string(),
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
    if marker == "previous_response_fresh_fallback"
        || marker == "previous_response_fresh_fallback_blocked"
    {
        fields
            .push(
                "Replay shape",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("request_shape"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .push(
                "Replay reason",
                summary
                    .marker_last_fields
                    .get(marker)
                    .and_then(|fields| fields.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
            );
        if marker == "previous_response_fresh_fallback_blocked" {
            fields
                .push(
                    "Replay blocked shapes",
                    diagnosis::runtime_doctor_count_breakdown(
                        &summary.previous_response_fresh_fallback_blocked_by_request_shape,
                    ),
                )
                .push(
                    "Replay next step",
                    diagnosis::runtime_doctor_context_fallback_blocked_next_step(summary),
                );
        }
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
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn json_object_keys(value: &serde_json::Value) -> BTreeSet<String> {
        value
            .as_object()
            .expect("runtime doctor JSON should be an object")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn runtime_doctor_json_value_keeps_stable_top_level_shape() {
        let mut summary = RuntimeDoctorSummary {
            log_path: Some(PathBuf::from("/tmp/prodex-runtime.log")),
            pointer_exists: true,
            log_exists: true,
            line_count: 7,
            diagnosis: "Runtime broker registry broker-a points to dead pid 123 at 127.0.0.1:1234; run `prodex cleanup` or restart `prodex run` so a fresh broker registry is written."
                .to_string(),
            runtime_broker_identities: vec![
                "broker_key=broker-a pid=123 listen_addr=127.0.0.1:1234 status=dead_pid mismatch=none version=0.1.0 path=/opt/prodex sha256=abc123 source=registry stale_leases=2"
                    .to_string(),
            ],
            ..RuntimeDoctorSummary::default()
        };
        summary
            .marker_counts
            .insert("runtime_proxy_queue_overloaded", 2);
        summary.marker_counts.insert("profile_circuit_open", 1);
        summary.marker_last_fields.insert(
            "runtime_proxy_queue_overloaded",
            BTreeMap::from([
                ("lane".to_string(), "responses".to_string()),
                ("request".to_string(), "9".to_string()),
            ]),
        );
        summary.failure_class_counts =
            BTreeMap::from([("admission".to_string(), 2), ("transport".to_string(), 1)]);

        let value = runtime_doctor_json_value(&summary);
        let keys = json_object_keys(&value);
        let expected_keys = [
            "log_path",
            "pointer_exists",
            "log_exists",
            "line_count",
            "first_timestamp",
            "last_timestamp",
            "compat_warning_count",
            "top_client_family",
            "top_client",
            "top_tool_surface",
            "top_compat_warning",
            "marker_counts",
            "marker_last_fields",
            "facet_counts",
            "previous_response_not_found_by_route",
            "previous_response_not_found_by_transport",
            "chain_retried_owner_by_reason",
            "chain_dead_upstream_confirmed_by_reason",
            "stale_continuation_by_reason",
            "latest_chain_event",
            "latest_stale_continuation_reason",
            "last_marker_line",
            "selection_pressure",
            "transport_pressure",
            "persistence_pressure",
            "quota_freshness_pressure",
            "startup_audit_pressure",
            "persisted_retry_backoffs",
            "persisted_transport_backoffs",
            "persisted_route_circuits",
            "persisted_usage_snapshots",
            "persisted_response_bindings",
            "persisted_session_bindings",
            "persisted_turn_state_bindings",
            "persisted_session_id_bindings",
            "persisted_verified_continuations",
            "persisted_warm_continuations",
            "persisted_suspect_continuations",
            "persisted_dead_continuations",
            "persisted_continuation_journal_response_bindings",
            "persisted_continuation_journal_session_bindings",
            "persisted_continuation_journal_turn_state_bindings",
            "persisted_continuation_journal_session_id_bindings",
            "persisted_turn_state_coverage_percent",
            "state_save_queue_backlog",
            "state_save_lag_ms",
            "continuation_journal_save_backlog",
            "continuation_journal_save_lag_ms",
            "profile_probe_refresh_backlog",
            "profile_probe_refresh_lag_ms",
            "continuation_journal_saved_at",
            "suspect_continuation_bindings",
            "stale_persisted_usage_snapshots",
            "recovered_state_file",
            "recovered_continuations_file",
            "recovered_continuation_journal_file",
            "recovered_scores_file",
            "recovered_usage_snapshots_file",
            "recovered_backoffs_file",
            "last_good_backups_present",
            "degraded_routes",
            "orphan_managed_dirs",
            "prodex_binary_identities",
            "runtime_broker_identities",
            "runtime_broker_artifacts",
            "prodex_binary_mismatch",
            "runtime_broker_mismatch",
            "failure_class_counts",
            "profiles",
            "diagnosis",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
        assert_eq!(keys, expected_keys);
        assert_eq!(value["log_path"], "/tmp/prodex-runtime.log");
        assert_eq!(value["pointer_exists"], true);
        assert_eq!(value["log_exists"], true);
        assert_eq!(value["line_count"], 7);
        assert_eq!(value["marker_counts"]["runtime_proxy_queue_overloaded"], 2);
        assert_eq!(value["marker_counts"]["profile_circuit_open"], 1);
        assert_eq!(
            value["marker_last_fields"]["runtime_proxy_queue_overloaded"]["lane"],
            "responses"
        );
        assert_eq!(
            value["runtime_broker_artifacts"][0]["broker_key"],
            "broker-a"
        );
        assert_eq!(value["runtime_broker_artifacts"][0]["status"], "dead_pid");
        assert_eq!(value["runtime_broker_artifacts"][0]["stale_leases"], 2);
        assert!(
            value["diagnosis"]
                .as_str()
                .expect("diagnosis should be a string")
                .contains("dead pid 123")
        );
    }

    #[test]
    fn runtime_doctor_json_value_keeps_profile_and_route_shape() {
        let summary = RuntimeDoctorSummary {
            profiles: vec![RuntimeDoctorProfileSummary {
                profile: "alpha".to_string(),
                quota_freshness: "fresh".to_string(),
                quota_age_seconds: 5,
                retry_backoff_until: Some(11),
                transport_backoff_until: Some(13),
                routes: vec![RuntimeDoctorRouteSummary {
                    route: "responses".to_string(),
                    circuit_state: "closed".to_string(),
                    circuit_until: Some(17),
                    transport_backoff_until: Some(19),
                    health_score: 21,
                    bad_pairing_score: 23,
                    performance_score: 25,
                    quota_band: "healthy".to_string(),
                    five_hour_status: "ok".to_string(),
                    weekly_status: "ok".to_string(),
                }],
            }],
            ..RuntimeDoctorSummary::default()
        };

        let value = runtime_doctor_json_value(&summary);
        let profile = &value["profiles"][0];
        let route = &profile["routes"][0];

        assert_eq!(
            json_object_keys(profile),
            BTreeSet::from([
                "profile".to_string(),
                "quota_freshness".to_string(),
                "quota_age_seconds".to_string(),
                "retry_backoff_until".to_string(),
                "transport_backoff_until".to_string(),
                "routes".to_string(),
            ])
        );
        assert_eq!(
            json_object_keys(route),
            BTreeSet::from([
                "route".to_string(),
                "circuit_state".to_string(),
                "circuit_until".to_string(),
                "transport_backoff_until".to_string(),
                "health_score".to_string(),
                "bad_pairing_score".to_string(),
                "performance_score".to_string(),
                "quota_band".to_string(),
                "five_hour_status".to_string(),
                "weekly_status".to_string(),
            ])
        );
        assert_eq!(profile["profile"], "alpha");
        assert_eq!(route["route"], "responses");
        assert_eq!(route["health_score"], 21);
    }
}
