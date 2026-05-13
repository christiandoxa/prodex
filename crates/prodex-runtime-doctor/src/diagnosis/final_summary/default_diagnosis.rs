use crate::RuntimeDoctorSummary;

use super::super::broker::runtime_doctor_broker_issue_diagnosis;
use super::super::marker_accessors::*;
use super::super::next_steps::*;
use super::compact::runtime_doctor_compact_exit_counts;
use super::continuations::{
    runtime_doctor_has_context_dependent_fail_closed,
    runtime_doctor_previous_response_continuation_label,
};

pub(super) fn runtime_doctor_default_diagnosis(summary: &RuntimeDoctorSummary) -> String {
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
    } else if runtime_doctor_marker_count(summary, "websocket_proxy_tunnel_failure") > 0 {
        "Recent websocket upstream proxy tunnel failed before the upstream websocket handshake completed. Next step: inspect HTTPS_PROXY/NO_PROXY and the proxy's CONNECT support.".to_string()
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
