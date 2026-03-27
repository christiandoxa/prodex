use anyhow::{Context, Result};
use chrono::Local;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::*;

pub(crate) fn runtime_doctor_json_value(summary: &RuntimeDoctorSummary) -> serde_json::Value {
    let marker_counts = summary
        .marker_counts
        .iter()
        .map(|(marker, count)| ((*marker).to_string(), serde_json::Value::from(*count)))
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let marker_last_fields = summary
        .marker_last_fields
        .iter()
        .map(|(marker, fields)| {
            let fields = fields
                .iter()
                .map(|(key, value)| (key.clone(), serde_json::Value::from(value.clone())))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            ((*marker).to_string(), serde_json::Value::Object(fields))
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let facet_counts = summary
        .facet_counts
        .iter()
        .map(|(facet, counts)| {
            let counts = counts
                .iter()
                .map(|(value, count)| (value.clone(), serde_json::Value::from(*count)))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            (facet.clone(), serde_json::Value::Object(counts))
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let previous_response_not_found_by_route = summary
        .previous_response_not_found_by_route
        .iter()
        .map(|(route, count)| (route.clone(), serde_json::Value::from(*count)))
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let previous_response_not_found_by_transport = summary
        .previous_response_not_found_by_transport
        .iter()
        .map(|(transport, count)| (transport.clone(), serde_json::Value::from(*count)))
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let profiles = summary
        .profiles
        .iter()
        .map(|profile| {
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
                        "health_score": route.health_score,
                        "bad_pairing_score": route.bad_pairing_score,
                        "performance_score": route.performance_score,
                        "quota_band": route.quota_band,
                        "five_hour_status": route.five_hour_status,
                        "weekly_status": route.weekly_status,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "log_path": summary.log_path.as_ref().map(|path| path.display().to_string()),
        "pointer_exists": summary.pointer_exists,
        "log_exists": summary.log_exists,
        "line_count": summary.line_count,
        "first_timestamp": summary.first_timestamp,
        "last_timestamp": summary.last_timestamp,
        "marker_counts": marker_counts,
        "marker_last_fields": marker_last_fields,
        "facet_counts": facet_counts,
        "previous_response_not_found_by_route": previous_response_not_found_by_route,
        "previous_response_not_found_by_transport": previous_response_not_found_by_transport,
        "last_marker_line": summary.last_marker_line,
        "selection_pressure": summary.selection_pressure,
        "transport_pressure": summary.transport_pressure,
        "persistence_pressure": summary.persistence_pressure,
        "quota_freshness_pressure": summary.quota_freshness_pressure,
        "startup_audit_pressure": summary.startup_audit_pressure,
        "persisted_retry_backoffs": summary.persisted_retry_backoffs,
        "persisted_transport_backoffs": summary.persisted_transport_backoffs,
        "persisted_route_circuits": summary.persisted_route_circuits,
        "persisted_usage_snapshots": summary.persisted_usage_snapshots,
        "persisted_response_bindings": summary.persisted_response_bindings,
        "persisted_session_bindings": summary.persisted_session_bindings,
        "persisted_turn_state_bindings": summary.persisted_turn_state_bindings,
        "persisted_session_id_bindings": summary.persisted_session_id_bindings,
        "stale_persisted_usage_snapshots": summary.stale_persisted_usage_snapshots,
        "recovered_state_file": summary.recovered_state_file,
        "recovered_continuations_file": summary.recovered_continuations_file,
        "recovered_scores_file": summary.recovered_scores_file,
        "recovered_usage_snapshots_file": summary.recovered_usage_snapshots_file,
        "recovered_backoffs_file": summary.recovered_backoffs_file,
        "last_good_backups_present": summary.last_good_backups_present,
        "degraded_routes": summary.degraded_routes,
        "orphan_managed_dirs": summary.orphan_managed_dirs,
        "profiles": profiles,
        "diagnosis": summary.diagnosis,
    })
}

pub(crate) fn runtime_doctor_fields() -> Vec<(String, String)> {
    let pointer_path = runtime_proxy_latest_log_pointer_path();
    let summary = collect_runtime_doctor_summary();
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

    vec![
        (
            "Log pointer".to_string(),
            format!(
                "{} ({})",
                pointer_path.display(),
                if summary.pointer_exists {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        ("Latest log".to_string(), latest_log),
        (
            "Log sample".to_string(),
            format!("{} lines", summary.line_count),
        ),
        (
            "Queue overload".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_queue_overloaded").to_string(),
        ),
        (
            "Active limit".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_active_limit_reached").to_string(),
        ),
        (
            "Lane limit".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_lane_limit_reached").to_string(),
        ),
        (
            "Overload backoff".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_overload_backoff").to_string(),
        ),
        (
            "Connect failures".to_string(),
            (runtime_doctor_marker_count(&summary, "upstream_connect_timeout")
                + runtime_doctor_marker_count(&summary, "upstream_connect_error"))
            .to_string(),
        ),
        (
            "Pre-commit budget".to_string(),
            runtime_doctor_marker_count(&summary, "precommit_budget_exhausted").to_string(),
        ),
        (
            "Retry backoff".to_string(),
            runtime_doctor_marker_count(&summary, "profile_retry_backoff").to_string(),
        ),
        (
            "Transport backoff".to_string(),
            runtime_doctor_marker_count(&summary, "profile_transport_backoff").to_string(),
        ),
        (
            "Route circuits".to_string(),
            runtime_doctor_marker_count(&summary, "profile_circuit_open").to_string(),
        ),
        (
            "Health penalties".to_string(),
            runtime_doctor_marker_count(&summary, "profile_health").to_string(),
        ),
        (
            "Latency penalties".to_string(),
            runtime_doctor_marker_count(&summary, "profile_latency").to_string(),
        ),
        (
            "Bad pairing".to_string(),
            runtime_doctor_marker_count(&summary, "profile_bad_pairing").to_string(),
        ),
        (
            "Prev not found".to_string(),
            runtime_doctor_marker_count(&summary, "previous_response_not_found").to_string(),
        ),
        (
            "Prev not found routes".to_string(),
            runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_route),
        ),
        (
            "Prev not found xport".to_string(),
            runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_transport),
        ),
        (
            "Prev negative cache".to_string(),
            runtime_doctor_marker_count(&summary, "previous_response_negative_cache").to_string(),
        ),
        (
            "Compact guard".to_string(),
            runtime_doctor_marker_count(&summary, "compact_fresh_fallback_blocked").to_string(),
        ),
        (
            "Selection picks".to_string(),
            runtime_doctor_marker_count(&summary, "selection_pick").to_string(),
        ),
        (
            "Selection skips".to_string(),
            runtime_doctor_marker_count(&summary, "selection_skip_current").to_string(),
        ),
        (
            "WS reuse watchdog".to_string(),
            runtime_doctor_marker_count(&summary, "websocket_reuse_watchdog").to_string(),
        ),
        (
            "WS first-frame timeouts".to_string(),
            runtime_doctor_marker_count(&summary, "websocket_precommit_frame_timeout").to_string(),
        ),
        (
            "Stream read errors".to_string(),
            runtime_doctor_marker_count(&summary, "stream_read_error").to_string(),
        ),
        (
            "Writer errors".to_string(),
            runtime_doctor_marker_count(&summary, "local_writer_error").to_string(),
        ),
        (
            "State save errors".to_string(),
            runtime_doctor_marker_count(&summary, "state_save_error").to_string(),
        ),
        (
            "State save ok".to_string(),
            runtime_doctor_marker_count(&summary, "state_save_ok").to_string(),
        ),
        (
            "State save skipped".to_string(),
            runtime_doctor_marker_count(&summary, "state_save_skipped").to_string(),
        ),
        (
            "Startup audit".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_startup_audit").to_string(),
        ),
        (
            "Startup pressure".to_string(),
            summary.startup_audit_pressure.clone(),
        ),
        (
            "Admission recovered".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_admission_recovered").to_string(),
        ),
        (
            "Queue recovered".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_queue_recovered").to_string(),
        ),
        (
            "Probe refresh".to_string(),
            runtime_doctor_marker_count(&summary, "profile_probe_refresh_start").to_string(),
        ),
        (
            "Probe refresh errors".to_string(),
            runtime_doctor_marker_count(&summary, "profile_probe_refresh_error").to_string(),
        ),
        (
            "Hot lane".to_string(),
            runtime_doctor_top_facet(&summary, "lane").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Hot route".to_string(),
            runtime_doctor_top_facet(&summary, "route").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Hot profile".to_string(),
            runtime_doctor_top_facet(&summary, "profile").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Hot reason".to_string(),
            runtime_doctor_top_facet(&summary, "reason").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Quota source".to_string(),
            runtime_doctor_top_facet(&summary, "quota_source").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Selection pressure".to_string(),
            summary.selection_pressure.clone(),
        ),
        (
            "Transport pressure".to_string(),
            summary.transport_pressure.clone(),
        ),
        (
            "Persistence pressure".to_string(),
            summary.persistence_pressure.clone(),
        ),
        (
            "Quota freshness".to_string(),
            summary.quota_freshness_pressure.clone(),
        ),
        (
            "Persisted backoffs".to_string(),
            format!(
                "retry={} transport={} circuits={}",
                summary.persisted_retry_backoffs,
                summary.persisted_transport_backoffs,
                summary.persisted_route_circuits
            ),
        ),
        (
            "Persisted snapshots".to_string(),
            format!(
                "{} total, {} stale",
                summary.persisted_usage_snapshots, summary.stale_persisted_usage_snapshots
            ),
        ),
        (
            "Persisted continuations".to_string(),
            format!(
                "responses={} sessions={} turns={} session_ids={}",
                summary.persisted_response_bindings,
                summary.persisted_session_bindings,
                summary.persisted_turn_state_bindings,
                summary.persisted_session_id_bindings
            ),
        ),
        (
            "Recovered state".to_string(),
            format!(
                "state={} continuations={} scores={} usage={} backoffs={} backups={}",
                summary.recovered_state_file,
                summary.recovered_continuations_file,
                summary.recovered_scores_file,
                summary.recovered_usage_snapshots_file,
                summary.recovered_backoffs_file,
                summary.last_good_backups_present
            ),
        ),
        (
            "Degraded routes".to_string(),
            if summary.degraded_routes.is_empty() {
                "-".to_string()
            } else {
                summary.degraded_routes.join(" | ")
            },
        ),
        (
            "Orphan dirs".to_string(),
            if summary.orphan_managed_dirs.is_empty() {
                "-".to_string()
            } else {
                summary.orphan_managed_dirs.join(", ")
            },
        ),
        (
            "Last marker".to_string(),
            summary
                .last_marker_line
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        ),
        ("Diagnosis".to_string(), summary.diagnosis),
    ]
}

pub(crate) fn runtime_doctor_marker_count(
    summary: &RuntimeDoctorSummary,
    marker: &'static str,
) -> usize {
    summary.marker_counts.get(marker).copied().unwrap_or(0)
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
            transport_backoff_until: backoffs.transport_backoff_until.get(profile_name).copied(),
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
    summary.persisted_response_bindings = continuations.value.response_profile_bindings.len();
    summary.persisted_session_bindings = continuations.value.session_profile_bindings.len();
    summary.persisted_turn_state_bindings = continuations.value.turn_state_bindings.len();
    summary.persisted_session_id_bindings = continuations.value.session_id_bindings.len();
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
    summary.last_good_backups_present = [
        state_last_good_file_path(paths),
        runtime_continuations_last_good_file_path(paths),
        runtime_scores_last_good_file_path(paths),
        runtime_usage_snapshots_last_good_file_path(paths),
        runtime_backoffs_last_good_file_path(paths),
    ]
    .into_iter()
    .filter(|path| path.exists())
    .count();
    summary.orphan_managed_dirs = orphan_managed_dirs;
    summary.profiles = runtime_doctor_profile_summaries(
        &state.value,
        &usage_snapshots.value,
        &scores.value,
        &backoffs.value,
        now,
    );

    let mut degraded_routes = Vec::new();
    for (key, until) in &backoffs.value.route_circuit_open_until {
        if let Some((route, profile_name)) =
            runtime_profile_route_key_parts(key, "__route_circuit__:")
        {
            let state = if *until > now { "open" } else { "half-open" };
            degraded_routes.push(format!(
                "{profile_name}/{route} circuit={state} until={until}"
            ));
        }
    }
    for (profile_name, until) in &backoffs.value.transport_backoff_until {
        degraded_routes.push(format!(
            "{profile_name}/transport transport_backoff until={until}"
        ));
    }
    for (profile_name, until) in &backoffs.value.retry_backoff_until {
        degraded_routes.push(format!("{profile_name}/retry retry_backoff until={until}"));
    }
    for (key, health) in &scores.value {
        if let Some((route, profile_name)) =
            runtime_profile_route_key_parts(key, "__route_bad_pairing__:")
        {
            let score = runtime_profile_effective_score(
                health,
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            if score > 0 {
                degraded_routes.push(format!("{profile_name}/{route} bad_pairing={score}"));
            }
            continue;
        }
        if let Some((route, profile_name)) =
            runtime_profile_route_key_parts(key, "__route_health__:")
        {
            let score = runtime_profile_effective_health_score(health, now);
            if score > 0 {
                degraded_routes.push(format!("{profile_name}/{route} health={score}"));
            }
        }
    }
    degraded_routes.sort();
    degraded_routes.dedup();
    summary.degraded_routes = degraded_routes.into_iter().take(8).collect();
}

pub(crate) fn collect_runtime_doctor_summary() -> RuntimeDoctorSummary {
    let paths = AppPaths::discover().ok();
    let pointer_path = runtime_proxy_latest_log_pointer_path();
    let pointer_content = fs::read_to_string(&pointer_path).ok();
    let log_path = pointer_content
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let pointer_exists = pointer_path.exists();
    let log_exists = log_path.as_ref().is_some_and(|path| path.exists());

    let mut summary = if let Some(log_path) = log_path.as_ref().filter(|path| path.exists()) {
        match read_runtime_log_tail(log_path, RUNTIME_PROXY_DOCTOR_TAIL_BYTES) {
            Ok(tail) => summarize_runtime_log_tail(&tail),
            Err(err) => RuntimeDoctorSummary {
                diagnosis: format!("Failed to read the latest runtime log tail: {err}"),
                ..RuntimeDoctorSummary::default()
            },
        }
    } else {
        RuntimeDoctorSummary::default()
    };

    summary.pointer_exists = pointer_exists;
    summary.log_exists = log_exists;
    summary.log_path = log_path;
    if let Some(paths) = paths.as_ref() {
        collect_runtime_doctor_state(paths, &mut summary);
    }
    summary.selection_pressure = if runtime_doctor_marker_count(&summary, "selection_pick") > 0
        || runtime_doctor_marker_count(&summary, "selection_skip_current") > 0
        || runtime_doctor_marker_count(&summary, "selection_skip_affinity") > 0
        || runtime_doctor_marker_count(&summary, "precommit_budget_exhausted") > 0
    {
        "elevated".to_string()
    } else {
        "low".to_string()
    };
    summary.transport_pressure = if runtime_doctor_marker_count(&summary, "stream_read_error") > 0
        || runtime_doctor_marker_count(&summary, "upstream_connect_timeout") > 0
        || runtime_doctor_marker_count(&summary, "upstream_connect_error") > 0
        || runtime_doctor_marker_count(&summary, "profile_transport_backoff") > 0
        || runtime_doctor_marker_count(&summary, "profile_circuit_open") > 0
        || runtime_doctor_marker_count(&summary, "profile_circuit_half_open_probe") > 0
        || runtime_doctor_marker_count(&summary, "websocket_precommit_frame_timeout") > 0
        || runtime_doctor_marker_count(&summary, "local_writer_error") > 0
    {
        "elevated".to_string()
    } else {
        "low".to_string()
    };
    summary.persistence_pressure = if runtime_doctor_marker_count(&summary, "state_save_error") > 0
    {
        "elevated".to_string()
    } else if runtime_doctor_marker_count(&summary, "state_save_skipped") > 0 {
        "active".to_string()
    } else {
        "low".to_string()
    };
    summary.startup_audit_pressure = if !summary.orphan_managed_dirs.is_empty()
        || runtime_doctor_marker_count(&summary, "runtime_proxy_startup_audit") > 0
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
                }) {
        "elevated".to_string()
    } else {
        "low".to_string()
    };
    summary.quota_freshness_pressure = if summary.stale_persisted_usage_snapshots > 0
        || runtime_doctor_marker_count(&summary, "profile_probe_refresh_error") > 0
        || runtime_doctor_top_facet(&summary, "quota_source")
            .is_some_and(|value| value.starts_with("persisted_snapshot "))
    {
        "stale_risk".to_string()
    } else if runtime_doctor_marker_count(&summary, "profile_probe_refresh_start") > 0
        || runtime_doctor_marker_count(&summary, "profile_probe_refresh_ok") > 0
    {
        "active".to_string()
    } else {
        "low".to_string()
    };
    if summary.diagnosis.is_empty() {
        summary.diagnosis = if !summary.pointer_exists {
            "No runtime log pointer has been created yet.".to_string()
        } else if !summary.log_exists {
            "Latest runtime log path does not exist.".to_string()
        } else if summary.line_count == 0 {
            "Latest runtime log is empty.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_overload_backoff") > 0 {
            "Recent local proxy overload backoff was triggered.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_lane_limit_reached") > 0 {
            "Recent per-lane admission limit was triggered.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_active_limit_reached") > 0 {
            "Recent global active-request admission limit was triggered.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_queue_overloaded") > 0 {
            "Recent proxy saturation detected before commit.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_circuit_open") > 0 {
            "Recent route-level circuit breaker opened; fresh selection is temporarily steering away from a degraded profile.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_circuit_half_open_probe") > 0 {
            "Recent route-level circuit breaker entered half-open probing; fresh selection is cautiously testing a degraded profile before fully restoring it.".to_string()
        } else if runtime_doctor_marker_count(&summary, "websocket_precommit_frame_timeout") > 0 {
            "Recent websocket reuse/connect path failed to produce a first upstream frame before the pre-commit deadline.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_inflight_saturated") > 0 {
            "Recent per-profile in-flight saturation forced a fail-fast response.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_bad_pairing") > 0 {
            "Recent route-specific bad pairing memory is steering fresh selection away from a flaky account.".to_string()
        } else if runtime_doctor_marker_count(&summary, "compact_fresh_fallback_blocked") > 0 {
            "Recent compact lineage guard blocked a fresh fallback so a follow-up stayed owner-first until upstream continuity was proven dead.".to_string()
        } else if runtime_doctor_marker_count(&summary, "previous_response_not_found") > 0 {
            format!(
                "Recent previous_response_id continuity failures were observed: {}.",
                runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_route)
            )
        } else if runtime_doctor_marker_count(&summary, "websocket_reuse_watchdog") > 0 {
            "Recent websocket session reuse degraded before a terminal event; fresh reuse may be steering away from that profile.".to_string()
        } else if runtime_doctor_marker_count(&summary, "selection_pick") > 0
            || runtime_doctor_marker_count(&summary, "selection_skip_current") > 0
        {
            "Recent selection decisions were logged; inspect the last marker for why a profile was picked or skipped.".to_string()
        } else if runtime_doctor_marker_count(&summary, "precommit_budget_exhausted") > 0 {
            "Recent candidate selection exhausted before commit.".to_string()
        } else if runtime_doctor_marker_count(&summary, "stream_read_error") > 0 {
            "Recent upstream stream read failure detected after commit.".to_string()
        } else if runtime_doctor_marker_count(&summary, "local_writer_error") > 0 {
            "Recent local writer failure detected while forwarding an upstream stream.".to_string()
        } else if runtime_doctor_marker_count(&summary, "upstream_connect_timeout") > 0
            || runtime_doctor_marker_count(&summary, "upstream_connect_error") > 0
        {
            "Recent upstream connect failures detected.".to_string()
        } else if runtime_doctor_marker_count(&summary, "state_save_error") > 0 {
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
        } else if runtime_doctor_marker_count(&summary, "profile_probe_refresh_error") > 0 {
            "Recent background quota refresh failures detected; fresh selection may rely on stale quota snapshots.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_probe_refresh_start") > 0 {
            "Background quota refresh activity was detected; inspect the last marker for the most recent profile refresh.".to_string()
        } else if runtime_doctor_marker_count(&summary, "first_upstream_chunk") > 0
            && runtime_doctor_marker_count(&summary, "first_local_chunk") == 0
        {
            "Likely writer stall: upstream produced data but the local writer did not emit a first chunk in the sampled tail."
                .to_string()
        } else {
            "No recent overload or stream-failure markers were detected in the sampled runtime tail."
                .to_string()
        };
    }
    summary
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
            for facet in [
                "lane",
                "route",
                "profile",
                "reason",
                "transport",
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
            ] {
                if let Some(value) = fields.get(facet).cloned() {
                    *summary
                        .facet_counts
                        .entry(facet.to_string())
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
    summary
}

fn runtime_doctor_line_timestamp(line: &str) -> Option<String> {
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

fn runtime_doctor_parse_fields(line: &str) -> BTreeMap<String, String> {
    let message = line
        .split_once("] ")
        .map(|(_, message)| message)
        .unwrap_or(line)
        .trim();
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
    [
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
        "compact_lineage_released",
        "selection_pick",
        "selection_skip_current",
        "selection_skip_affinity",
        "quota_release_profile_affinity",
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
        "runtime_proxy_restore_counts",
        "runtime_proxy_startup_audit",
        "profile_probe_refresh_queued",
        "profile_probe_refresh_start",
        "profile_probe_refresh_ok",
        "profile_probe_refresh_error",
        "quota_blocked_affinity_released",
    ]
    .into_iter()
    .find(|marker| line.contains(marker))
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
