use crate::RuntimeDoctorSummary;
use crate::diagnosis::broker::runtime_doctor_broker_issue_diagnosis;
use crate::diagnosis::final_summary::compact::runtime_doctor_compact_exit_counts;
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
        marker_counts[index] = if *marker == "compat_warning" {
            marker_count_value(summary.compat_warning_count)
        } else {
            marker_count(summary, marker)
        };
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

pub(super) fn runtime_doctor_default_diagnosis(
    summary: &RuntimeDoctorSummary,
    plan: &RuntimeDoctorSummaryPlan,
) -> String {
    if matches!(
        plan.diagnosis_kind,
        RUNTIME_DOCTOR_DIAGNOSIS_NO_POINTER
            | RUNTIME_DOCTOR_DIAGNOSIS_NO_LOG
            | RUNTIME_DOCTOR_DIAGNOSIS_EMPTY_LOG
    ) {
        return render_diagnosis(plan.diagnosis_kind, &[]);
    }
    if let Some(diagnosis) = runtime_doctor_broker_issue_diagnosis(summary) {
        return diagnosis;
    }

    use std::borrow::Cow;
    let kind = plan.diagnosis_kind;
    let values: Vec<Option<Cow<'_, str>>> = match kind {
        RUNTIME_DOCTOR_DIAGNOSIS_LANE_PRESSURE => vec![
            runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
                .map(Cow::Borrowed),
            Some(Cow::Owned(runtime_doctor_lane_pressure_next_step(summary))),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_ACTIVE_PRESSURE => vec![Some(Cow::Owned(
            runtime_doctor_active_pressure_next_step(summary),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_REJECT
        | RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_ENQUEUE
        | RUNTIME_DOCTOR_DIAGNOSIS_WEBSOCKET_CONNECT_DISPATCH => vec![Some(Cow::Owned(
            runtime_doctor_websocket_connect_overflow_next_step(summary),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_INFLIGHT => vec![
            runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "profile")
                .map(Cow::Borrowed),
            runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "hard_limit")
                .map(Cow::Borrowed),
            Some(Cow::Owned(
                runtime_doctor_profile_inflight_saturated_next_step(summary),
            )),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_HEALTH => vec![
            Some(Cow::Owned(
                runtime_doctor_marker_scope(summary, "profile_health", "profile", "route")
                    .unwrap_or_else(|| "unknown route".to_string()),
            )),
            runtime_doctor_marker_last_field(summary, "profile_health", "score").map(Cow::Borrowed),
            runtime_doctor_marker_last_field(summary, "profile_health", "reason")
                .map(Cow::Borrowed),
            Some(Cow::Owned(runtime_doctor_route_health_next_step(summary))),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_PROFILE_AUTH_FAILURE => vec![Some(Cow::Owned(
            runtime_doctor_profile_auth_recovery_next_step(summary),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_PROVIDER_AUTH_FAILURE => vec![
            runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_provider_auth_failure",
                "provider",
            )
            .map(Cow::Borrowed),
            runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_provider_auth_failure",
                "profile",
            )
            .map(Cow::Borrowed),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_CHAIN_DEAD | RUNTIME_DOCTOR_DIAGNOSIS_CHAIN_RETRIED => {
            vec![summary.latest_chain_event.as_deref().map(Cow::Borrowed)]
        }
        RUNTIME_DOCTOR_DIAGNOSIS_STALE_CONTINUATION => vec![
            summary
                .latest_stale_continuation_reason
                .as_deref()
                .map(Cow::Borrowed),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_BLOCKED => {
            let marker = "previous_response_fresh_fallback_blocked";
            vec![
                Some(Cow::Owned(
                    super::continuations::runtime_doctor_previous_response_continuation_label(
                        summary, marker,
                    ),
                )),
                runtime_doctor_marker_last_field(summary, marker, "reason").map(Cow::Borrowed),
                super::continuations::runtime_doctor_has_context_dependent_fail_closed(summary)
                    .then_some(Cow::Borrowed("1")),
                Some(Cow::Owned(
                    runtime_doctor_previous_response_fail_closed_next_step(summary),
                )),
            ]
        }
        RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_FALLBACK => {
            let marker = "previous_response_fresh_fallback";
            vec![
                Some(Cow::Owned(
                    super::continuations::runtime_doctor_previous_response_continuation_label(
                        summary, marker,
                    ),
                )),
                runtime_doctor_marker_last_field(summary, marker, "reason").map(Cow::Borrowed),
            ]
        }
        RUNTIME_DOCTOR_DIAGNOSIS_PREVIOUS_RESPONSE_NOT_FOUND => vec![Some(Cow::Owned(
            runtime_doctor_count_breakdown(&summary.previous_response_not_found_by_route),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_FINAL_FAILURE => vec![
            runtime_doctor_marker_last_field(summary, "compact_final_failure", "exit")
                .map(Cow::Borrowed),
            runtime_doctor_marker_last_field(summary, "compact_final_failure", "reason")
                .map(Cow::Borrowed),
            Some(Cow::Owned(runtime_doctor_compact_final_failure_next_step(
                summary,
            ))),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_COMPACT_EXIT_PATHS => {
            let exits = runtime_doctor_compact_exit_counts(summary);
            vec![(!exits.is_empty()).then(|| Cow::Owned(runtime_doctor_count_breakdown(&exits)))]
        }
        RUNTIME_DOCTOR_DIAGNOSIS_COMPAT_WARNING => vec![
            summary
                .top_client
                .as_deref()
                .or(summary.top_client_family.as_deref())
                .map(Cow::Borrowed),
            summary.top_compat_warning.as_deref().map(Cow::Borrowed),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_DEAD_CONTINUATIONS => vec![Some(Cow::Owned(
            summary.persisted_dead_continuations.to_string(),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_SUSPECT_CONTINUATIONS => vec![Some(Cow::Owned(
            summary.suspect_continuation_bindings.join(", "),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_AUTH_RECOVERED => vec![Some(Cow::Owned(
            runtime_doctor_profile_auth_recovery_next_step(summary),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_QUOTA_RETRY => vec![
            runtime_doctor_marker_last_field(
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
            .map(Cow::Borrowed),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_PROVIDER_MODEL_FALLBACK => vec![
            runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_provider_model_fallback",
                "provider",
            )
            .map(Cow::Borrowed),
            runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_provider_model_fallback",
                "from_model",
            )
            .map(Cow::Borrowed),
            runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_provider_model_fallback",
                "to_model",
            )
            .map(Cow::Borrowed),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_STREAM_RETRY => vec![
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
            .map(Cow::Borrowed),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_GEMINI_COMPACT_FALLBACK => vec![
            runtime_doctor_marker_last_field(
                summary,
                "local_rewrite_gemini_compact_fallback",
                "reason",
            )
            .map(Cow::Borrowed),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_PERSISTENCE => vec![Some(Cow::Owned(
            runtime_doctor_persistence_backpressure_next_step(summary),
        ))],
        RUNTIME_DOCTOR_DIAGNOSIS_SYNC_PROBE => vec![
            runtime_doctor_marker_last_field(summary, "selection_skip_sync_probe", "route")
                .map(Cow::Borrowed),
            Some(Cow::Owned(runtime_doctor_sync_probe_skip_next_step(
                summary,
            ))),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_PROBE_BACKPRESSURE => vec![
            runtime_doctor_marker_last_field(
                summary,
                "profile_probe_refresh_backpressure",
                "profile",
            )
            .map(Cow::Borrowed),
            runtime_doctor_marker_last_usize_field(
                summary,
                "profile_probe_refresh_backpressure",
                "backlog",
            )
            .or(summary.profile_probe_refresh_backlog)
            .map(|value| Cow::Owned(value.to_string())),
            Some(Cow::Owned(
                runtime_doctor_probe_refresh_backpressure_next_step(summary),
            )),
        ],
        RUNTIME_DOCTOR_DIAGNOSIS_DEGRADED_ROUTES => {
            vec![Some(Cow::Owned(summary.degraded_routes.join(", ")))]
        }
        RUNTIME_DOCTOR_DIAGNOSIS_ORPHAN_DIRS => {
            vec![Some(Cow::Owned(summary.orphan_managed_dirs.join(", ")))]
        }
        _ => Vec::new(),
    };
    let views = values
        .iter()
        .map(|value| value.as_deref())
        .collect::<Vec<_>>();
    render_diagnosis(kind, &views)
}

fn render_diagnosis(kind: i64, values: &[Option<&str>]) -> String {
    prodex_mojo_core::rich::runtime_doctor_render(
        prodex_mojo_core::rich::RuntimeDoctorRenderInput {
            operation: 14,
            detail: kind,
            values,
        },
    )
    .expect("Mojo runtime-doctor diagnosis renderer returned invalid output")
}
