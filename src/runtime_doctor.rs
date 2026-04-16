use anyhow::{Context, Result};
use chrono::Local;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::*;

const RUNTIME_DOCTOR_FACETS: &[&str] = &[
    "lane",
    "route",
    "profile",
    "reason",
    "transport",
    "family",
    "client",
    "tool_surface",
    "continuation",
    "origin",
    "warning",
    "quota_source",
    "quota_band",
    "five_hour_status",
    "weekly_status",
    "affinity",
    "context",
    "event",
    "stage",
    "state",
    "source",
];

const RUNTIME_DOCTOR_MARKERS: &[&str] = &[
    "runtime_proxy_queue_overloaded",
    "runtime_proxy_active_limit_reached",
    "runtime_proxy_lane_limit_reached",
    "runtime_proxy_overload_backoff",
    "runtime_proxy_admission_wait_started",
    "runtime_proxy_admission_recovered",
    "runtime_proxy_queue_wait_started",
    "runtime_proxy_queue_recovered",
    "profile_inflight_saturated",
    "upstream_connect_timeout",
    "upstream_connect_dns_error",
    "upstream_tls_handshake_error",
    "upstream_connect_error",
    "precommit_budget_exhausted",
    "profile_retry_backoff",
    "profile_transport_backoff",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
    "profile_health",
    "profile_latency",
    "profile_bad_pairing",
    "previous_response_not_found",
    "previous_response_negative_cache",
    "compact_committed_owner",
    "compact_followup_owner",
    "compact_fresh_fallback_blocked",
    "compact_pressure_shed",
    "compact_lineage_released",
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "responses_pre_send_skip",
    "websocket_pre_send_skip",
    "quota_release_profile_affinity",
    "quota_critical_floor_before_send",
    "upstream_usage_limit_passthrough",
    "compat_request_surface",
    "compat_warning",
    "websocket_reuse_skip_quota_exhausted",
    "websocket_reuse_watchdog",
    "websocket_precommit_frame_timeout",
    "stream_read_error",
    "local_writer_error",
    "first_upstream_chunk",
    "first_local_chunk",
    "state_save_ok",
    "state_save_skipped",
    "state_save_error",
    "state_save_queued",
    "continuation_journal_save_ok",
    "continuation_journal_save_error",
    "continuation_journal_save_queued",
    "runtime_proxy_restore_counts",
    "runtime_proxy_startup_audit",
    "profile_probe_refresh_queued",
    "profile_probe_refresh_start",
    "profile_probe_refresh_ok",
    "profile_probe_refresh_error",
    "quota_blocked_affinity_released",
];

const RUNTIME_DOCTOR_COUNT_FIELD_ROWS: &[(&str, &str)] = &[
    ("Queue overload", "runtime_proxy_queue_overloaded"),
    ("Active limit", "runtime_proxy_active_limit_reached"),
    ("Lane limit", "runtime_proxy_lane_limit_reached"),
    ("Overload backoff", "runtime_proxy_overload_backoff"),
    ("Pre-commit budget", "precommit_budget_exhausted"),
    ("Responses pre-send skips", "responses_pre_send_skip"),
    ("Websocket pre-send skips", "websocket_pre_send_skip"),
    (
        "Quota critical floor pre-send",
        "quota_critical_floor_before_send",
    ),
    (
        "Upstream usage-limit passthrough",
        "upstream_usage_limit_passthrough",
    ),
    ("Retry backoff", "profile_retry_backoff"),
    ("Transport backoff", "profile_transport_backoff"),
    ("Route circuits", "profile_circuit_open"),
    ("Health penalties", "profile_health"),
    ("Latency penalties", "profile_latency"),
    ("Bad pairing", "profile_bad_pairing"),
    ("Prev not found", "previous_response_not_found"),
    ("Prev negative cache", "previous_response_negative_cache"),
    ("Compact guard", "compact_fresh_fallback_blocked"),
    ("Compact shed", "compact_pressure_shed"),
    ("Selection picks", "selection_pick"),
    ("Selection skips", "selection_skip_current"),
    ("WS reuse watchdog", "websocket_reuse_watchdog"),
    (
        "WS first-frame timeouts",
        "websocket_precommit_frame_timeout",
    ),
    ("Stream read errors", "stream_read_error"),
    ("Writer errors", "local_writer_error"),
    ("State save errors", "state_save_error"),
    ("Cont journal err", "continuation_journal_save_error"),
    ("State save ok", "state_save_ok"),
    ("Cont journal ok", "continuation_journal_save_ok"),
    ("State save skipped", "state_save_skipped"),
    ("Startup audit", "runtime_proxy_startup_audit"),
    ("Admission recovered", "runtime_proxy_admission_recovered"),
    ("Queue recovered", "runtime_proxy_queue_recovered"),
    ("Probe refresh", "profile_probe_refresh_start"),
    ("Probe refresh errors", "profile_probe_refresh_error"),
    ("Compat samples", "compat_request_surface"),
];

const RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS: &[&str] = &[
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "precommit_budget_exhausted",
];

const RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS: &[&str] = &[
    "stream_read_error",
    "upstream_connect_timeout",
    "upstream_connect_dns_error",
    "upstream_tls_handshake_error",
    "upstream_connect_error",
    "profile_transport_backoff",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
    "websocket_precommit_frame_timeout",
    "local_writer_error",
];

const RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS: &[&str] =
    &["state_save_error", "continuation_journal_save_error"];

const RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS: &[&str] = &["state_save_skipped"];

const RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS: &[&str] =
    &["profile_probe_refresh_start", "profile_probe_refresh_ok"];

struct RuntimeDoctorJsonBuilder {
    value: serde_json::Map<String, serde_json::Value>,
}

impl RuntimeDoctorJsonBuilder {
    fn new() -> Self {
        Self {
            value: serde_json::Map::new(),
        }
    }

    fn insert<T: serde::Serialize>(&mut self, key: &str, value: T) -> &mut Self {
        let value = serde_json::to_value(value)
            .expect("runtime doctor serialization should always succeed");
        self.value.insert(key.to_string(), value);
        self
    }

    fn build(self) -> serde_json::Value {
        serde_json::Value::Object(self.value)
    }
}

struct RuntimeDoctorFieldBuilder {
    rows: Vec<(String, String)>,
}

impl RuntimeDoctorFieldBuilder {
    fn new() -> Self {
        Self { rows: Vec::new() }
    }

    fn push(&mut self, label: &str, value: impl Into<String>) -> &mut Self {
        self.rows.push((label.to_string(), value.into()));
        self
    }

    fn push_marker_count(
        &mut self,
        label: &str,
        summary: &RuntimeDoctorSummary,
        marker: &'static str,
    ) -> &mut Self {
        self.push(
            label,
            runtime_doctor_marker_count(summary, marker).to_string(),
        )
    }

    fn build(self) -> Vec<(String, String)> {
        self.rows
    }
}

struct RuntimeDoctorDegradedRouteBuilder {
    routes: Vec<String>,
}

impl RuntimeDoctorDegradedRouteBuilder {
    fn new() -> Self {
        Self { routes: Vec::new() }
    }

    fn push_route_circuits(&mut self, backoffs: &RuntimeProfileBackoffs, now: i64) -> &mut Self {
        for (key, until) in &backoffs.route_circuit_open_until {
            if let Some((route, profile_name)) =
                runtime_profile_route_key_parts(key, "__route_circuit__:")
            {
                let state = if *until > now { "open" } else { "half-open" };
                self.routes.push(format!(
                    "{profile_name}/{route} circuit={state} until={until}"
                ));
            }
        }
        self
    }

    fn push_transport_backoffs(&mut self, backoffs: &RuntimeProfileBackoffs) -> &mut Self {
        for (profile_name, until) in &backoffs.transport_backoff_until {
            if let Some((route, profile_name)) =
                runtime_profile_transport_backoff_key_parts(profile_name)
            {
                self.routes.push(format!(
                    "{profile_name}/{route} transport_backoff until={until}"
                ));
            } else {
                self.routes.push(format!(
                    "{profile_name}/transport transport_backoff until={until}"
                ));
            }
        }
        self
    }

    fn push_retry_backoffs(&mut self, backoffs: &RuntimeProfileBackoffs) -> &mut Self {
        for (profile_name, until) in &backoffs.retry_backoff_until {
            self.routes
                .push(format!("{profile_name}/retry retry_backoff until={until}"));
        }
        self
    }

    fn push_scores(
        &mut self,
        scores: &BTreeMap<String, RuntimeProfileHealth>,
        now: i64,
    ) -> &mut Self {
        for (key, health) in scores {
            if let Some((route, profile_name)) =
                runtime_profile_route_key_parts(key, "__route_bad_pairing__:")
            {
                let score = runtime_profile_effective_score(
                    health,
                    now,
                    RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
                );
                if score > 0 {
                    self.routes
                        .push(format!("{profile_name}/{route} bad_pairing={score}"));
                }
                continue;
            }
            if let Some((route, profile_name)) =
                runtime_profile_route_key_parts(key, "__route_health__:")
            {
                let score = runtime_profile_effective_health_score(health, now);
                if score > 0 {
                    self.routes
                        .push(format!("{profile_name}/{route} health={score}"));
                }
            }
        }
        self
    }

    fn build(mut self) -> Vec<String> {
        self.routes.sort();
        self.routes.dedup();
        self.routes.into_iter().take(8).collect()
    }
}

struct RuntimeDoctorCollector {
    paths: Option<AppPaths>,
    pointer_path: PathBuf,
    pointed_log_path: Option<PathBuf>,
    newest_log_path: Option<PathBuf>,
}

impl RuntimeDoctorCollector {
    fn discover() -> Self {
        let pointer_path = runtime_proxy_latest_log_pointer_path();
        let pointed_log_path = fs::read_to_string(&pointer_path)
            .ok()
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            paths: AppPaths::discover().ok(),
            pointer_path,
            pointed_log_path,
            newest_log_path: newest_runtime_proxy_log_in_dir(&runtime_proxy_log_dir()),
        }
    }

    fn pointer_exists(&self) -> bool {
        self.pointer_path.exists()
    }

    fn pointer_target_exists(&self) -> bool {
        self.pointed_log_path
            .as_ref()
            .is_some_and(|path| path.exists())
    }

    fn pointer_note(&self) -> Option<&'static str> {
        match (
            self.pointed_log_path.as_ref(),
            self.newest_log_path.as_ref(),
        ) {
            (Some(pointed), Some(newest)) if pointed.exists() && newest != pointed => {
                Some("Runtime log pointer was stale; sampled a newer log instead.")
            }
            (Some(_), Some(_)) if !self.pointer_target_exists() => {
                Some("Runtime log pointer target was missing; sampled a newer log instead.")
            }
            (Some(_), None) if !self.pointer_target_exists() => {
                Some("Runtime log pointer target was missing.")
            }
            _ => None,
        }
    }

    fn log_path(&self) -> Option<PathBuf> {
        if self.pointer_target_exists() {
            self.newest_log_path
                .as_ref()
                .filter(|path| {
                    self.pointed_log_path
                        .as_ref()
                        .is_some_and(|pointed| *path != pointed)
                })
                .cloned()
                .or_else(|| self.pointed_log_path.clone())
        } else {
            self.newest_log_path.clone()
        }
    }

    fn collect(self) -> RuntimeDoctorSummary {
        let log_path = self.log_path();
        let mut summary = runtime_doctor_summary_from_log(log_path.as_deref());
        summary.pointer_exists = self.pointer_exists();
        summary.log_exists = log_path.as_ref().is_some_and(|path| path.exists());
        summary.log_path = log_path;
        if let Some(paths) = self.paths.as_ref() {
            collect_runtime_doctor_state(paths, &mut summary);
        }
        runtime_doctor_finalize_summary(&mut summary);
        runtime_doctor_append_pointer_note(&mut summary, self.pointer_note());
        summary
    }
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
    let mut builder = RuntimeDoctorJsonBuilder::new();
    builder
        .insert(
            "log_path",
            summary
                .log_path
                .as_ref()
                .map(|path| path.display().to_string()),
        )
        .insert("pointer_exists", summary.pointer_exists)
        .insert("log_exists", summary.log_exists)
        .insert("line_count", summary.line_count)
        .insert("first_timestamp", summary.first_timestamp.clone())
        .insert("last_timestamp", summary.last_timestamp.clone())
        .insert("compat_warning_count", summary.compat_warning_count)
        .insert("top_client_family", summary.top_client_family.clone())
        .insert("top_client", summary.top_client.clone())
        .insert("top_tool_surface", summary.top_tool_surface.clone())
        .insert("top_compat_warning", summary.top_compat_warning.clone())
        .insert(
            "marker_counts",
            runtime_doctor_json_map(
                summary
                    .marker_counts
                    .iter()
                    .map(|(marker, count)| ((*marker).to_string(), *count)),
            ),
        )
        .insert(
            "marker_last_fields",
            runtime_doctor_json_map(summary.marker_last_fields.iter().map(|(marker, fields)| {
                (
                    (*marker).to_string(),
                    runtime_doctor_json_map(fields.clone()),
                )
            })),
        )
        .insert(
            "facet_counts",
            runtime_doctor_json_map(
                summary.facet_counts.iter().map(|(facet, counts)| {
                    (facet.clone(), runtime_doctor_json_map(counts.clone()))
                }),
            ),
        )
        .insert(
            "previous_response_not_found_by_route",
            runtime_doctor_json_map(summary.previous_response_not_found_by_route.clone()),
        )
        .insert(
            "previous_response_not_found_by_transport",
            runtime_doctor_json_map(summary.previous_response_not_found_by_transport.clone()),
        )
        .insert("last_marker_line", summary.last_marker_line.clone())
        .insert("selection_pressure", summary.selection_pressure.clone())
        .insert("transport_pressure", summary.transport_pressure.clone())
        .insert("persistence_pressure", summary.persistence_pressure.clone())
        .insert(
            "quota_freshness_pressure",
            summary.quota_freshness_pressure.clone(),
        )
        .insert(
            "startup_audit_pressure",
            summary.startup_audit_pressure.clone(),
        )
        .insert("persisted_retry_backoffs", summary.persisted_retry_backoffs)
        .insert(
            "persisted_transport_backoffs",
            summary.persisted_transport_backoffs,
        )
        .insert("persisted_route_circuits", summary.persisted_route_circuits)
        .insert(
            "persisted_usage_snapshots",
            summary.persisted_usage_snapshots,
        )
        .insert(
            "persisted_response_bindings",
            summary.persisted_response_bindings,
        )
        .insert(
            "persisted_session_bindings",
            summary.persisted_session_bindings,
        )
        .insert(
            "persisted_turn_state_bindings",
            summary.persisted_turn_state_bindings,
        )
        .insert(
            "persisted_session_id_bindings",
            summary.persisted_session_id_bindings,
        )
        .insert(
            "persisted_verified_continuations",
            summary.persisted_verified_continuations,
        )
        .insert(
            "persisted_warm_continuations",
            summary.persisted_warm_continuations,
        )
        .insert(
            "persisted_suspect_continuations",
            summary.persisted_suspect_continuations,
        )
        .insert(
            "persisted_dead_continuations",
            summary.persisted_dead_continuations,
        )
        .insert(
            "persisted_continuation_journal_response_bindings",
            summary.persisted_continuation_journal_response_bindings,
        )
        .insert(
            "persisted_continuation_journal_session_bindings",
            summary.persisted_continuation_journal_session_bindings,
        )
        .insert(
            "persisted_continuation_journal_turn_state_bindings",
            summary.persisted_continuation_journal_turn_state_bindings,
        )
        .insert(
            "persisted_continuation_journal_session_id_bindings",
            summary.persisted_continuation_journal_session_id_bindings,
        )
        .insert("state_save_queue_backlog", summary.state_save_queue_backlog)
        .insert("state_save_lag_ms", summary.state_save_lag_ms)
        .insert(
            "continuation_journal_save_backlog",
            summary.continuation_journal_save_backlog,
        )
        .insert(
            "continuation_journal_save_lag_ms",
            summary.continuation_journal_save_lag_ms,
        )
        .insert(
            "profile_probe_refresh_backlog",
            summary.profile_probe_refresh_backlog,
        )
        .insert(
            "profile_probe_refresh_lag_ms",
            summary.profile_probe_refresh_lag_ms,
        )
        .insert(
            "continuation_journal_saved_at",
            summary.continuation_journal_saved_at,
        )
        .insert(
            "suspect_continuation_bindings",
            summary.suspect_continuation_bindings.clone(),
        )
        .insert(
            "stale_persisted_usage_snapshots",
            summary.stale_persisted_usage_snapshots,
        )
        .insert("recovered_state_file", summary.recovered_state_file)
        .insert(
            "recovered_continuations_file",
            summary.recovered_continuations_file,
        )
        .insert(
            "recovered_continuation_journal_file",
            summary.recovered_continuation_journal_file,
        )
        .insert("recovered_scores_file", summary.recovered_scores_file)
        .insert(
            "recovered_usage_snapshots_file",
            summary.recovered_usage_snapshots_file,
        )
        .insert("recovered_backoffs_file", summary.recovered_backoffs_file)
        .insert(
            "last_good_backups_present",
            summary.last_good_backups_present,
        )
        .insert("degraded_routes", summary.degraded_routes.clone())
        .insert("orphan_managed_dirs", summary.orphan_managed_dirs.clone())
        .insert(
            "failure_class_counts",
            runtime_doctor_json_map(summary.failure_class_counts.clone()),
        )
        .insert(
            "profiles",
            summary
                .profiles
                .iter()
                .map(runtime_doctor_profile_json)
                .collect::<Vec<_>>(),
        )
        .insert("diagnosis", summary.diagnosis.clone());
    builder.build()
}

pub(crate) fn runtime_doctor_fields() -> Vec<(String, String)> {
    let pointer_path = runtime_proxy_latest_log_pointer_path();
    let summary = collect_runtime_doctor_summary();
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
    let mut fields = RuntimeDoctorFieldBuilder::new();
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
        fields.push_marker_count(label, summary, marker);
        if *marker == "runtime_proxy_overload_backoff" {
            fields.push(
                "Connect failures",
                (runtime_doctor_marker_count(summary, "upstream_connect_timeout")
                    + runtime_doctor_marker_count(summary, "upstream_connect_error"))
                .to_string(),
            );
        }
        if *marker == "previous_response_not_found" {
            fields
                .push(
                    "Prev not found routes",
                    runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_route),
                )
                .push(
                    "Prev not found xport",
                    runtime_doctor_count_breakdown(
                        &summary.previous_response_not_found_by_transport,
                    ),
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
                    runtime_doctor_top_facet(summary, "lane").unwrap_or_else(|| "-".to_string()),
                )
                .push(
                    "Hot route",
                    runtime_doctor_top_facet(summary, "route").unwrap_or_else(|| "-".to_string()),
                )
                .push(
                    "Hot profile",
                    runtime_doctor_top_facet(summary, "profile").unwrap_or_else(|| "-".to_string()),
                )
                .push(
                    "Hot reason",
                    runtime_doctor_top_facet(summary, "reason").unwrap_or_else(|| "-".to_string()),
                )
                .push(
                    "Quota source",
                    runtime_doctor_top_facet(summary, "quota_source")
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
            runtime_doctor_count_breakdown(&summary.failure_class_counts),
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
                "responses={} sessions={} turns={} session_ids={}",
                summary.persisted_response_bindings,
                summary.persisted_session_bindings,
                summary.persisted_turn_state_bindings,
                summary.persisted_session_id_bindings
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

pub(crate) fn runtime_doctor_marker_count(
    summary: &RuntimeDoctorSummary,
    marker: &'static str,
) -> usize {
    summary.marker_counts.get(marker).copied().unwrap_or(0)
}

fn runtime_doctor_has_any_markers(
    summary: &RuntimeDoctorSummary,
    markers: &[&'static str],
) -> bool {
    markers
        .iter()
        .any(|marker| runtime_doctor_marker_count(summary, marker) > 0)
}

fn runtime_doctor_facet_count(summary: &RuntimeDoctorSummary, facet: &str, value: &str) -> usize {
    summary
        .facet_counts
        .get(facet)
        .and_then(|counts| counts.get(value))
        .copied()
        .unwrap_or(0)
}

fn runtime_doctor_marker_last_field<'a>(
    summary: &'a RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<&'a str> {
    summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| fields.get(field))
        .map(String::as_str)
}

fn runtime_doctor_marker_last_usize_field(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<usize> {
    runtime_doctor_marker_last_field(summary, marker, field)?
        .parse()
        .ok()
}

fn runtime_doctor_marker_last_u64_field(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<u64> {
    runtime_doctor_marker_last_field(summary, marker, field)?
        .parse()
        .ok()
}

fn runtime_doctor_count_breakdown(counts: &BTreeMap<String, usize>) -> String {
    if counts.is_empty() {
        return "-".to_string();
    }
    counts
        .iter()
        .map(|(label, count)| format!("{label}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn runtime_doctor_failure_class_counts(summary: &RuntimeDoctorSummary) -> BTreeMap<String, usize> {
    let classes: [(&str, &[&str]); 5] = [
        (
            "admission",
            &[
                "runtime_proxy_queue_overloaded",
                "runtime_proxy_active_limit_reached",
                "runtime_proxy_lane_limit_reached",
                "runtime_proxy_overload_backoff",
                "runtime_proxy_admission_wait_started",
                "runtime_proxy_admission_wait_exhausted",
                "runtime_proxy_queue_wait_started",
                "runtime_proxy_queue_wait_exhausted",
                "profile_inflight_saturated",
            ],
        ),
        (
            "continuation",
            &[
                "previous_response_not_found",
                "previous_response_negative_cache",
                "compact_fresh_fallback_blocked",
                "compact_pressure_shed",
            ],
        ),
        (
            "persistence",
            &[
                "state_save_error",
                "state_save_skipped",
                "continuation_journal_save_error",
            ],
        ),
        (
            "quota",
            &[
                "profile_retry_backoff",
                "profile_transport_backoff",
                "profile_circuit_open",
                "profile_circuit_half_open_probe",
                "profile_health",
                "profile_latency",
                "profile_bad_pairing",
                "profile_probe_refresh_error",
                "responses_pre_send_skip",
                "websocket_pre_send_skip",
                "quota_critical_floor_before_send",
                "upstream_usage_limit_passthrough",
            ],
        ),
        (
            "transport",
            &[
                "upstream_connect_timeout",
                "upstream_connect_dns_error",
                "upstream_tls_handshake_error",
                "upstream_connect_error",
                "stream_read_error",
                "local_writer_error",
                "websocket_precommit_frame_timeout",
            ],
        ),
    ];

    classes
        .into_iter()
        .map(|(class, markers)| {
            (
                class.to_string(),
                markers
                    .iter()
                    .map(|marker| runtime_doctor_marker_count(summary, marker))
                    .sum(),
            )
        })
        .filter(|(_, count)| *count > 0)
        .collect()
}

fn runtime_doctor_finalize_log_summary(summary: &mut RuntimeDoctorSummary) {
    summary.state_save_queue_backlog =
        runtime_doctor_marker_last_usize_field(summary, "state_save_queued", "backlog");
    summary.state_save_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "state_save_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(summary, "state_save_skipped", "lag_ms")
            })
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(summary, "state_save_error", "lag_ms")
            });
    summary.continuation_journal_save_backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "continuation_journal_save_queued",
        "backlog",
    );
    summary.continuation_journal_save_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "continuation_journal_save_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(
                    summary,
                    "continuation_journal_save_error",
                    "lag_ms",
                )
            });
    summary.profile_probe_refresh_backlog =
        runtime_doctor_marker_last_usize_field(summary, "profile_probe_refresh_queued", "backlog");
    summary.profile_probe_refresh_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "profile_probe_refresh_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(
                    summary,
                    "profile_probe_refresh_error",
                    "lag_ms",
                )
            });
    // Count quota-floor pre-send skips via the reason facet so the doctor keeps
    // exposing the hardening signal even though the runtime logs it as a reason,
    // not as a standalone marker.
    let quota_floor_before_send_count =
        runtime_doctor_facet_count(summary, "reason", "quota_critical_floor_before_send");
    if quota_floor_before_send_count > 0 {
        *summary
            .marker_counts
            .entry("quota_critical_floor_before_send")
            .or_insert(0) += quota_floor_before_send_count;
    }
    summary.compat_warning_count = runtime_doctor_marker_count(summary, "compat_warning");
    summary.top_client_family = runtime_doctor_top_facet(summary, "family");
    summary.top_client = runtime_doctor_top_facet(summary, "client");
    summary.top_tool_surface = runtime_doctor_top_facet(summary, "tool_surface");
    summary.top_compat_warning = runtime_doctor_top_facet(summary, "warning");
    summary.failure_class_counts = runtime_doctor_failure_class_counts(summary);
}

pub(crate) fn runtime_doctor_top_facet(
    summary: &RuntimeDoctorSummary,
    facet: &str,
) -> Option<String> {
    summary.facet_counts.get(facet).and_then(|counts| {
        counts
            .iter()
            .max_by_key(|(value, count)| (**count, value.as_str()))
            .map(|(value, count)| format!("{value} ({count})"))
    })
}

fn runtime_doctor_quota_freshness_label(
    snapshot: Option<&RuntimeProfileUsageSnapshot>,
    now: i64,
) -> String {
    match snapshot {
        Some(snapshot) if runtime_usage_snapshot_is_usable(snapshot, now) => "fresh".to_string(),
        Some(_) => "stale".to_string(),
        None => "missing".to_string(),
    }
}

fn runtime_doctor_route_circuit_state(until: Option<i64>, now: i64) -> String {
    match until {
        Some(until) if until > now => "open".to_string(),
        Some(_) => "half_open".to_string(),
        None => "closed".to_string(),
    }
}

fn runtime_doctor_profile_summaries(
    state: &AppState,
    usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
    backoffs: &RuntimeProfileBackoffs,
    now: i64,
) -> Vec<RuntimeDoctorProfileSummary> {
    let mut profiles = Vec::new();
    for profile_name in state.profiles.keys() {
        let snapshot = usage_snapshots.get(profile_name);
        let quota_age_seconds = snapshot
            .map(|snapshot| now.saturating_sub(snapshot.checked_at))
            .unwrap_or(i64::MAX);
        let routes = [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Standard,
        ]
        .into_iter()
        .map(|route_kind| {
            let quota_summary = snapshot
                .map(|snapshot| runtime_quota_summary_from_usage_snapshot(snapshot, route_kind))
                .unwrap_or(RuntimeQuotaSummary {
                    five_hour: RuntimeQuotaWindowSummary {
                        status: RuntimeQuotaWindowStatus::Unknown,
                        remaining_percent: 0,
                        reset_at: i64::MAX,
                    },
                    weekly: RuntimeQuotaWindowSummary {
                        status: RuntimeQuotaWindowStatus::Unknown,
                        remaining_percent: 0,
                        reset_at: i64::MAX,
                    },
                    route_band: RuntimeQuotaPressureBand::Unknown,
                });
            RuntimeDoctorRouteSummary {
                route: runtime_route_kind_label(route_kind).to_string(),
                circuit_state: runtime_doctor_route_circuit_state(
                    backoffs
                        .route_circuit_open_until
                        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
                        .copied(),
                    now,
                ),
                circuit_until: backoffs
                    .route_circuit_open_until
                    .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
                    .copied(),
                transport_backoff_until: runtime_profile_transport_backoff_until_from_map(
                    &backoffs.transport_backoff_until,
                    profile_name,
                    route_kind,
                    now,
                ),
                health_score: runtime_profile_effective_health_score_from_map(
                    scores,
                    &runtime_profile_route_health_key(profile_name, route_kind),
                    now,
                ),
                bad_pairing_score: runtime_profile_effective_score_from_map(
                    scores,
                    &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
                    now,
                    RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
                ),
                performance_score: runtime_profile_effective_score_from_map(
                    scores,
                    &runtime_profile_route_performance_key(profile_name, route_kind),
                    now,
                    RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
                ),
                quota_band: runtime_quota_pressure_band_reason(quota_summary.route_band)
                    .to_string(),
                five_hour_status: runtime_quota_window_status_reason(
                    quota_summary.five_hour.status,
                )
                .to_string(),
                weekly_status: runtime_quota_window_status_reason(quota_summary.weekly.status)
                    .to_string(),
            }
        })
        .collect::<Vec<_>>();
        profiles.push(RuntimeDoctorProfileSummary {
            profile: profile_name.clone(),
            quota_freshness: runtime_doctor_quota_freshness_label(snapshot, now),
            quota_age_seconds,
            retry_backoff_until: backoffs.retry_backoff_until.get(profile_name).copied(),
            transport_backoff_until: runtime_profile_transport_backoff_max_until(
                &backoffs.transport_backoff_until,
                profile_name,
                now,
            ),
            routes,
        });
    }
    profiles
}

pub(crate) fn collect_runtime_doctor_state(paths: &AppPaths, summary: &mut RuntimeDoctorSummary) {
    let Ok(state) = AppState::load_with_recovery(paths) else {
        return;
    };
    let now = Local::now().timestamp();
    let usage_snapshots = load_runtime_usage_snapshots_with_recovery(paths, &state.value.profiles)
        .unwrap_or(RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        });
    let scores = load_runtime_profile_scores_with_recovery(paths, &state.value.profiles).unwrap_or(
        RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        },
    );
    let continuations = load_runtime_continuations_with_recovery(paths, &state.value.profiles)
        .unwrap_or(RecoveredLoad {
            value: RuntimeContinuationStore::default(),
            recovered_from_backup: false,
        });
    let continuation_journal =
        load_runtime_continuation_journal_with_recovery(paths, &state.value.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationJournal::default(),
                recovered_from_backup: false,
            },
        );
    let merged_continuations = merge_runtime_continuation_store(
        &continuations.value,
        &continuation_journal.value.continuations,
        &state.value.profiles,
    );
    let backoffs = load_runtime_profile_backoffs_with_recovery(paths, &state.value.profiles)
        .unwrap_or(RecoveredLoad {
            value: RuntimeProfileBackoffs::default(),
            recovered_from_backup: false,
        });
    let orphan_managed_dirs = collect_orphan_managed_profile_dirs(paths, &state.value);

    summary.persisted_retry_backoffs = backoffs.value.retry_backoff_until.len();
    summary.persisted_transport_backoffs = backoffs.value.transport_backoff_until.len();
    summary.persisted_route_circuits = backoffs.value.route_circuit_open_until.len();
    summary.persisted_usage_snapshots = usage_snapshots.value.len();
    summary.persisted_response_bindings =
        runtime_external_response_profile_bindings(&continuations.value.response_profile_bindings)
            .len();
    summary.persisted_session_bindings = continuations.value.session_profile_bindings.len();
    summary.persisted_turn_state_bindings = continuations.value.turn_state_bindings.len();
    summary.persisted_session_id_bindings = continuations.value.session_id_bindings.len();
    summary.persisted_continuation_journal_response_bindings = continuation_journal
        .value
        .continuations
        .response_profile_bindings
        .len();
    summary.persisted_continuation_journal_session_bindings = continuation_journal
        .value
        .continuations
        .session_profile_bindings
        .len();
    summary.persisted_continuation_journal_turn_state_bindings = continuation_journal
        .value
        .continuations
        .turn_state_bindings
        .len();
    summary.persisted_continuation_journal_session_id_bindings = continuation_journal
        .value
        .continuations
        .session_id_bindings
        .len();
    summary.continuation_journal_saved_at =
        (continuation_journal.value.saved_at > 0).then_some(continuation_journal.value.saved_at);
    summary.stale_persisted_usage_snapshots = usage_snapshots
        .value
        .values()
        .filter(|snapshot| !runtime_usage_snapshot_is_usable(snapshot, now))
        .count();
    summary.recovered_state_file = state.recovered_from_backup;
    summary.recovered_continuations_file = continuations.recovered_from_backup;
    summary.recovered_scores_file = scores.recovered_from_backup;
    summary.recovered_usage_snapshots_file = usage_snapshots.recovered_from_backup;
    summary.recovered_backoffs_file = backoffs.recovered_from_backup;
    summary.recovered_continuation_journal_file = continuation_journal.recovered_from_backup;
    summary.last_good_backups_present = [
        state_last_good_file_path(paths),
        runtime_continuations_last_good_file_path(paths),
        runtime_continuation_journal_last_good_file_path(paths),
        runtime_scores_last_good_file_path(paths),
        runtime_usage_snapshots_last_good_file_path(paths),
        runtime_backoffs_last_good_file_path(paths),
    ]
    .into_iter()
    .filter(|path| path.exists())
    .count();
    for statuses in [
        &merged_continuations.statuses.response,
        &merged_continuations.statuses.turn_state,
        &merged_continuations.statuses.session_id,
    ] {
        for (key, status) in statuses {
            match status.state {
                RuntimeContinuationBindingLifecycle::Verified => {
                    summary.persisted_verified_continuations += 1;
                }
                RuntimeContinuationBindingLifecycle::Warm => {
                    summary.persisted_warm_continuations += 1;
                }
                RuntimeContinuationBindingLifecycle::Suspect => {
                    summary.persisted_suspect_continuations += 1;
                    summary.suspect_continuation_bindings.push(format!(
                        "{key}:{}",
                        runtime_continuation_status_label(status)
                    ));
                }
                RuntimeContinuationBindingLifecycle::Dead => {
                    summary.persisted_dead_continuations += 1;
                }
            }
        }
    }
    summary.suspect_continuation_bindings.sort();
    summary.orphan_managed_dirs = orphan_managed_dirs;
    summary.profiles = runtime_doctor_profile_summaries(
        &state.value,
        &usage_snapshots.value,
        &scores.value,
        &backoffs.value,
        now,
    );
    let mut degraded_routes = RuntimeDoctorDegradedRouteBuilder::new();
    degraded_routes
        .push_route_circuits(&backoffs.value, now)
        .push_transport_backoffs(&backoffs.value)
        .push_retry_backoffs(&backoffs.value)
        .push_scores(&scores.value, now);
    summary.degraded_routes = degraded_routes.build();
}

pub(crate) fn collect_runtime_doctor_summary() -> RuntimeDoctorSummary {
    RuntimeDoctorCollector::discover().collect()
}

fn runtime_doctor_summary_from_log(log_path: Option<&Path>) -> RuntimeDoctorSummary {
    if let Some(log_path) = log_path.filter(|path| path.exists()) {
        match read_runtime_log_tail(log_path, RUNTIME_PROXY_DOCTOR_TAIL_BYTES) {
            Ok(tail) => summarize_runtime_log_tail(&tail),
            Err(err) => RuntimeDoctorSummary {
                diagnosis: format!("Failed to read the latest runtime log tail: {err}"),
                ..RuntimeDoctorSummary::default()
            },
        }
    } else {
        RuntimeDoctorSummary::default()
    }
}

fn runtime_doctor_selection_pressure(summary: &RuntimeDoctorSummary) -> String {
    if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS) {
        "elevated".to_string()
    } else {
        "low".to_string()
    }
}

fn runtime_doctor_transport_pressure(summary: &RuntimeDoctorSummary) -> String {
    if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS) {
        "elevated".to_string()
    } else {
        "low".to_string()
    }
}

fn runtime_doctor_persistence_pressure(summary: &RuntimeDoctorSummary) -> String {
    if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS) {
        "elevated".to_string()
    } else if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS) {
        "active".to_string()
    } else {
        "low".to_string()
    }
}

fn runtime_doctor_startup_audit_pressure(summary: &RuntimeDoctorSummary) -> String {
    if !summary.orphan_managed_dirs.is_empty()
        || runtime_doctor_marker_count(summary, "runtime_proxy_startup_audit") > 0
            && summary
                .marker_last_fields
                .get("runtime_proxy_startup_audit")
                .is_some_and(|fields| {
                    fields
                        .get("missing_managed_dirs")
                        .is_some_and(|value| value != "0")
                        || fields
                            .get("orphan_managed_dirs")
                            .is_some_and(|value| value != "0")
                })
    {
        "elevated".to_string()
    } else {
        "low".to_string()
    }
}

fn runtime_doctor_quota_freshness_pressure(summary: &RuntimeDoctorSummary) -> String {
    if summary.stale_persisted_usage_snapshots > 0
        || runtime_doctor_marker_count(summary, "profile_probe_refresh_error") > 0
        || runtime_doctor_top_facet(summary, "quota_source")
            .is_some_and(|value| value.starts_with("persisted_snapshot "))
    {
        "stale_risk".to_string()
    } else if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS) {
        "active".to_string()
    } else {
        "low".to_string()
    }
}

fn runtime_doctor_default_diagnosis(summary: &RuntimeDoctorSummary) -> String {
    if !summary.pointer_exists {
        "No runtime log pointer has been created yet.".to_string()
    } else if !summary.log_exists {
        "Latest runtime log path does not exist.".to_string()
    } else if summary.line_count == 0 {
        "Latest runtime log is empty.".to_string()
    } else if runtime_doctor_marker_count(summary, "runtime_proxy_overload_backoff") > 0 {
        "Recent local proxy overload backoff was triggered.".to_string()
    } else if runtime_doctor_marker_count(summary, "runtime_proxy_lane_limit_reached") > 0 {
        "Recent per-lane admission limit was triggered.".to_string()
    } else if runtime_doctor_marker_count(summary, "runtime_proxy_active_limit_reached") > 0 {
        "Recent global active-request admission limit was triggered.".to_string()
    } else if runtime_doctor_marker_count(summary, "runtime_proxy_queue_overloaded") > 0 {
        "Recent proxy saturation detected before commit.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_circuit_open") > 0 {
        "Recent route-level circuit breaker opened; fresh selection is temporarily steering away from a degraded profile.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_circuit_half_open_probe") > 0 {
        "Recent route-level circuit breaker entered half-open probing; fresh selection is cautiously testing a degraded profile before fully restoring it.".to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_precommit_frame_timeout") > 0 {
        "Recent websocket reuse/connect path failed to produce a first upstream frame before the pre-commit deadline.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_inflight_saturated") > 0 {
        "Recent per-profile in-flight saturation forced a fail-fast response.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_bad_pairing") > 0 {
        "Recent route-specific bad pairing memory is steering fresh selection away from a flaky account.".to_string()
    } else if runtime_doctor_marker_count(summary, "compact_fresh_fallback_blocked") > 0 {
        "Recent compact lineage guard blocked a fresh fallback so a follow-up stayed owner-first until upstream continuity was proven dead.".to_string()
    } else if runtime_doctor_marker_count(summary, "compact_pressure_shed") > 0 {
        "Recent pressure mode is shedding fresh compact requests to preserve continuation-heavy traffic.".to_string()
    } else if runtime_doctor_marker_count(summary, "previous_response_not_found") > 0 {
        format!(
            "Recent previous_response_id continuity failures were observed: {}.",
            runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_route)
        )
    } else if summary.compat_warning_count > 0 {
        format!(
            "Recent compatibility warnings were observed for {}: {}.",
            summary
                .top_client
                .clone()
                .or_else(|| summary.top_client_family.clone())
                .unwrap_or_else(|| "unknown client".to_string()),
            summary
                .top_compat_warning
                .clone()
                .unwrap_or_else(|| "inspect compat_warning markers".to_string())
        )
    } else if summary.persisted_dead_continuations > 0 {
        format!(
            "Some persisted continuations are currently dead and will be pruned: {}.",
            summary.persisted_dead_continuations
        )
    } else if !summary.suspect_continuation_bindings.is_empty() {
        format!(
            "Some persisted continuations are currently suspect: {}.",
            summary.suspect_continuation_bindings.join(", ")
        )
    } else if runtime_doctor_marker_count(summary, "websocket_reuse_watchdog") > 0 {
        "Recent websocket session reuse degraded before a terminal event; fresh reuse may be steering away from that profile.".to_string()
    } else if runtime_doctor_marker_count(summary, "selection_pick") > 0
        || runtime_doctor_marker_count(summary, "selection_skip_current") > 0
    {
        "Recent selection decisions were logged; inspect the last marker for why a profile was picked or skipped.".to_string()
    } else if runtime_doctor_marker_count(summary, "precommit_budget_exhausted") > 0 {
        "Recent candidate selection exhausted before commit.".to_string()
    } else if runtime_doctor_marker_count(summary, "upstream_usage_limit_passthrough") > 0
        || runtime_doctor_marker_count(summary, "responses_pre_send_skip") > 0
        || runtime_doctor_marker_count(summary, "websocket_pre_send_skip") > 0
        || runtime_doctor_marker_count(summary, "quota_critical_floor_before_send") > 0
    {
        "Recent quota hardening skipped near-exhausted sends or passed through upstream usage-limit responses.".to_string()
    } else if runtime_doctor_marker_count(summary, "stream_read_error") > 0 {
        "Recent upstream stream read failure detected after commit.".to_string()
    } else if runtime_doctor_marker_count(summary, "local_writer_error") > 0 {
        "Recent local writer failure detected while forwarding an upstream stream.".to_string()
    } else if runtime_doctor_marker_count(summary, "upstream_connect_timeout") > 0
        || runtime_doctor_marker_count(summary, "upstream_connect_dns_error") > 0
        || runtime_doctor_marker_count(summary, "upstream_tls_handshake_error") > 0
        || runtime_doctor_marker_count(summary, "upstream_connect_error") > 0
    {
        "Recent upstream connect failures detected.".to_string()
    } else if runtime_doctor_marker_count(summary, "state_save_error") > 0 {
        "Recent runtime state save failures detected.".to_string()
    } else if !summary.degraded_routes.is_empty() {
        format!(
            "Persisted degraded runtime routes are still active: {}",
            summary.degraded_routes.join(", ")
        )
    } else if !summary.orphan_managed_dirs.is_empty() {
        format!(
            "Orphan managed profile directories were detected: {}",
            summary.orphan_managed_dirs.join(", ")
        )
    } else if runtime_doctor_marker_count(summary, "profile_probe_refresh_error") > 0 {
        "Recent background quota refresh failures detected; fresh selection may rely on stale quota snapshots.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_probe_refresh_start") > 0 {
        "Background quota refresh activity was detected; inspect the last marker for the most recent profile refresh.".to_string()
    } else if runtime_doctor_marker_count(summary, "first_upstream_chunk") > 0
        && runtime_doctor_marker_count(summary, "first_local_chunk") == 0
    {
        "Likely writer stall: upstream produced data but the local writer did not emit a first chunk in the sampled tail."
            .to_string()
    } else {
        "No recent overload or stream-failure markers were detected in the sampled runtime tail."
            .to_string()
    }
}

fn runtime_doctor_finalize_summary(summary: &mut RuntimeDoctorSummary) {
    summary.selection_pressure = runtime_doctor_selection_pressure(summary);
    summary.transport_pressure = runtime_doctor_transport_pressure(summary);
    summary.persistence_pressure = runtime_doctor_persistence_pressure(summary);
    summary.startup_audit_pressure = runtime_doctor_startup_audit_pressure(summary);
    summary.quota_freshness_pressure = runtime_doctor_quota_freshness_pressure(summary);
    if summary.diagnosis.is_empty() {
        summary.diagnosis = runtime_doctor_default_diagnosis(summary);
    }
}

fn runtime_doctor_append_pointer_note(
    summary: &mut RuntimeDoctorSummary,
    pointer_note: Option<&str>,
) {
    if let Some(note) = pointer_note
        && !summary.diagnosis.contains(note)
    {
        summary.diagnosis = format!("{} {}", summary.diagnosis, note);
    }
}

pub(crate) fn read_runtime_log_tail(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .len();
    let start = len.saturating_sub(max_bytes as u64);
    file.seek(SeekFrom::Start(start))
        .with_context(|| format!("failed to seek {}", path.display()))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if start > 0
        && let Some(position) = buffer.iter().position(|byte| *byte == b'\n')
    {
        buffer.drain(..=position);
    }
    Ok(buffer)
}

pub(crate) fn summarize_runtime_log_tail(tail: &[u8]) -> RuntimeDoctorSummary {
    let text = String::from_utf8_lossy(tail);
    let mut summary = RuntimeDoctorSummary::default();
    for line in text.lines() {
        summary.line_count += 1;
        if let Some(timestamp) = runtime_doctor_line_timestamp(line) {
            if summary.first_timestamp.is_none() {
                summary.first_timestamp = Some(timestamp.clone());
            }
            summary.last_timestamp = Some(timestamp);
        }
        if let Some(marker) = runtime_doctor_marker_name(line) {
            *summary.marker_counts.entry(marker).or_insert(0) += 1;
            summary.last_marker_line = Some(runtime_doctor_truncate_line(line, 160));
            let fields = runtime_doctor_parse_fields(line);
            if marker == "previous_response_not_found" {
                if let Some(route) = fields.get("route").cloned() {
                    *summary
                        .previous_response_not_found_by_route
                        .entry(route)
                        .or_insert(0) += 1;
                }
                if let Some(transport) = fields.get("transport").cloned() {
                    *summary
                        .previous_response_not_found_by_transport
                        .entry(transport)
                        .or_insert(0) += 1;
                }
            }
            for facet in RUNTIME_DOCTOR_FACETS {
                if let Some(value) = fields.get(*facet).cloned() {
                    *summary
                        .facet_counts
                        .entry((*facet).to_string())
                        .or_default()
                        .entry(value)
                        .or_insert(0) += 1;
                }
            }
            if !fields.is_empty() {
                summary.marker_last_fields.insert(marker, fields);
            }
        }
    }
    runtime_doctor_finalize_log_summary(&mut summary);
    summary
}

fn runtime_doctor_line_timestamp(line: &str) -> Option<String> {
    if let Some(value) = runtime_doctor_json_line_value(line) {
        return value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
    }
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

fn runtime_doctor_json_line_value(line: &str) -> Option<serde_json::Value> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn runtime_doctor_line_message<'a>(line: &'a str) -> Cow<'a, str> {
    if let Some(value) = runtime_doctor_json_line_value(line)
        && let Some(message) = value.get("message").and_then(serde_json::Value::as_str)
    {
        return Cow::Owned(message.to_string());
    }
    Cow::Borrowed(
        line.split_once("] ")
            .map(|(_, message)| message)
            .unwrap_or(line)
            .trim(),
    )
}

fn runtime_doctor_parse_fields(line: &str) -> BTreeMap<String, String> {
    if let Some(value) = runtime_doctor_json_line_value(line)
        && let Some(fields) = value.get("fields").and_then(serde_json::Value::as_object)
    {
        let mut parsed = BTreeMap::new();
        for (key, value) in fields {
            let string_value = match value {
                serde_json::Value::String(value) => value.clone(),
                serde_json::Value::Number(value) => value.to_string(),
                serde_json::Value::Bool(value) => value.to_string(),
                _ => continue,
            };
            parsed.insert(key.clone(), string_value);
        }
        if !parsed.is_empty() {
            return parsed;
        }
    }

    let message = runtime_doctor_line_message(line);
    let mut fields = BTreeMap::new();
    for token in message.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    fields
}

fn runtime_doctor_marker_name(line: &str) -> Option<&'static str> {
    let message = runtime_doctor_line_message(line);
    RUNTIME_DOCTOR_MARKERS
        .iter()
        .copied()
        .find(|marker| message.contains(marker))
}

fn runtime_doctor_truncate_line(line: &str, limit: usize) -> String {
    let trimmed = line.trim();
    let count = trimmed.chars().count();
    if count <= limit {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>()
        + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_runtime_log_tail_understands_json_lines() {
        let tail = br#"{"timestamp":"2026-04-08 10:00:00.000 +00:00","message":"request=7 profile_health profile=main route=responses score=4","fields":{"request":"7","profile":"main","route":"responses","score":"4"}}"#;
        let summary = summarize_runtime_log_tail(tail);

        assert_eq!(summary.line_count, 1);
        assert_eq!(
            summary.marker_counts.get("profile_health").copied(),
            Some(1)
        );
        assert_eq!(
            summary.first_timestamp.as_deref(),
            Some("2026-04-08 10:00:00.000 +00:00")
        );
        assert_eq!(
            summary
                .marker_last_fields
                .get("profile_health")
                .and_then(|fields| fields.get("profile"))
                .map(String::as_str),
            Some("main")
        );
    }
}
