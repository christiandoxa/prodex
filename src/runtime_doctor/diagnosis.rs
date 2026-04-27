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

fn runtime_doctor_marker_scope(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    profile_field: &str,
    route_field: &str,
) -> Option<String> {
    let profile = runtime_doctor_marker_last_field(summary, marker, profile_field);
    let route = runtime_doctor_marker_last_field(summary, marker, route_field);
    match (profile, route) {
        (Some(profile), Some(route)) => Some(format!("{profile}/{route}")),
        (Some(profile), None) => Some(profile.to_string()),
        (None, Some(route)) => Some(route.to_string()),
        (None, None) => None,
    }
}

fn runtime_doctor_admission_pressure_load(summary: &RuntimeDoctorSummary, marker: &str) -> String {
    match (
        runtime_doctor_marker_last_field(summary, marker, "active"),
        runtime_doctor_marker_last_field(summary, marker, "limit"),
    ) {
        (Some(active), Some(limit)) => format!(" Latest load: {active}/{limit}."),
        _ => String::new(),
    }
}

pub(crate) fn runtime_doctor_previous_response_fail_closed_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    let reason = runtime_doctor_marker_last_field(
        summary,
        "previous_response_fresh_fallback_blocked",
        "reason",
    )
    .unwrap_or("unknown_reason");
    if runtime_doctor_has_context_dependent_fail_closed(summary) {
        return format!(
            "Inspect `previous_response_not_found`, affinity bindings, and owning-profile chain markers before retrying; Prodex failed closed because this follow-up is context-dependent and cannot be replayed safely. Start a fresh turn only if context continuity can be abandoned. Latest guard: {reason}."
        );
    }
    format!(
        "Inspect `previous_response_not_found` and `chain_dead_upstream_confirmed` for the owning context before retrying; fail-closed stale continuation handling blocks fresh replay when continuity is unverified. Start a fresh turn instead of forcing rotation if the owner cannot be recovered. Latest guard: {reason}."
    )
}

pub(crate) fn runtime_doctor_compact_final_failure_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    let exit =
        runtime_doctor_marker_last_field(summary, "compact_final_failure", "exit").unwrap_or("-");
    let reason =
        runtime_doctor_marker_last_field(summary, "compact_final_failure", "reason").unwrap_or("-");
    let profile = runtime_doctor_marker_last_field(summary, "compact_final_failure", "profile")
        .map(|profile| format!(" on profile {profile}"))
        .unwrap_or_default();
    match (exit, reason) {
        ("pressure", _) => {
            "Reduce fresh compact volume or wait for continuation-heavy traffic to drain before retrying compact.".to_string()
        }
        (_, "quota") => format!(
            "Inspect compact budget and candidate-exhausted markers{profile}, then retry after compact quota refreshes or another profile becomes eligible."
        ),
        (_, "overload") => format!(
            "Inspect compact overload and backoff markers{profile}, then retry after the local pressure clears."
        ),
        (_, "inflight_saturation") => format!(
            "Wait for in-flight compact work to drain{profile} before retrying."
        ),
        _ => format!(
            "Inspect compact exit markers around `{exit}`{profile} and retry after the blocking condition clears."
        ),
    }
}

pub(crate) fn runtime_doctor_lane_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    let lane =
        runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
            .unwrap_or("unknown");
    let load = runtime_doctor_admission_pressure_load(summary, "runtime_proxy_lane_limit_reached");
    if lane == "responses" {
        format!(
            "Reduce concurrent terminals or bursty side-lane work until the responses lane drains.{load}"
        )
    } else {
        format!(
            "Inspect repeated lane={lane} markers and trim bursty {lane} traffic if it is starving responses.{load}"
        )
    }
}

pub(crate) fn runtime_doctor_active_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    let load =
        runtime_doctor_admission_pressure_load(summary, "runtime_proxy_active_limit_reached");
    format!(
        "Reduce concurrent fresh work or wait for in-flight requests to drain before retrying.{load}"
    )
}

pub(crate) fn runtime_doctor_profile_inflight_saturated_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    let profile =
        runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "profile")
            .map(|profile| format!(" on profile {profile}"))
            .unwrap_or_default();
    let hard_limit =
        runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "hard_limit");
    match hard_limit {
        Some(limit) => format!(
            "Wait for in-flight work{profile} to drop below hard limit {limit} before retrying, or let fresh selection land on another eligible profile."
        ),
        None => format!(
            "Wait for in-flight work{profile} to drain before retrying, or let fresh selection land on another eligible profile."
        ),
    }
}

pub(crate) fn runtime_doctor_route_health_next_step(summary: &RuntimeDoctorSummary) -> String {
    let scope = runtime_doctor_marker_scope(summary, "profile_health", "profile", "route")
        .unwrap_or_else(|| "that route".to_string());
    let reason = runtime_doctor_marker_last_field(summary, "profile_health", "reason")
        .unwrap_or("unknown_reason");
    format!(
        "Inspect recent transport or overload markers for {scope}, especially `{reason}`, and wait for that route score to decay before expecting fresh selection to reuse it."
    )
}

fn runtime_doctor_websocket_connect_overflow_marker(
    summary: &RuntimeDoctorSummary,
) -> &'static str {
    if runtime_doctor_marker_count(summary, "websocket_connect_overflow_rejected") > 0 {
        "websocket_connect_overflow_rejected"
    } else if runtime_doctor_marker_count(summary, "websocket_connect_overflow_reject") > 0 {
        "websocket_connect_overflow_reject"
    } else if runtime_doctor_marker_count(summary, "websocket_connect_overflow_enqueue") > 0 {
        "websocket_connect_overflow_enqueue"
    } else {
        "websocket_connect_overflow_dispatch"
    }
}

pub(crate) fn runtime_doctor_websocket_connect_overflow_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    let marker = runtime_doctor_websocket_connect_overflow_marker(summary);
    let reason =
        runtime_doctor_marker_last_field(summary, marker, "reason").unwrap_or("unknown_reason");
    let pending =
        runtime_doctor_marker_last_field(summary, marker, "overflow_pending").unwrap_or("-");
    let max_pending =
        runtime_doctor_marker_last_field(summary, marker, "overflow_max_pending").unwrap_or("-");
    let worker_count =
        runtime_doctor_marker_last_field(summary, marker, "worker_count").unwrap_or("-");
    let queue_capacity =
        runtime_doctor_marker_last_field(summary, marker, "queue_capacity").unwrap_or("-");
    if marker == "websocket_connect_overflow_rejected"
        || marker == "websocket_connect_overflow_reject"
    {
        format!(
            "Reduce concurrent websocket session starts or wait for websocket connect workers to drain before retrying. Latest reason: {reason}; pending={pending}/{max_pending}, workers={worker_count}, queue_capacity={queue_capacity}."
        )
    } else if marker == "websocket_connect_overflow_dispatch" {
        format!(
            "Overflow queued websocket connect work drained back into the bounded workers; inspect earlier enqueue/reject markers if dispatch repeats. Latest reason: {reason}; pending={pending}/{max_pending}, workers={worker_count}, queue_capacity={queue_capacity}."
        )
    } else {
        format!(
            "Watch for matching dispatch or rejected markers; repeated enqueue means websocket connect workers are saturated. Latest reason: {reason}; pending={pending}/{max_pending}, workers={worker_count}, queue_capacity={queue_capacity}."
        )
    }
}

fn runtime_doctor_profile_auth_marker(summary: &RuntimeDoctorSummary) -> &'static str {
    if runtime_doctor_marker_count(summary, "profile_auth_recovery_failed") > 0 {
        "profile_auth_recovery_failed"
    } else {
        "profile_auth_recovered"
    }
}

pub(crate) fn runtime_doctor_profile_auth_recovery_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    let marker = runtime_doctor_profile_auth_marker(summary);
    let profile = runtime_doctor_marker_last_field(summary, marker, "profile").unwrap_or("-");
    let route = runtime_doctor_marker_last_field(summary, marker, "route").unwrap_or("-");
    if marker == "profile_auth_recovery_failed" {
        let error =
            runtime_doctor_marker_last_field(summary, marker, "error").unwrap_or("unknown_error");
        format!(
            "Refresh credentials for profile {profile} with `prodex login --profile {profile}` and retry route {route}; latest recovery error: {error}."
        )
    } else {
        let source = runtime_doctor_marker_last_field(summary, marker, "source").unwrap_or("-");
        let changed = runtime_doctor_marker_last_field(summary, marker, "changed").unwrap_or("-");
        format!(
            "Auth recovered for profile {profile} on route {route} via {source} (changed={changed}); if this repeats, restart active sessions after login refresh."
        )
    }
}

pub(crate) fn runtime_doctor_persistence_backpressure_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    let mut backlogs = Vec::new();
    if let Some(backlog) = summary.state_save_queue_backlog {
        backlogs.push(format!("state={backlog}"));
    }
    if let Some(backlog) = summary.continuation_journal_save_backlog {
        backlogs.push(format!("journal={backlog}"));
    }
    let latest_reason =
        runtime_doctor_marker_last_field(summary, "state_save_queue_backpressure", "reason")
            .or_else(|| {
                runtime_doctor_marker_last_field(
                    summary,
                    "continuation_journal_queue_backpressure",
                    "reason",
                )
            });
    let backlog_detail = if backlogs.is_empty() {
        String::new()
    } else {
        format!(" Latest backlog: {}.", backlogs.join(" "))
    };
    let reason_detail = latest_reason
        .map(|reason| format!(" Latest reason: {reason}."))
        .unwrap_or_default();
    format!(
        "Reduce rapid rotation or continuation churn and wait for background persistence queues to drain.{backlog_detail}{reason_detail}"
    )
}

pub(crate) fn runtime_doctor_sync_probe_skip_next_step(summary: &RuntimeDoctorSummary) -> String {
    let route = runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "route")
        .unwrap_or("unknown");
    let reason = runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "reason")
        .unwrap_or("unknown_reason");
    let deferred =
        runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "cold_start_jobs")
            .map(|count| format!("{count} cold-start job(s)"))
            .or_else(|| {
                runtime_doctor_marker_last_field(
                    summary,
                    "selection_skip_sync_probe",
                    "cold_start_profiles",
                )
                .map(|count| format!("{count} cold-start profile(s)"))
            })
            .unwrap_or_else(|| "cold-start work".to_string());
    format!(
        "Inspect `selection_skip_sync_probe`, `profile_probe_refresh_backpressure`, and `profile_probe_refresh_queued` markers for route {route}; pressure mode ({reason}) deferred {deferred}, so cold-start profiles may stay on stale quota data until background probes finish."
    )
}

pub(crate) fn runtime_doctor_probe_refresh_backpressure_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    let profile =
        runtime_doctor_marker_last_field(summary, "profile_probe_refresh_backpressure", "profile");
    let backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "profile_probe_refresh_backpressure",
        "backlog",
    )
    .or(summary.profile_probe_refresh_backlog);
    let profile_detail = profile
        .map(|profile| format!(" for profile {profile}"))
        .unwrap_or_default();
    let backlog_detail = backlog
        .map(|backlog| format!(" Latest probe backlog: {backlog}."))
        .unwrap_or_default();
    format!(
        "Let the background quota-refresh queue drain{profile_detail} before expecting cold-start profiles to become selectable again.{backlog_detail}"
    )
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
    let classes: [(&str, &[&str]); 6] = [
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
                "websocket_connect_overflow_reject",
                "websocket_connect_overflow_rejected",
                "compact_precommit_budget_exhausted",
                "compact_candidate_exhausted",
            ],
        ),
        (
            "auth",
            &[
                "profile_auth_recovery_failed",
                "profile_auth_proactive_sync_failed",
            ],
        ),
        (
            "continuation",
            &[
                "previous_response_not_found",
                "previous_response_negative_cache",
                "previous_response_fresh_fallback",
                "chain_retried_owner",
                "chain_dead_upstream_confirmed",
                "stale_continuation",
                "previous_response_fresh_fallback_blocked",
                "compact_fresh_fallback_blocked",
                "compact_pressure_shed",
            ],
        ),
        (
            "persistence",
            &[
                "state_save_error",
                "state_save_queue_backpressure",
                "state_save_skipped",
                "continuation_journal_save_error",
                "continuation_journal_queue_backpressure",
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
                "profile_quota_quarantine",
                "profile_auth_backoff",
                "profile_probe_refresh_error",
                "profile_probe_refresh_panic",
                "profile_probe_refresh_backpressure",
                "selection_skip_sync_probe",
                "runtime_proxy_sync_probe_pressure_pause",
                "local_selection_blocked",
                "responses_pre_send_skip",
                "websocket_pre_send_skip",
                "quota_blocked",
                "quota_critical_floor_before_send",
                "upstream_usage_limit_passthrough",
                "upstream_overload_passthrough",
                "upstream_overloaded",
                "compact_retryable_failure",
                "compact_overload_conservative_retry",
                "compact_quota_unclassified",
            ],
        ),
        (
            "transport",
            &[
                "upstream_connect_timeout",
                "upstream_connect_dns_error",
                "upstream_tls_handshake_error",
                "upstream_connect_error",
                "upstream_connect_http",
                "upstream_close_before_completed",
                "upstream_connection_closed",
                "upstream_read_error",
                "upstream_send_error",
                "upstream_stream_error",
                "profile_transport_failure",
                "stream_read_error",
                "local_writer_error",
                "websocket_precommit_frame_timeout",
                "websocket_precommit_hold_timeout",
                "websocket_dns_resolve_timeout",
                "websocket_dns_overflow_enqueue",
                "websocket_dns_overflow_dispatch",
                "websocket_dns_overflow_reject",
                "websocket_connect_local_pressure",
                "websocket_connect_overflow_enqueue",
                "websocket_connect_overflow_dispatch",
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
        runtime_doctor_marker_last_usize_field(summary, "state_save_queue_backpressure", "backlog")
            .or_else(|| {
                runtime_doctor_marker_last_usize_field(summary, "state_save_queued", "backlog")
            });
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
        "continuation_journal_queue_backpressure",
        "backlog",
    )
    .or_else(|| {
        runtime_doctor_marker_last_usize_field(
            summary,
            "continuation_journal_save_queued",
            "backlog",
        )
    });
    summary.continuation_journal_save_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "continuation_journal_save_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(
                    summary,
                    "continuation_journal_save_error",
                    "lag_ms",
                )
            });
    summary.profile_probe_refresh_backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "profile_probe_refresh_backpressure",
        "backlog",
    )
    .or_else(|| {
        runtime_doctor_marker_last_usize_field(summary, "profile_probe_refresh_queued", "backlog")
    });
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

fn runtime_doctor_previous_response_continuation_label(
    summary: &RuntimeDoctorSummary,
    marker: &str,
) -> String {
    let request_shape = if marker == "previous_response_fresh_fallback_blocked"
        && runtime_doctor_has_context_dependent_fail_closed(summary)
    {
        Some("continuation_only".to_string())
    } else {
        runtime_doctor_marker_last_field(summary, marker, "request_shape")
            .map(str::to_string)
            .or_else(|| {
                summary
                    .facet_counts
                    .get("request_shape")
                    .and_then(|counts| counts.get("session_replayable"))
                    .map(|_| "session_replayable".to_string())
            })
    };
    match request_shape.as_deref() {
        Some("session_replayable") => "session-scoped previous_response_id continuation",
        Some("empty_input") => "empty-input previous_response_id continuation",
        Some("replayable_input") => "input-only previous_response_id continuation",
        Some("tool_output_only") => "tool-output previous_response_id continuation",
        Some("continuation_only") => "context-dependent previous_response_id continuation",
        _ => "previous_response_id continuation",
    }
    .to_string()
}

fn runtime_doctor_previous_response_fresh_fallback_blocked_shape_count(
    summary: &RuntimeDoctorSummary,
    request_shape: &str,
) -> usize {
    let counted = summary
        .previous_response_fresh_fallback_blocked_by_request_shape
        .get(request_shape)
        .copied()
        .unwrap_or(0);
    if counted > 0 {
        return counted;
    }
    if runtime_doctor_marker_count(summary, "previous_response_fresh_fallback_blocked") > 0
        && runtime_doctor_marker_last_field(
            summary,
            "previous_response_fresh_fallback_blocked",
            "request_shape",
        ) == Some(request_shape)
    {
        return 1;
    }
    0
}

fn runtime_doctor_has_context_dependent_fail_closed(summary: &RuntimeDoctorSummary) -> bool {
    runtime_doctor_previous_response_fresh_fallback_blocked_shape_count(
        summary,
        "continuation_only",
    ) > 0
}

fn runtime_doctor_compact_exit_counts(summary: &RuntimeDoctorSummary) -> BTreeMap<String, usize> {
    let compact_markers: [(&str, &[&str]); 10] = [
        (
            "candidate_exhausted",
            &[
                "compact_candidate_exhausted",
                "compact_exit_candidate_exhausted",
            ],
        ),
        (
            "committed",
            &["compact_committed", "compact_exit_committed"],
        ),
        (
            "committed_owner",
            &["compact_committed_owner", "compact_exit_committed_owner"],
        ),
        (
            "followup_owner",
            &["compact_followup_owner", "compact_exit_followup_owner"],
        ),
        (
            "lineage_released",
            &["compact_lineage_released", "compact_exit_lineage_released"],
        ),
        (
            "owner_retry",
            &[
                "compact_overload_conservative_retry",
                "compact_exit_overload_conservative_retry",
            ],
        ),
        (
            "precommit_budget",
            &[
                "compact_precommit_budget_exhausted",
                "compact_exit_precommit_budget_exhausted",
            ],
        ),
        (
            "pressure_shed",
            &["compact_pressure_shed", "compact_exit_pressure_shed"],
        ),
        (
            "quota_misc",
            &[
                "compact_quota_unclassified",
                "compact_exit_quota_unclassified",
            ],
        ),
        (
            "retryable_failure",
            &[
                "compact_retryable_failure",
                "compact_exit_retryable_failure",
            ],
        ),
    ];

    compact_markers
        .into_iter()
        .filter_map(|(label, markers)| {
            let count = markers
                .iter()
                .map(|marker| runtime_doctor_marker_count(summary, marker))
                .sum::<usize>();
            (count > 0).then(|| (label.to_string(), count))
        })
        .collect()
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
        || runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure") > 0
        || runtime_doctor_marker_count(summary, "selection_skip_sync_probe") > 0
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
        let lane =
            runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
                .unwrap_or("unknown");
        format!(
            "Recent per-lane admission limit was triggered on {lane}. Next step: {}",
            runtime_doctor_lane_pressure_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "runtime_proxy_active_limit_reached") > 0 {
        format!(
            "Recent global active-request admission limit was triggered. Next step: {}",
            runtime_doctor_active_pressure_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "runtime_proxy_queue_overloaded") > 0 {
        "Recent proxy saturation detected before commit.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_circuit_open") > 0 {
        "Recent route-level circuit breaker opened; fresh selection is temporarily steering away from a degraded profile.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_circuit_half_open_probe") > 0 {
        "Recent route-level circuit breaker entered half-open probing; fresh selection is cautiously testing a degraded profile before fully restoring it.".to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_precommit_frame_timeout") > 0 {
        "Recent websocket reuse/connect path failed to produce a first upstream frame before the pre-commit deadline.".to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_precommit_hold_timeout") > 0 {
        "Recent websocket pre-commit hold timed out before an upstream terminal frame arrived."
            .to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_dns_resolve_timeout") > 0 {
        "Recent websocket DNS resolution timed out before upstream connect completed.".to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_dns_overflow_reject") > 0 {
        "Recent websocket DNS resolution work was rejected after the overflow queue saturated."
            .to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_dns_overflow_enqueue") > 0
        || runtime_doctor_marker_count(summary, "websocket_dns_overflow_dispatch") > 0
    {
        "Recent websocket DNS resolution overflow queueing was observed.".to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_connect_local_pressure") > 0 {
        "Recent websocket connect failed due local pressure before upstream commit.".to_string()
    } else if runtime_doctor_marker_count(summary, "websocket_connect_overflow_rejected") > 0
        || runtime_doctor_marker_count(summary, "websocket_connect_overflow_reject") > 0
    {
        format!(
            "Recent websocket connect work was rejected after the overflow queue saturated. Next step: {}",
            runtime_doctor_websocket_connect_overflow_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "websocket_connect_overflow_enqueue") > 0 {
        format!(
            "Recent websocket connect overflow queueing was observed. Next step: {}",
            runtime_doctor_websocket_connect_overflow_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "websocket_connect_overflow_dispatch") > 0 {
        format!(
            "Recent websocket connect overflow dispatch was observed. Next step: {}",
            runtime_doctor_websocket_connect_overflow_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "profile_inflight_saturated") > 0 {
        let profile =
            runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "profile")
                .unwrap_or("an eligible profile");
        let hard_limit =
            runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "hard_limit")
                .map(|limit| format!(" at hard limit {limit}"))
                .unwrap_or_default();
        format!(
            "Recent per-profile in-flight saturation blocked {profile}{hard_limit}. Next step: {}",
            runtime_doctor_profile_inflight_saturated_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "profile_health") > 0 {
        let scope = runtime_doctor_marker_scope(summary, "profile_health", "profile", "route")
            .unwrap_or_else(|| "unknown route".to_string());
        let score =
            runtime_doctor_marker_last_field(summary, "profile_health", "score").unwrap_or("-");
        let reason = runtime_doctor_marker_last_field(summary, "profile_health", "reason")
            .unwrap_or("unknown_reason");
        format!(
            "Recent route-specific health penalty is steering fresh selection away from {scope} (score {score}, reason {reason}). Next step: {}",
            runtime_doctor_route_health_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "profile_bad_pairing") > 0 {
        "Recent route-specific bad pairing memory is steering fresh selection away from a flaky account.".to_string()
    } else if runtime_doctor_marker_count(summary, "profile_auth_recovery_failed") > 0 {
        format!(
            "Recent profile auth recovery failed after an upstream unauthorized response. Next step: {}",
            runtime_doctor_profile_auth_recovery_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "compact_fresh_fallback_blocked") > 0 {
        "Recent compact lineage guard failed closed so a follow-up stayed owner-first until upstream continuity was proven dead.".to_string()
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
            "Recent stale continuation was surfaced to Codex via fail-closed handling. Latest reason: {}.",
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
    } else if runtime_doctor_marker_count(summary, "previous_response_fresh_fallback_blocked") > 0 {
        let marker = "previous_response_fresh_fallback_blocked";
        let label = runtime_doctor_previous_response_continuation_label(summary, marker);
        let reason = runtime_doctor_marker_last_field(summary, marker, "reason")
            .unwrap_or("inspect previous_response_fresh_fallback_blocked markers");
        if runtime_doctor_has_context_dependent_fail_closed(summary) {
            format!(
                "Recent context-dependent previous_response_id continuation failed closed before commit. Fresh replay is disabled to preserve continuity. Latest reason: {reason}. Next step: {}",
                runtime_doctor_previous_response_fail_closed_next_step(summary)
            )
        } else {
            format!(
                "Recent {label} failed closed before commit. Fresh replay is disabled for stale continuation handling. Latest reason: {reason}. Next step: {}",
                runtime_doctor_previous_response_fail_closed_next_step(summary)
            )
        }
    } else if runtime_doctor_marker_count(summary, "previous_response_fresh_fallback") > 0 {
        let marker = "previous_response_fresh_fallback";
        let label = runtime_doctor_previous_response_continuation_label(summary, marker);
        let reason = runtime_doctor_marker_last_field(summary, marker, "reason")
            .unwrap_or("inspect previous_response_fresh_fallback markers");
        format!(
            "Legacy previous_response recovery marker was observed for {label}, but current runtime should fail closed instead of treating this as recoverable. Latest reason: {reason}. Restart active prodex/codex sessions if this came from a live broker."
        )
    } else if runtime_doctor_marker_count(summary, "previous_response_not_found") > 0 {
        format!(
            "Recent previous_response_id continuity failures were observed: {}.",
            runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_route)
        )
    } else if runtime_doctor_marker_count(summary, "compact_final_failure") > 0 {
        let exit = runtime_doctor_marker_last_field(summary, "compact_final_failure", "exit")
            .unwrap_or("-");
        let reason = runtime_doctor_marker_last_field(summary, "compact_final_failure", "reason")
            .unwrap_or("-");
        format!(
            "Recent compact final failure exited via {exit} with reason {reason}. Next step: {}",
            runtime_doctor_compact_final_failure_next_step(summary)
        )
    } else if !runtime_doctor_compact_exit_counts(summary).is_empty() {
        format!(
            "Recent compact exit paths were logged: {}.",
            runtime_doctor_count_breakdown(&runtime_doctor_compact_exit_counts(summary))
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
    } else if runtime_doctor_marker_count(summary, "profile_auth_recovered") > 0 {
        format!(
            "Recent profile auth recovered after an upstream unauthorized response. Next step: {}",
            runtime_doctor_profile_auth_recovery_next_step(summary)
        )
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
    } else if runtime_doctor_marker_count(summary, "state_save_queue_backpressure") > 0
        || runtime_doctor_marker_count(summary, "continuation_journal_queue_backpressure") > 0
    {
        format!(
            "Recent background persistence queue backpressure was detected. Next step: {}",
            runtime_doctor_persistence_backpressure_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "selection_skip_sync_probe") > 0 {
        let route = runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "route")
            .unwrap_or("unknown");
        format!(
            "Recent fresh selection skipped inline quota probing on route {route} under pressure mode. Next step: {}",
            runtime_doctor_sync_probe_skip_next_step(summary)
        )
    } else if runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure") > 0 {
        let profile = runtime_doctor_marker_last_field(
            summary,
            "profile_probe_refresh_backpressure",
            "profile",
        )
        .unwrap_or("unknown");
        let backlog = runtime_doctor_marker_last_usize_field(
            summary,
            "profile_probe_refresh_backpressure",
            "backlog",
        )
        .map(|backlog| format!(" with backlog {backlog}"))
        .unwrap_or_default();
        format!(
            "Recent background quota refresh queue backpressure was detected for profile {profile}{backlog}. Next step: {}",
            runtime_doctor_probe_refresh_backpressure_next_step(summary)
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_doctor_finalize_summary_uses_broker_artifact_diagnosis() {
        let mut summary = RuntimeDoctorSummary {
            pointer_exists: true,
            log_exists: true,
            line_count: 1,
            runtime_broker_identities: vec![
                "broker_key=broker-a pid=123 listen_addr=127.0.0.1:1234 status=dead_pid mismatch=none version=0.1.0 path=/opt/prodex sha256=abc123 source=registry stale_leases=2"
                    .to_string(),
            ],
            ..RuntimeDoctorSummary::default()
        };

        runtime_doctor_finalize_summary(&mut summary);

        assert!(
            summary.diagnosis.contains("broker-a") && summary.diagnosis.contains("dead pid 123")
        );
        assert!(summary.diagnosis.contains("prodex cleanup"));
    }
}
