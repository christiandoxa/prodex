use crate::RuntimeDoctorSummary;
use crate::diagnosis::broker::runtime_doctor_broker_issue_diagnosis;
use crate::diagnosis::marker_accessors::*;
use crate::diagnosis::next_steps::*;
use prodex_mojo_core::rich::*;

const RUNTIME_DOCTOR_SUMMARY_MARKERS: &[&str] = &[
    "runtime_proxy_overload_backoff",
    "runtime_proxy_lane_limit_reached",
    "runtime_proxy_active_limit_reached",
    "runtime_proxy_queue_overloaded",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
    "websocket_precommit_frame_timeout",
    "websocket_precommit_hold_timeout",
    "websocket_dns_resolve_timeout",
    "websocket_dns_overflow_reject",
    "websocket_dns_overflow_enqueue",
    "websocket_dns_overflow_dispatch",
    "websocket_connect_local_pressure",
    "websocket_connect_overflow_reject",
    "websocket_connect_overflow_rejected",
    "websocket_connect_overflow_enqueue",
    "websocket_connect_overflow_dispatch",
    "websocket_proxy_tunnel_failure",
    "profile_inflight_saturated",
    "profile_health",
    "profile_bad_pairing",
    "profile_auth_recovery_failed",
    "local_rewrite_provider_auth_failure",
    "compact_fresh_fallback_blocked",
    "compact_pressure_shed",
    "chain_dead_upstream_confirmed",
    "stale_continuation",
    "chain_retried_owner",
    "previous_response_fresh_fallback_blocked",
    "previous_response_fresh_fallback",
    "previous_response_not_found",
    "compact_final_failure",
    "compat_warning",
    "websocket_reuse_watchdog",
    "profile_auth_recovered",
    "precommit_budget_exhausted",
    "upstream_usage_limit_passthrough",
    "responses_pre_send_skip",
    "websocket_pre_send_skip",
    "quota_critical_floor_before_send",
    "local_rewrite_gemini_quota_rotate",
    "local_rewrite_gemini_rate_limit_retry",
    "local_rewrite_provider_model_fallback",
    "local_rewrite_gemini_invalid_stream_retry",
    "local_rewrite_gemini_invalid_stream_model_fallback",
    "local_rewrite_gemini_compact_fallback",
    "local_rewrite_gemini_live_error",
    "local_rewrite_gemini_live_sidecar_error",
    "local_rewrite_gemini_live_sidecar_session_error",
    "stream_read_error",
    "local_writer_error",
    "upstream_connect_timeout",
    "upstream_tls_handshake_error",
    "upstream_connect_error",
    "state_save_error",
    "state_save_queue_backpressure",
    "continuation_journal_queue_backpressure",
    "selection_skip_sync_probe",
    "profile_probe_refresh_backpressure",
    "profile_probe_refresh_error",
    "profile_probe_refresh_start",
    "first_upstream_chunk",
    "first_local_chunk",
    "runtime_proxy_startup_audit",
    "compact_exit_candidate_exhausted",
    "compact_exit_committed",
    "compact_exit_committed_owner",
    "compact_exit_followup_owner",
    "compact_exit_lineage_released",
    "compact_exit_overload_conservative_retry",
    "compact_exit_precommit_budget_exhausted",
    "compact_exit_pressure_shed",
    "compact_exit_quota_unclassified",
    "compact_exit_retryable_failure",
    "compact_transport_failure",
    "compact_committed",
    "compact_committed_owner",
    "compact_followup_owner",
    "compact_lineage_released",
    "compact_overload_conservative_retry",
    "compact_precommit_budget_exhausted",
    "compact_pressure_shed",
    "compact_quota_unclassified",
    "compact_retryable_failure",
    "selection_keep_affinity",
    "selection_keep_current",
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "state_save_skipped",
    "profile_probe_refresh_ok",
    "upstream_connect_dns_error",
    "local_rewrite_gemini_live_sidecar_accept_error",
    "local_selection_blocked",
    "upstream_overload_passthrough",
    "upstream_overloaded",
    "upstream_read_error",
    "upstream_send_error",
    "upstream_stream_error",
    "profile_transport_backoff",
    "profile_transport_failure",
    "continuation_journal_save_error",
    "upstream_connect_http",
    "upstream_close_before_completed",
    "upstream_connection_closed",
    "compact_candidate_exhausted",
    "selection_plan",
    "profile_auth_proactive_sync_failed",
    "profile_quota_quarantine",
];

fn marker_count(summary: &RuntimeDoctorSummary, marker: &str) -> i64 {
    summary
        .marker_counts
        .get(marker)
        .copied()
        .unwrap_or_default()
        .min(RUNTIME_DOCTOR_PLAN_MAX_COUNT as usize) as i64
}

fn input(summary: &RuntimeDoctorSummary) -> RuntimeDoctorSummaryPlanInput {
    let mut marker_counts = [0_i64; RUNTIME_DOCTOR_SUMMARY_MARKER_COUNT];
    for (index, marker) in RUNTIME_DOCTOR_SUMMARY_MARKERS.iter().enumerate() {
        marker_counts[index] = marker_count(summary, marker);
    }
    let startup_audit_risk = summary
        .marker_last_fields
        .get("runtime_proxy_startup_audit")
        .is_some_and(|fields| {
            fields
                .get("missing_managed_dirs")
                .is_some_and(|value| value != "0")
                || fields
                    .get("orphan_managed_dirs")
                    .is_some_and(|value| value != "0")
        });
    let persisted_quota_snapshot_risk = super::runtime_doctor_top_facet(summary, "quota_source")
        .is_some_and(|value| value.starts_with("persisted_snapshot "));
    RuntimeDoctorSummaryPlanInput {
        marker_counts,
        line_count: marker_count_value(summary.line_count),
        pointer_exists: i64::from(summary.pointer_exists),
        log_exists: i64::from(summary.log_exists),
        stale_persisted_usage_snapshots: marker_count_value(
            summary.stale_persisted_usage_snapshots,
        ),
        orphan_managed_dirs: i64::from(!summary.orphan_managed_dirs.is_empty()),
        startup_audit_risk: i64::from(startup_audit_risk),
        persisted_dead_continuations: marker_count_value(summary.persisted_dead_continuations),
        suspect_continuations: i64::from(!summary.suspect_continuation_bindings.is_empty()),
        degraded_routes: i64::from(!summary.degraded_routes.is_empty()),
        runtime_broker_mismatch: i64::from(summary.runtime_broker_mismatch),
        prodex_binary_mismatch: i64::from(summary.prodex_binary_mismatch),
        persisted_quota_snapshot_risk: i64::from(persisted_quota_snapshot_risk),
    }
}

fn marker_count_value(value: usize) -> i64 {
    value.min(RUNTIME_DOCTOR_PLAN_MAX_COUNT as usize) as i64
}

pub(super) fn runtime_doctor_summary_plan(
    summary: &RuntimeDoctorSummary,
) -> RuntimeDoctorSummaryPlan {
    prodex_mojo_core::rich::runtime_doctor_summary_plan(input(summary))
        .expect("Mojo runtime-doctor summary plan returned invalid output")
}

pub(super) fn pressure_label(value: i64) -> &'static str {
    match value {
        RUNTIME_DOCTOR_PRESSURE_ELEVATED => "elevated",
        RUNTIME_DOCTOR_PRESSURE_ACTIVE => "active",
        RUNTIME_DOCTOR_PRESSURE_STALE_RISK => "stale_risk",
        _ => "low",
    }
}

fn field<'a>(summary: &'a RuntimeDoctorSummary, marker: &str, name: &str) -> &'a str {
    runtime_doctor_marker_last_field(summary, marker, name).unwrap_or("-")
}

pub(super) fn runtime_doctor_default_diagnosis(
    summary: &RuntimeDoctorSummary,
    plan: &RuntimeDoctorSummaryPlan,
) -> String {
    match plan.diagnosis_kind {
        RUNTIME_DOCTOR_DIAGNOSIS_NO_POINTER => {
            return "No runtime log pointer has been created yet.".to_string();
        }
        RUNTIME_DOCTOR_DIAGNOSIS_NO_LOG => {
            return "Latest runtime log path does not exist.".to_string();
        }
        RUNTIME_DOCTOR_DIAGNOSIS_EMPTY_LOG => return "Latest runtime log is empty.".to_string(),
        _ => {}
    }
    if let Some(diagnosis) = runtime_doctor_broker_issue_diagnosis(summary) {
        return diagnosis;
    }

    match plan.diagnosis_kind {
        RUNTIME_DOCTOR_DIAGNOSIS_PROXY_OVERLOAD_BACKOFF => {
            "Recent local proxy overload backoff was triggered.".to_string()
        }
        RUNTIME_DOCTOR_DIAGNOSIS_LANE_PRESSURE => format!(
            "Recent per-lane admission limit was triggered on {}. Next step: {}",
            field(summary, "runtime_proxy_lane_limit_reached", "lane"),
            runtime_doctor_lane_pressure_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_ACTIVE_PRESSURE => format!(
            "Recent global active-request admission limit was triggered. Next step: {}",
            runtime_doctor_active_pressure_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_QUEUE_OVERLOAD => {
            "Recent proxy saturation detected before commit.".to_string()
        }
        RUNTIME_DOCTOR_DIAGNOSIS_CIRCUIT_OPEN => "Recent route-level circuit breaker opened; fresh selection is temporarily steering away from a degraded profile.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_CIRCUIT_HALF_OPEN => "Recent route-level circuit breaker entered half-open probing; fresh selection is cautiously testing a degraded profile before fully restoring it.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_FRAME_TIMEOUT => "Recent websocket reuse/connect path failed to produce a first upstream frame before the pre-commit deadline.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_HOLD_TIMEOUT => "Recent websocket pre-commit hold timed out before an upstream terminal frame arrived.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_DNS_TIMEOUT => "Recent websocket DNS resolution timed out before upstream connect completed.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_DNS_REJECT => "Recent websocket DNS resolution work was rejected after the overflow queue saturated.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_DNS_OVERFLOW => "Recent websocket DNS resolution overflow queueing was observed.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_LOCAL_PRESSURE => "Recent websocket connect failed due local pressure before upstream commit.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_REJECT => format!(
            "Recent websocket connect work was rejected after the overflow queue saturated. Next step: {}",
            runtime_doctor_websocket_connect_overflow_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_ENQUEUE => format!(
            "Recent websocket connect overflow queueing was observed. Next step: {}",
            runtime_doctor_websocket_connect_overflow_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_DISPATCH => format!(
            "Recent websocket connect overflow dispatch was observed. Next step: {}",
            runtime_doctor_websocket_connect_overflow_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_TUNNEL_FAILURE => "Recent websocket upstream proxy tunnel failed before the upstream websocket handshake completed. Next step: inspect HTTPS_PROXY/NO_PROXY and the proxy's CONNECT support.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_INFLIGHT => {
            let profile = field(summary, "profile_inflight_saturated", "profile");
            let hard_limit = runtime_doctor_marker_last_field(
                summary,
                "profile_inflight_saturated",
                "hard_limit",
            )
            .map(|limit| format!(" at hard limit {limit}"))
            .unwrap_or_default();
            format!(
                "Recent per-profile in-flight saturation blocked {profile}{hard_limit}. Next step: {}",
                runtime_doctor_profile_inflight_saturated_next_step(summary)
            )
        }
        RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_HEALTH => format!(
            "Recent route-specific health penalty is steering fresh selection away from {} (score {}, reason {}). Next step: {}",
            runtime_doctor_marker_scope(summary, "profile_health", "profile", "route")
                .unwrap_or_else(|| "unknown route".to_string()),
            field(summary, "profile_health", "score"),
            runtime_doctor_marker_last_field(summary, "profile_health", "reason")
                .unwrap_or("unknown_reason"),
            runtime_doctor_route_health_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_BAD_PAIRING => "Recent route-specific bad pairing memory is steering fresh selection away from a flaky account.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_AUTH_FAILURE => format!(
            "Recent profile auth recovery failed after an upstream unauthorized response. Next step: {}",
            runtime_doctor_profile_auth_recovery_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_PROVIDER_AUTH_FAILURE => format!(
            "Recent {} provider auth failure was observed for profile {}; refresh that provider login or API key before retrying.",
            field(summary, "local_rewrite_provider_auth_failure", "provider"),
            field(summary, "local_rewrite_provider_auth_failure", "profile")
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_FALLBACK_BLOCKED => "Recent compact lineage guard failed closed so a follow-up stayed owner-first until upstream continuity was proven dead.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_PRESSURE_SHED => "Recent pressure mode is shedding fresh compact requests to preserve continuation-heavy traffic.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_CHAIN_DEAD => format!(
            "Recent previous_response_id chain was confirmed dead upstream after owner retries. Latest chain event: {}.",
            summary
                .latest_chain_event
                .clone()
                .unwrap_or_else(|| "inspect chain_dead_upstream_confirmed markers".to_string())
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_STALE_CONTINUATION => format!(
            "Recent stale continuation was surfaced to Codex via fail-closed handling. Latest reason: {}.",
            summary
                .latest_stale_continuation_reason
                .clone()
                .unwrap_or_else(|| "inspect stale_continuation markers".to_string())
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_CHAIN_RETRIED => format!(
            "Recent continuation chain was retried on the owning profile before commit. Latest chain event: {}.",
            summary
                .latest_chain_event
                .clone()
                .unwrap_or_else(|| "inspect chain_retried_owner markers".to_string())
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_BLOCKED => {
            let marker = "previous_response_fresh_fallback_blocked";
            let label = super::continuations::runtime_doctor_previous_response_continuation_label(
                summary, marker,
            );
            let reason = field(summary, marker, "reason");
            if super::continuations::runtime_doctor_has_context_dependent_fail_closed(summary) {
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
        }
        RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_FALLBACK => {
            let marker = "previous_response_fresh_fallback";
            let label = super::continuations::runtime_doctor_previous_response_continuation_label(
                summary, marker,
            );
            format!(
                "Legacy previous_response recovery marker was observed for {label}, but current runtime should fail closed instead of treating this as recoverable. Latest reason: {}. Restart active prodex/codex sessions if this came from a live broker.",
                field(summary, marker, "reason")
            )
        }
        RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_NOT_FOUND => format!(
            "Recent previous_response_id continuity failures were observed: {}.",
            runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_route)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_FINAL_FAILURE => format!(
            "Recent compact final failure exited via {} with reason {}. Next step: {}",
            field(summary, "compact_final_failure", "exit"),
            field(summary, "compact_final_failure", "reason"),
            runtime_doctor_compact_final_failure_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_EXIT_PATHS => {
            let counts = super::compact::runtime_doctor_compact_exit_counts(summary);
            if counts.is_empty() {
                "No recent overload or stream-failure markers were detected in the sampled runtime tail.".to_string()
            } else {
                format!(
                    "Recent compact exit paths were logged: {}.",
                    runtime_doctor_count_breakdown(&counts)
                )
            }
        }
        RUNTIME_DOCTOR_DIAGNOSIS_COMPAT_WARNING => format!(
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
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_DEAD_CONTINUATIONS => format!(
            "Some persisted continuations are currently dead and will be pruned: {}.",
            summary.persisted_dead_continuations
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_SUSPECT_CONTINUATIONS => format!(
            "Some persisted continuations are currently suspect: {}.",
            summary.suspect_continuation_bindings.join(", ")
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_WATCHDOG => "Recent websocket session reuse degraded before a terminal event; fresh reuse may be steering away from that profile.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_AUTH_RECOVERED => format!(
            "Recent profile auth recovered after an upstream unauthorized response. Next step: {}",
            runtime_doctor_profile_auth_recovery_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_PRECOMMIT_BUDGET => {
            "Recent candidate selection exhausted before commit.".to_string()
        }
        RUNTIME_DOCTOR_DIAGNOSIS_QUOTA_HARDENING => "Recent quota hardening skipped near-exhausted sends or passed through upstream usage-limit responses.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_QUOTA_RETRY => {
            let profile = runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_gemini_quota_rotate",
                "profile",
            )
            .or_else(|| {
                runtime_doctor_marker_last_field(
                    summary,
                    "local_rewrite_gemini_rate_limit_retry",
                    "profile",
                )
            })
            .unwrap_or("unknown");
            format!(
                "Recent Gemini quota or rate-limit recovery was observed for profile {profile} before commit; OAuth profile rotation/retry kept the Codex-facing request recoverable."
            )
        }
        RUNTIME_DOCTOR_DIAGNOSIS_PROVIDER_MODEL_FALLBACK => format!(
            "Recent {} model fallback was used before commit ({} -> {}).",
            field(summary, "local_rewrite_provider_model_fallback", "provider"),
            field(summary, "local_rewrite_provider_model_fallback", "from_model"),
            field(summary, "local_rewrite_provider_model_fallback", "to_model")
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_STREAM_RETRY => format!(
            "Recent Gemini stream produced an invalid pre-commit prefix ({}); Prodex retried or fell back before exposing it to Codex.",
            runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_gemini_invalid_stream_retry",
                "reason",
            )
            .or_else(|| {
                runtime_doctor_marker_last_field(
                    summary,
                    "local_rewrite_gemini_invalid_stream_model_fallback",
                    "reason",
                )
            })
            .unwrap_or("invalid_stream")
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_COMPACT_FALLBACK => format!(
            "Recent Gemini semantic compact failed before commit, so Prodex preserved continuity with the bounded local fallback. Latest reason: {}.",
            field(summary, "local_rewrite_gemini_compact_fallback", "reason")
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_LIVE_ERROR => "Recent Gemini Live bridge errors were observed; inspect local_rewrite_gemini_live_* markers for the failing profile/request.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_STREAM_READ => "Recent upstream stream read failure detected after commit.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_LOCAL_WRITER => "Recent local writer failure detected while forwarding an upstream stream.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_UPSTREAM_CONNECT => "Recent upstream connect failures detected.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_STATE_SAVE => "Recent runtime state save failures detected.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_PERSISTENCE => format!(
            "Recent background persistence queue backpressure was detected. Next step: {}",
            runtime_doctor_persistence_backpressure_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_SYNC_PROBE => format!(
            "Recent fresh selection skipped inline quota probing on route {} under pressure mode. Next step: {}",
            field(summary, "selection_skip_sync_probe", "route"),
            runtime_doctor_sync_probe_skip_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_PROBE_BACKPRESSURE => format!(
            "Recent background quota refresh queue backpressure was detected for profile {}{}. Next step: {}",
            field(summary, "profile_probe_refresh_backpressure", "profile"),
            runtime_doctor_marker_last_usize_field(
                summary,
                "profile_probe_refresh_backpressure",
                "backlog",
            )
            .map(|backlog| format!(" with backlog {backlog}"))
            .unwrap_or_default(),
            runtime_doctor_probe_refresh_backpressure_next_step(summary)
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_DEGRADED_ROUTES => format!(
            "Persisted degraded runtime routes are still active: {}",
            summary.degraded_routes.join(", ")
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_ORPHAN_DIRS => format!(
            "Orphan managed profile directories were detected: {}",
            summary.orphan_managed_dirs.join(", ")
        ),
        RUNTIME_DOCTOR_DIAGNOSIS_PROBE_ERROR => "Recent background quota refresh failures detected; fresh selection may rely on stale quota snapshots.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_PROBE_ACTIVITY => "Background quota refresh activity was detected; inspect the last marker for the most recent profile refresh.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_WRITER_STALL => "Likely writer stall: upstream produced data but the local writer did not emit a first chunk in the sampled tail.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_BROKER_MISMATCH => "A running runtime broker uses a different prodex binary than this command; restart active prodex/codex sessions so the patched runtime is loaded.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_BINARY_MISMATCH => "Multiple prodex binaries on PATH differ by version or hash; align installs so new sessions use the patched runtime.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_SELECTION => "Recent selection decisions were logged; inspect the last marker for why a profile was picked or skipped.".to_string(),
        RUNTIME_DOCTOR_DIAGNOSIS_NONE | RUNTIME_DOCTOR_DIAGNOSIS_NO_RECENT_FAILURE => "No recent overload or stream-failure markers were detected in the sampled runtime tail.".to_string(),
        _ => unreachable!("validated Mojo runtime-doctor diagnosis kind"),
    }
}
