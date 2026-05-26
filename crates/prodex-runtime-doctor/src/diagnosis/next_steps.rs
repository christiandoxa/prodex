use super::final_summary::runtime_doctor_has_context_dependent_fail_closed;
use super::marker_accessors::*;
use super::*;

pub fn runtime_doctor_previous_response_fail_closed_next_step(
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

pub fn runtime_doctor_compact_final_failure_next_step(summary: &RuntimeDoctorSummary) -> String {
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
        (_, "transport") => format!(
            "Inspect compact transport markers{profile}; Prodex backed off the failing route, so retry after short transport backoff or let a fresh compact select another eligible profile."
        ),
        (_, "inflight_saturation") => format!(
            "Wait for in-flight compact work to drain{profile} before retrying."
        ),
        _ => format!(
            "Inspect compact exit markers around `{exit}`{profile} and retry after the blocking condition clears."
        ),
    }
}

pub fn runtime_doctor_lane_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
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

pub fn runtime_doctor_active_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    let load =
        runtime_doctor_admission_pressure_load(summary, "runtime_proxy_active_limit_reached");
    format!(
        "Reduce concurrent fresh work or wait for in-flight requests to drain before retrying.{load}"
    )
}

pub fn runtime_doctor_profile_inflight_saturated_next_step(
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

pub fn runtime_doctor_route_health_next_step(summary: &RuntimeDoctorSummary) -> String {
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

pub fn runtime_doctor_websocket_connect_overflow_next_step(
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

pub fn runtime_doctor_profile_auth_recovery_next_step(summary: &RuntimeDoctorSummary) -> String {
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

pub fn runtime_doctor_persistence_backpressure_next_step(summary: &RuntimeDoctorSummary) -> String {
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

pub fn runtime_doctor_sync_probe_skip_next_step(summary: &RuntimeDoctorSummary) -> String {
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

pub fn runtime_doctor_probe_refresh_backpressure_next_step(
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

pub fn runtime_doctor_transport_backoff_next_step(summary: &RuntimeDoctorSummary) -> String {
    let scope =
        runtime_doctor_marker_scope(summary, "profile_transport_backoff", "profile", "route")
            .or_else(|| {
                runtime_doctor_marker_scope(
                    summary,
                    "profile_transport_failure",
                    "profile",
                    "route",
                )
            })
            .or_else(|| {
                runtime_doctor_marker_scope(summary, "stream_read_error", "profile", "route")
            })
            .or_else(|| {
                runtime_doctor_marker_scope(summary, "upstream_connect_timeout", "profile", "route")
            })
            .or_else(|| {
                runtime_doctor_marker_scope(summary, "upstream_connect_error", "profile", "route")
            })
            .or_else(|| {
                runtime_doctor_marker_scope(
                    summary,
                    "upstream_connect_dns_error",
                    "profile",
                    "route",
                )
            })
            .or_else(|| {
                runtime_doctor_marker_scope(
                    summary,
                    "upstream_tls_handshake_error",
                    "profile",
                    "route",
                )
            })
            .unwrap_or_else(|| "affected route".to_string());
    let reason = runtime_doctor_top_facet(summary, "reason")
        .unwrap_or_else(|| "inspect latest transport marker".to_string());
    format!(
        "Inspect network/proxy and upstream transport markers for {scope}; wait for short transport backoff to expire before retrying fresh work. Top reason: {reason}."
    )
}

pub fn runtime_doctor_quota_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    if runtime_doctor_marker_count(summary, "selection_skip_sync_probe") > 0 {
        return runtime_doctor_sync_probe_skip_next_step(summary);
    }
    if runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure") > 0 {
        return runtime_doctor_probe_refresh_backpressure_next_step(summary);
    }
    let profile = runtime_doctor_marker_last_field(summary, "quota_blocked", "profile")
        .or_else(|| runtime_doctor_marker_last_field(summary, "responses_pre_send_skip", "profile"))
        .or_else(|| runtime_doctor_marker_last_field(summary, "websocket_pre_send_skip", "profile"))
        .map(|profile| format!(" for profile {profile}"))
        .unwrap_or_default();
    if summary.quota_freshness_pressure == "stale_risk" {
        return format!(
            "Refresh quota visibility with `prodex quota --all --once` and let background probes drain{profile} before retrying selection-heavy work."
        );
    }
    format!(
        "Wait for quota reset or use another eligible profile{profile}; verify current limits with `prodex quota --all --once`."
    )
}

pub fn runtime_doctor_precommit_budget_next_step(summary: &RuntimeDoctorSummary) -> String {
    let route = runtime_doctor_marker_last_field(summary, "precommit_budget_exhausted", "route")
        .or_else(|| {
            runtime_doctor_marker_last_field(summary, "compact_precommit_budget_exhausted", "route")
        })
        .or_else(|| {
            runtime_doctor_marker_last_field(
                summary,
                "compact_exit_precommit_budget_exhausted",
                "route",
            )
        })
        .unwrap_or("affected route");
    if runtime_doctor_marker_count(summary, "compact_precommit_budget_exhausted") > 0
        || runtime_doctor_marker_count(summary, "compact_exit_precommit_budget_exhausted") > 0
        || runtime_doctor_marker_count(summary, "compact_candidate_exhausted") > 0
        || runtime_doctor_marker_count(summary, "compact_exit_candidate_exhausted") > 0
    {
        return format!(
            "Reduce fresh compact volume on {route} or wait for quota/backoff pressure to clear before retrying compact."
        );
    }
    format!(
        "Inspect selection skip, quota, and transport backoff markers for {route}; retry after an eligible profile becomes available."
    )
}
