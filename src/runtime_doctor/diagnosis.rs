use std::collections::BTreeMap;

use super::*;

fn runtime_doctor_broker_artifact_fields(line: &str) -> BTreeMap<String, String> {
    line.split_whitespace()
        .filter_map(|token| token.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn runtime_doctor_broker_issue_diagnosis(summary: &RuntimeDoctorSummary) -> Option<String> {
    let mut stale_lease_diagnosis = None;
    for line in &summary.runtime_broker_identities {
        let fields = runtime_doctor_broker_artifact_fields(line);
        let broker_key = fields
            .get("broker_key")
            .map(String::as_str)
            .unwrap_or("unknown");
        let pid = fields.get("pid").map(String::as_str).unwrap_or("-");
        let listen_addr = fields.get("listen_addr").map(String::as_str).unwrap_or("-");
        let stale_leases = fields
            .get("stale_leases")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        match fields
            .get("status")
            .map(String::as_str)
            .unwrap_or("unknown")
        {
            "dead_pid" => {
                return Some(format!(
                    "Runtime broker registry {broker_key} points to dead pid {pid} at {listen_addr}; run `prodex cleanup` or restart `prodex run` so a fresh broker registry is written."
                ));
            }
            "health_timeout" => {
                return Some(format!(
                    "Runtime broker {broker_key} pid {pid} at {listen_addr} is alive but the health probe timed out; inspect local listener state and restart active prodex/codex sessions if it does not recover."
                ));
            }
            "health_unreachable" => {
                return Some(format!(
                    "Runtime broker {broker_key} pid {pid} at {listen_addr} is alive but the health probe is unreachable; inspect the local listener and restart active prodex/codex sessions if needed."
                ));
            }
            "binary_mismatch" => {
                let version = fields.get("version").map(String::as_str).unwrap_or("-");
                let path = fields.get("path").map(String::as_str).unwrap_or("-");
                let mismatch = fields
                    .get("mismatch")
                    .map(String::as_str)
                    .unwrap_or("unknown");
                return Some(format!(
                    "Runtime broker {broker_key} pid {pid} is running a different prodex binary ({mismatch}, observed version {version}, path {path}); restart active prodex/codex sessions so the current binary is loaded."
                ));
            }
            _ if stale_leases > 0 => {
                stale_lease_diagnosis = Some(format!(
                    "Runtime broker {broker_key} still has {stale_leases} stale lease file(s); run `prodex cleanup` after old terminals using that broker have exited."
                ));
            }
            _ => {}
        }
    }
    stale_lease_diagnosis
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

pub(crate) fn runtime_doctor_count_breakdown(counts: &BTreeMap<String, usize>) -> String {
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
                "chain_retried_owner",
                "chain_dead_upstream_confirmed",
                "stale_continuation",
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

pub(crate) fn runtime_doctor_finalize_log_summary(summary: &mut RuntimeDoctorSummary) {
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
    } else if let Some(diagnosis) = runtime_doctor_broker_issue_diagnosis(summary) {
        diagnosis
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
    } else if runtime_doctor_marker_count(summary, "chain_dead_upstream_confirmed") > 0 {
        format!(
            "Recent previous_response_id chain was confirmed dead upstream after owner retries. Latest chain event: {}.",
            summary
                .latest_chain_event
                .clone()
                .unwrap_or_else(|| "inspect chain_dead_upstream_confirmed markers".to_string())
        )
    } else if runtime_doctor_marker_count(summary, "stale_continuation") > 0 {
        format!(
            "Recent stale continuation was surfaced to Codex. Latest reason: {}.",
            summary
                .latest_stale_continuation_reason
                .clone()
                .unwrap_or_else(|| "inspect stale_continuation markers".to_string())
        )
    } else if runtime_doctor_marker_count(summary, "chain_retried_owner") > 0 {
        format!(
            "Recent continuation chain was retried on the owning profile before commit. Latest chain event: {}.",
            summary
                .latest_chain_event
                .clone()
                .unwrap_or_else(|| "inspect chain_retried_owner markers".to_string())
        )
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
    } else if summary.runtime_broker_mismatch {
        "A running runtime broker uses a different prodex binary than this command; restart active prodex/codex sessions so the patched runtime is loaded.".to_string()
    } else if summary.prodex_binary_mismatch {
        "Multiple prodex binaries on PATH differ by version or hash; align installs so new sessions use the patched runtime.".to_string()
    } else {
        "No recent overload or stream-failure markers were detected in the sampled runtime tail."
            .to_string()
    }
}

pub(crate) fn runtime_doctor_finalize_summary(summary: &mut RuntimeDoctorSummary) {
    summary.selection_pressure = runtime_doctor_selection_pressure(summary);
    summary.transport_pressure = runtime_doctor_transport_pressure(summary);
    summary.persistence_pressure = runtime_doctor_persistence_pressure(summary);
    summary.startup_audit_pressure = runtime_doctor_startup_audit_pressure(summary);
    summary.quota_freshness_pressure = runtime_doctor_quota_freshness_pressure(summary);
    if summary.diagnosis.is_empty() {
        summary.diagnosis = runtime_doctor_default_diagnosis(summary);
    }
}

pub(crate) fn runtime_doctor_append_pointer_note(
    summary: &mut RuntimeDoctorSummary,
    pointer_note: Option<&str>,
) {
    if let Some(note) = pointer_note
        && !summary.diagnosis.contains(note)
    {
        summary.diagnosis = format!("{} {}", summary.diagnosis, note);
    }
}
