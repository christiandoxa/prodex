use super::*;

#[cfg(not(feature = "mojo"))]
mod rust_fallback {
    use super::final_summary::runtime_doctor_has_context_dependent_fail_closed;
    use super::marker_accessors::*;
    use super::*;

    pub(super) fn runtime_doctor_previous_response_fail_closed_next_step(
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
}

#[cfg(feature = "mojo")]
use super::final_summary::runtime_doctor_has_context_dependent_fail_closed;
#[cfg(feature = "mojo")]
use super::marker_accessors::*;
#[cfg(feature = "mojo")]
use crate::RuntimeDoctorSummary;
#[cfg(feature = "mojo")]
use crate::suggestions::runtime_doctor_plan_input;
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::*;

#[cfg(feature = "mojo")]
mod mojo_render {
    use super::*;

    fn plan(summary: &RuntimeDoctorSummary, operation: i64) -> RuntimeDoctorPlan {
        runtime_doctor_plan(runtime_doctor_plan_input(summary, operation))
            .expect("Mojo runtime-doctor plan returned invalid output")
    }

    fn field<'a>(summary: &'a RuntimeDoctorSummary, marker: &str, name: &str) -> &'a str {
        runtime_doctor_marker_last_field(summary, marker, name).unwrap_or("-")
    }

    fn selected_marker(marker: i64) -> &'static str {
        match marker {
            RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECTED => "websocket_connect_overflow_rejected",
            RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECT => "websocket_connect_overflow_reject",
            RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_ENQUEUE => "websocket_connect_overflow_enqueue",
            RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_DISPATCH => "websocket_connect_overflow_dispatch",
            RUNTIME_DOCTOR_PLAN_MARKER_AUTH_FAILED => "profile_auth_recovery_failed",
            RUNTIME_DOCTOR_PLAN_MARKER_AUTH_RECOVERED => "profile_auth_recovered",
            RUNTIME_DOCTOR_PLAN_MARKER_TRANSPORT_BACKOFF => "profile_transport_backoff",
            RUNTIME_DOCTOR_PLAN_MARKER_PROFILE_TRANSPORT_FAILURE => "profile_transport_failure",
            RUNTIME_DOCTOR_PLAN_MARKER_STREAM_READ_ERROR => "stream_read_error",
            RUNTIME_DOCTOR_PLAN_MARKER_CONNECT_TIMEOUT => "upstream_connect_timeout",
            RUNTIME_DOCTOR_PLAN_MARKER_CONNECT_ERROR => "upstream_connect_error",
            RUNTIME_DOCTOR_PLAN_MARKER_DNS_ERROR => "upstream_connect_dns_error",
            RUNTIME_DOCTOR_PLAN_MARKER_TLS_ERROR => "upstream_tls_handshake_error",
            _ => "websocket_connect_overflow_dispatch",
        }
    }

    fn quota_profile_marker(source: i64) -> Option<&'static str> {
        match source {
            RUNTIME_DOCTOR_PLAN_SOURCE_QUOTA => Some("quota_blocked"),
            RUNTIME_DOCTOR_PLAN_SOURCE_RESPONSES_SKIP => Some("responses_pre_send_skip"),
            RUNTIME_DOCTOR_PLAN_SOURCE_WEBSOCKET_SKIP => Some("websocket_pre_send_skip"),
            _ => None,
        }
    }

    fn profile_detail(summary: &RuntimeDoctorSummary, source: i64) -> String {
        quota_profile_marker(source)
            .and_then(|marker| runtime_doctor_marker_last_field(summary, marker, "profile"))
            .map(|profile| format!(" for profile {profile}"))
            .unwrap_or_default()
    }

    fn persistence(summary: &RuntimeDoctorSummary, selected_source: i64) -> String {
        let mut backlogs = Vec::new();
        if let Some(backlog) = summary.state_save_queue_backlog {
            backlogs.push(format!("state={backlog}"));
        }
        if let Some(backlog) = summary.continuation_journal_save_backlog {
            backlogs.push(format!("journal={backlog}"));
        }
        let backlog_detail = if backlogs.is_empty() {
            String::new()
        } else {
            format!(" Latest backlog: {}.", backlogs.join(" "))
        };
        let reason_detail = match selected_source {
            RUNTIME_DOCTOR_PLAN_SOURCE_STATE => {
                Some(field(summary, "state_save_queue_backpressure", "reason"))
            }
            RUNTIME_DOCTOR_PLAN_SOURCE_JOURNAL => Some(field(
                summary,
                "continuation_journal_queue_backpressure",
                "reason",
            )),
            _ => None,
        }
        .filter(|reason| *reason != "-")
        .map(|reason| format!(" Latest reason: {reason}."))
        .unwrap_or_default();
        format!(
            "Reduce rapid rotation or continuation churn and wait for background persistence queues to drain.{backlog_detail}{reason_detail}"
        )
    }

    fn sync_probe(summary: &RuntimeDoctorSummary, source: i64) -> String {
        let route = runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "route")
            .unwrap_or("unknown");
        let reason =
            runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "reason")
                .unwrap_or("unknown_reason");
        let deferred = match source {
            RUNTIME_DOCTOR_PLAN_SOURCE_JOBS => runtime_doctor_marker_last_field(
                summary,
                "selection_skip_sync_probe",
                "cold_start_jobs",
            )
            .map(|count| format!("{count} cold-start job(s)")),
            RUNTIME_DOCTOR_PLAN_SOURCE_PROFILES => runtime_doctor_marker_last_field(
                summary,
                "selection_skip_sync_probe",
                "cold_start_profiles",
            )
            .map(|count| format!("{count} cold-start profile(s)")),
            _ => None,
        }
        .unwrap_or_else(|| "cold-start work".to_string());
        format!(
            "Inspect `selection_skip_sync_probe`, `profile_probe_refresh_backpressure`, and `profile_probe_refresh_queued` markers for route {route}; pressure mode ({reason}) deferred {deferred}, so cold-start profiles may stay on stale quota data until background probes finish."
        )
    }

    fn probe_refresh(summary: &RuntimeDoctorSummary) -> String {
        let profile = runtime_doctor_marker_last_field(
            summary,
            "profile_probe_refresh_backpressure",
            "profile",
        );
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

    pub(super) fn previous_response(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PREVIOUS_RESPONSE);
        assert_eq!(
            value.detail == RUNTIME_DOCTOR_PLAN_NEXT_CONTEXT_DEPENDENT,
            runtime_doctor_has_context_dependent_fail_closed(summary),
            "Mojo runtime-doctor plan disagreed with continuation guard"
        );
        let reason = runtime_doctor_marker_last_field(
            summary,
            "previous_response_fresh_fallback_blocked",
            "reason",
        )
        .unwrap_or("unknown_reason");
        if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_CONTEXT_DEPENDENT {
            return format!(
                "Inspect `previous_response_not_found`, affinity bindings, and owning-profile chain markers before retrying; Prodex failed closed because this follow-up is context-dependent and cannot be replayed safely. Start a fresh turn only if context continuity can be abandoned. Latest guard: {reason}."
            );
        }
        format!(
            "Inspect `previous_response_not_found` and `chain_dead_upstream_confirmed` for the owning context before retrying; fail-closed stale continuation handling blocks fresh replay when continuity is unverified. Start a fresh turn instead of forcing rotation if the owner cannot be recovered. Latest guard: {reason}."
        )
    }

    pub(super) fn compact_final_failure(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_COMPACT_FINAL_FAILURE);
        let exit = field(summary, "compact_final_failure", "exit");
        let profile = runtime_doctor_marker_last_field(summary, "compact_final_failure", "profile")
            .map(|profile| format!(" on profile {profile}"))
            .unwrap_or_default();
        match value.detail {
            RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_PRESSURE => {
                "Reduce fresh compact volume or wait for continuation-heavy traffic to drain before retrying compact.".to_string()
            }
            RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_QUOTA => format!(
                "Inspect compact budget and candidate-exhausted markers{profile}, then retry after compact quota refreshes or another profile becomes eligible."
            ),
            RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_OVERLOAD => format!(
                "Inspect compact overload and backoff markers{profile}, then retry after the local pressure clears."
            ),
            RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_TRANSPORT => format!(
                "Inspect compact transport markers{profile}; Prodex backed off the failing route, so retry after short transport backoff or let a fresh compact select another eligible profile."
            ),
            RUNTIME_DOCTOR_PLAN_NEXT_COMPACT_INFLIGHT => format!(
                "Wait for in-flight compact work to drain{profile} before retrying."
            ),
            _ => format!(
                "Inspect compact exit markers around `{exit}`{profile} and retry after the blocking condition clears."
            ),
        }
    }

    pub(super) fn lane_pressure(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_LANE_PRESSURE);
        let lane =
            runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
                .unwrap_or("unknown");
        let load =
            runtime_doctor_admission_pressure_load(summary, "runtime_proxy_lane_limit_reached");
        if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_LANE_RESPONSES {
            format!(
                "Reduce concurrent terminals or bursty side-lane work until the responses lane drains.{load}"
            )
        } else {
            format!(
                "Inspect repeated lane={lane} markers and trim bursty {lane} traffic if it is starving responses.{load}"
            )
        }
    }

    pub(super) fn active_pressure(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_ACTIVE_PRESSURE);
        assert_eq!(
            value.detail, RUNTIME_DOCTOR_PLAN_NEXT_ACTIVE_PRESSURE,
            "Mojo runtime-doctor active-pressure plan returned wrong detail"
        );
        let load =
            runtime_doctor_admission_pressure_load(summary, "runtime_proxy_active_limit_reached");
        format!(
            "Reduce concurrent fresh work or wait for in-flight requests to drain before retrying.{load}"
        )
    }

    pub(super) fn profile_inflight(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PROFILE_INFLIGHT);
        let profile =
            runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "profile")
                .map(|profile| format!(" on profile {profile}"))
                .unwrap_or_default();
        if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_PROFILE_HARD_LIMIT {
            format!(
                "Wait for in-flight work{profile} to drop below hard limit {} before retrying, or let fresh selection land on another eligible profile.",
                field(summary, "profile_inflight_saturated", "hard_limit")
            )
        } else {
            format!(
                "Wait for in-flight work{profile} to drain before retrying, or let fresh selection land on another eligible profile."
            )
        }
    }

    pub(super) fn route_health(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_ROUTE_HEALTH);
        assert_eq!(
            value.detail, RUNTIME_DOCTOR_PLAN_NEXT_ROUTE_HEALTH,
            "Mojo runtime-doctor route-health plan returned wrong detail"
        );
        let scope = runtime_doctor_marker_scope(summary, "profile_health", "profile", "route")
            .unwrap_or_else(|| "that route".to_string());
        let reason = runtime_doctor_marker_last_field(summary, "profile_health", "reason")
            .unwrap_or("unknown_reason");
        format!(
            "Inspect recent transport or overload markers for {scope}, especially `{reason}`, and wait for that route score to decay before expecting fresh selection to reuse it."
        )
    }

    pub(super) fn websocket_connect(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_WEBSOCKET_CONNECT);
        let marker = selected_marker(value.selected_marker);
        let reason =
            runtime_doctor_marker_last_field(summary, marker, "reason").unwrap_or("unknown_reason");
        let pending = field(summary, marker, "overflow_pending");
        let max_pending = field(summary, marker, "overflow_max_pending");
        let worker_count = field(summary, marker, "worker_count");
        let queue_capacity = field(summary, marker, "queue_capacity");
        if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_WEBSOCKET_REJECTED {
            format!(
                "Reduce concurrent websocket session starts or wait for websocket connect workers to drain before retrying. Latest reason: {reason}; pending={pending}/{max_pending}, workers={worker_count}, queue_capacity={queue_capacity}."
            )
        } else if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_WEBSOCKET_DISPATCH {
            format!(
                "Overflow queued websocket connect work drained back into the bounded workers; inspect earlier enqueue/reject markers if dispatch repeats. Latest reason: {reason}; pending={pending}/{max_pending}, workers={worker_count}, queue_capacity={queue_capacity}."
            )
        } else {
            format!(
                "Watch for matching dispatch or rejected markers; repeated enqueue means websocket connect workers are saturated. Latest reason: {reason}; pending={pending}/{max_pending}, workers={worker_count}, queue_capacity={queue_capacity}."
            )
        }
    }

    pub(super) fn profile_auth(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PROFILE_AUTH);
        let marker = selected_marker(value.selected_marker);
        let profile = field(summary, marker, "profile");
        let route = field(summary, marker, "route");
        if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_AUTH_FAILED {
            format!(
                "Refresh credentials for profile {profile} with `prodex login --profile {profile}` and retry route {route}; latest recovery error: {}.",
                runtime_doctor_marker_last_field(summary, marker, "error")
                    .unwrap_or("unknown_error")
            )
        } else {
            format!(
                "Auth recovered for profile {profile} on route {route} via {} (changed={}); if this repeats, restart active sessions after login refresh.",
                field(summary, marker, "source"),
                field(summary, marker, "changed")
            )
        }
    }

    pub(super) fn persistence_backpressure(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PERSISTENCE);
        persistence(summary, value.selected_source)
    }

    pub(super) fn sync_probe_skip(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_SYNC_PROBE);
        sync_probe(summary, value.selected_source)
    }

    pub(super) fn probe_refresh_backpressure(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PROBE_REFRESH);
        assert_eq!(
            value.detail, RUNTIME_DOCTOR_PLAN_NEXT_PROBE_REFRESH,
            "Mojo runtime-doctor probe-refresh plan returned wrong detail"
        );
        probe_refresh(summary)
    }

    pub(super) fn transport_backoff(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_TRANSPORT);
        let marker = selected_marker(value.selected_marker);
        let scope = runtime_doctor_marker_scope(summary, marker, "profile", "route")
            .unwrap_or_else(|| "affected route".to_string());
        let reason = runtime_doctor_top_facet(summary, "reason")
            .unwrap_or_else(|| "inspect latest transport marker".to_string());
        format!(
            "Inspect network/proxy and upstream transport markers for {scope}; wait for short transport backoff to expire before retrying fresh work. Top reason: {reason}."
        )
    }

    pub(super) fn quota_pressure(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_QUOTA);
        let profile = profile_detail(summary, value.selected_source);
        match value.detail {
            RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_SYNC => sync_probe(summary, value.selected_source),
            RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_PROBE => probe_refresh(summary),
            RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_STALE => format!(
                "Refresh quota visibility with `prodex quota --all --once` and let background probes drain{profile} before retrying selection-heavy work."
            ),
            _ => format!(
                "Wait for quota reset or use another eligible profile{profile}; verify current limits with `prodex quota --all --once`."
            ),
        }
    }

    pub(super) fn precommit_budget(summary: &RuntimeDoctorSummary) -> String {
        let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PRECOMMIT);
        let route =
            runtime_doctor_marker_last_field(summary, "precommit_budget_exhausted", "route")
                .or_else(|| {
                    runtime_doctor_marker_last_field(
                        summary,
                        "compact_precommit_budget_exhausted",
                        "route",
                    )
                })
                .or_else(|| {
                    runtime_doctor_marker_last_field(
                        summary,
                        "compact_exit_precommit_budget_exhausted",
                        "route",
                    )
                })
                .unwrap_or("affected route");
        if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_PRECOMMIT_COMPACT {
            return format!(
                "Reduce fresh compact volume on {route} or wait for quota/backoff pressure to clear before retrying compact."
            );
        }
        format!(
            "Inspect selection skip, quota, and transport backoff markers for {route}; retry after an eligible profile becomes available."
        )
    }
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_previous_response_fail_closed_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::previous_response(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_previous_response_fail_closed_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    rust_fallback::runtime_doctor_previous_response_fail_closed_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_compact_final_failure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::compact_final_failure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_compact_final_failure_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_compact_final_failure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_lane_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::lane_pressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_lane_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_lane_pressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_active_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::active_pressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_active_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_active_pressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_profile_inflight_saturated_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::profile_inflight(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_profile_inflight_saturated_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    rust_fallback_rest::runtime_doctor_profile_inflight_saturated_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_route_health_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::route_health(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_route_health_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_route_health_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_websocket_connect_overflow_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::websocket_connect(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_websocket_connect_overflow_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    rust_fallback_rest::runtime_doctor_websocket_connect_overflow_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_profile_auth_recovery_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::profile_auth(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_profile_auth_recovery_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_profile_auth_recovery_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_persistence_backpressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::persistence_backpressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_persistence_backpressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_persistence_backpressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_sync_probe_skip_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::sync_probe_skip(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_sync_probe_skip_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_sync_probe_skip_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_probe_refresh_backpressure_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::probe_refresh_backpressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_probe_refresh_backpressure_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    rust_fallback_rest::runtime_doctor_probe_refresh_backpressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_transport_backoff_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::transport_backoff(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_transport_backoff_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_transport_backoff_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_quota_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::quota_pressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_quota_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_quota_pressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_precommit_budget_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::precommit_budget(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_precommit_budget_next_step(summary: &RuntimeDoctorSummary) -> String {
    rust_fallback_rest::runtime_doctor_precommit_budget_next_step(summary)
}

#[cfg(not(feature = "mojo"))]
mod rust_fallback_rest {
    use super::marker_accessors::*;
    use super::*;

    pub(super) fn runtime_doctor_compact_final_failure_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        let exit = runtime_doctor_marker_last_field(summary, "compact_final_failure", "exit")
            .unwrap_or("-");
        let reason = runtime_doctor_marker_last_field(summary, "compact_final_failure", "reason")
            .unwrap_or("-");
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

    pub(super) fn runtime_doctor_lane_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
        let lane =
            runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
                .unwrap_or("unknown");
        let load =
            runtime_doctor_admission_pressure_load(summary, "runtime_proxy_lane_limit_reached");
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

    pub(super) fn runtime_doctor_active_pressure_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        let load =
            runtime_doctor_admission_pressure_load(summary, "runtime_proxy_active_limit_reached");
        format!(
            "Reduce concurrent fresh work or wait for in-flight requests to drain before retrying.{load}"
        )
    }

    pub(super) fn runtime_doctor_profile_inflight_saturated_next_step(
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

    pub(super) fn runtime_doctor_route_health_next_step(summary: &RuntimeDoctorSummary) -> String {
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

    pub(super) fn runtime_doctor_websocket_connect_overflow_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        let marker = runtime_doctor_websocket_connect_overflow_marker(summary);
        let reason =
            runtime_doctor_marker_last_field(summary, marker, "reason").unwrap_or("unknown_reason");
        let pending =
            runtime_doctor_marker_last_field(summary, marker, "overflow_pending").unwrap_or("-");
        let max_pending = runtime_doctor_marker_last_field(summary, marker, "overflow_max_pending")
            .unwrap_or("-");
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

    pub(super) fn runtime_doctor_profile_auth_recovery_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        let marker = runtime_doctor_profile_auth_marker(summary);
        let profile = runtime_doctor_marker_last_field(summary, marker, "profile").unwrap_or("-");
        let route = runtime_doctor_marker_last_field(summary, marker, "route").unwrap_or("-");
        if marker == "profile_auth_recovery_failed" {
            let error = runtime_doctor_marker_last_field(summary, marker, "error")
                .unwrap_or("unknown_error");
            format!(
                "Refresh credentials for profile {profile} with `prodex login --profile {profile}` and retry route {route}; latest recovery error: {error}."
            )
        } else {
            let source = runtime_doctor_marker_last_field(summary, marker, "source").unwrap_or("-");
            let changed =
                runtime_doctor_marker_last_field(summary, marker, "changed").unwrap_or("-");
            format!(
                "Auth recovered for profile {profile} on route {route} via {source} (changed={changed}); if this repeats, restart active sessions after login refresh."
            )
        }
    }

    pub(super) fn runtime_doctor_persistence_backpressure_next_step(
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

    pub(super) fn runtime_doctor_sync_probe_skip_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        let route = runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "route")
            .unwrap_or("unknown");
        let reason =
            runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "reason")
                .unwrap_or("unknown_reason");
        let deferred = runtime_doctor_marker_last_field(
            summary,
            "selection_skip_sync_probe",
            "cold_start_jobs",
        )
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

    pub(super) fn runtime_doctor_probe_refresh_backpressure_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        let profile = runtime_doctor_marker_last_field(
            summary,
            "profile_probe_refresh_backpressure",
            "profile",
        );
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

    pub(super) fn runtime_doctor_transport_backoff_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
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
                    runtime_doctor_marker_scope(
                        summary,
                        "upstream_connect_timeout",
                        "profile",
                        "route",
                    )
                })
                .or_else(|| {
                    runtime_doctor_marker_scope(
                        summary,
                        "upstream_connect_error",
                        "profile",
                        "route",
                    )
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

    pub(super) fn runtime_doctor_quota_pressure_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        if runtime_doctor_marker_count(summary, "selection_skip_sync_probe") > 0 {
            return runtime_doctor_sync_probe_skip_next_step(summary);
        }
        if runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure") > 0 {
            return runtime_doctor_probe_refresh_backpressure_next_step(summary);
        }
        let profile = runtime_doctor_marker_last_field(summary, "quota_blocked", "profile")
            .or_else(|| {
                runtime_doctor_marker_last_field(summary, "responses_pre_send_skip", "profile")
            })
            .or_else(|| {
                runtime_doctor_marker_last_field(summary, "websocket_pre_send_skip", "profile")
            })
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

    pub(super) fn runtime_doctor_precommit_budget_next_step(
        summary: &RuntimeDoctorSummary,
    ) -> String {
        let route =
            runtime_doctor_marker_last_field(summary, "precommit_budget_exhausted", "route")
                .or_else(|| {
                    runtime_doctor_marker_last_field(
                        summary,
                        "compact_precommit_budget_exhausted",
                        "route",
                    )
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
}
