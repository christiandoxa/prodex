use super::*;

use prodex_mojo_core::rich::*;

fn runtime_doctor_plan_marker_last_field<'a>(
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

fn runtime_doctor_plan_context_dependent(summary: &RuntimeDoctorSummary) -> bool {
    summary
        .previous_response_fresh_fallback_blocked_by_request_shape
        .get("continuation_only")
        .copied()
        .unwrap_or_default()
        > 0
        || summary
            .marker_counts
            .get("previous_response_fresh_fallback_blocked")
            .copied()
            .unwrap_or_default()
            > 0
            && runtime_doctor_plan_marker_last_field(
                summary,
                "previous_response_fresh_fallback_blocked",
                "request_shape",
            ) == Some("continuation_only")
}

fn runtime_doctor_plan_count(summary: &RuntimeDoctorSummary, marker: &'static str) -> i64 {
    summary
        .marker_counts
        .get(marker)
        .copied()
        .unwrap_or_default()
        .min(RUNTIME_DOCTOR_PLAN_MAX_COUNT as usize) as i64
}

fn runtime_doctor_plan_optional_field(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> i64 {
    runtime_doctor_plan_marker_last_field(summary, marker, field)
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as u64) as i64)
        .unwrap_or(-1)
}

fn runtime_doctor_plan_optional_summary_value(value: Option<usize>) -> i64 {
    value
        .map(|value| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as usize) as i64)
        .unwrap_or(-1)
}

fn runtime_doctor_plan_latest_marker(
    summary: &RuntimeDoctorSummary,
    markers: &[&'static str],
) -> Option<&'static str> {
    markers
        .iter()
        .copied()
        .find(|marker| runtime_doctor_plan_count(summary, marker) > 0)
}

fn runtime_doctor_plan_latest_field(
    summary: &RuntimeDoctorSummary,
    marker: Option<&'static str>,
    field: &str,
) -> i64 {
    marker
        .map(|marker| runtime_doctor_plan_optional_field(summary, marker, field))
        .unwrap_or(-1)
}

fn runtime_doctor_plan_lane(summary: &RuntimeDoctorSummary) -> i64 {
    match runtime_doctor_plan_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
    {
        Some("responses") => RUNTIME_DOCTOR_PLAN_LANE_RESPONSES,
        Some("compact") => RUNTIME_DOCTOR_PLAN_LANE_COMPACT,
        Some("websocket") => RUNTIME_DOCTOR_PLAN_LANE_WEBSOCKET,
        Some("standard") => RUNTIME_DOCTOR_PLAN_LANE_STANDARD,
        Some(_) => RUNTIME_DOCTOR_PLAN_LANE_OTHER,
        None => RUNTIME_DOCTOR_PLAN_LANE_MISSING,
    }
}

fn runtime_doctor_plan_compact_reason(summary: &RuntimeDoctorSummary) -> i64 {
    match runtime_doctor_plan_marker_last_field(summary, "compact_final_failure", "reason") {
        Some("quota") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_QUOTA,
        Some("overload") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_OVERLOAD,
        Some("transport") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_TRANSPORT,
        Some("inflight_saturation") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_INFLIGHT,
        _ => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_UNKNOWN,
    }
}

pub(crate) fn runtime_doctor_plan_input(
    summary: &RuntimeDoctorSummary,
    operation: i64,
) -> RuntimeDoctorPlanInput {
    let connect_marker = runtime_doctor_plan_latest_marker(
        summary,
        &[
            "websocket_connect_overflow_rejected",
            "websocket_connect_overflow_reject",
            "websocket_connect_overflow_enqueue",
            "websocket_connect_overflow_dispatch",
        ],
    );
    let dns_marker = runtime_doctor_plan_latest_marker(
        summary,
        &[
            "websocket_dns_overflow_reject",
            "websocket_dns_overflow_enqueue",
            "websocket_dns_overflow_dispatch",
        ],
    );
    let counts = RuntimeDoctorPlanMarkerCounts {
        lane: runtime_doctor_plan_count(summary, "runtime_proxy_lane_limit_reached"),
        active: runtime_doctor_plan_count(summary, "runtime_proxy_active_limit_reached"),
        profile_inflight: runtime_doctor_plan_count(summary, "profile_inflight_saturated"),
        profile_health: runtime_doctor_plan_count(summary, "profile_health"),
        websocket_rejected: runtime_doctor_plan_count(
            summary,
            "websocket_connect_overflow_rejected",
        ),
        websocket_reject: runtime_doctor_plan_count(summary, "websocket_connect_overflow_reject"),
        websocket_enqueue: runtime_doctor_plan_count(summary, "websocket_connect_overflow_enqueue"),
        websocket_dispatch: runtime_doctor_plan_count(
            summary,
            "websocket_connect_overflow_dispatch",
        ),
        auth_failed: runtime_doctor_plan_count(summary, "profile_auth_recovery_failed"),
        auth_recovered: runtime_doctor_plan_count(summary, "profile_auth_recovered"),
        state_backpressure: runtime_doctor_plan_count(summary, "state_save_queue_backpressure"),
        journal_backpressure: runtime_doctor_plan_count(
            summary,
            "continuation_journal_queue_backpressure",
        ),
        sync_probe_skip: runtime_doctor_plan_count(summary, "selection_skip_sync_probe"),
        probe_backpressure: runtime_doctor_plan_count(
            summary,
            "profile_probe_refresh_backpressure",
        ),
        transport_backoff: runtime_doctor_plan_count(summary, "profile_transport_backoff"),
        profile_transport_failure: runtime_doctor_plan_count(summary, "profile_transport_failure"),
        stream_read_error: runtime_doctor_plan_count(summary, "stream_read_error"),
        upstream_connect_timeout: runtime_doctor_plan_count(summary, "upstream_connect_timeout"),
        upstream_connect_error: runtime_doctor_plan_count(summary, "upstream_connect_error"),
        upstream_connect_dns_error: runtime_doctor_plan_count(
            summary,
            "upstream_connect_dns_error",
        ),
        upstream_tls_handshake_error: runtime_doctor_plan_count(
            summary,
            "upstream_tls_handshake_error",
        ),
        quota_blocked: runtime_doctor_plan_count(summary, "quota_blocked"),
        responses_pre_send_skip: runtime_doctor_plan_count(summary, "responses_pre_send_skip"),
        websocket_pre_send_skip: runtime_doctor_plan_count(summary, "websocket_pre_send_skip"),
        precommit_budget: runtime_doctor_plan_count(summary, "precommit_budget_exhausted"),
        compact_precommit_budget: runtime_doctor_plan_count(
            summary,
            "compact_precommit_budget_exhausted",
        ),
        compact_exit_precommit_budget: runtime_doctor_plan_count(
            summary,
            "compact_exit_precommit_budget_exhausted",
        ),
        compact_candidate: runtime_doctor_plan_count(summary, "compact_candidate_exhausted"),
        compact_exit_candidate: runtime_doctor_plan_count(
            summary,
            "compact_exit_candidate_exhausted",
        ),
        dns_reject: runtime_doctor_plan_count(summary, "websocket_dns_overflow_reject"),
        dns_enqueue: runtime_doctor_plan_count(summary, "websocket_dns_overflow_enqueue"),
        dns_dispatch: runtime_doctor_plan_count(summary, "websocket_dns_overflow_dispatch"),
    };
    let observations = RuntimeDoctorPlanObservations {
        lane_active: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_lane_limit_reached",
            "active",
        ),
        lane_limit: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_lane_limit_reached",
            "limit",
        ),
        active_active: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_active_limit_reached",
            "active",
        ),
        active_limit: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_active_limit_reached",
            "limit",
        ),
        inflight_hard_limit: runtime_doctor_plan_optional_field(
            summary,
            "profile_inflight_saturated",
            "hard_limit",
        ),
        websocket_pending: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "overflow_pending",
        ),
        websocket_max_pending: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "overflow_max_pending",
        ),
        websocket_worker_count: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "worker_count",
        ),
        websocket_queue_capacity: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "queue_capacity",
        ),
        dns_pending: runtime_doctor_plan_latest_field(summary, dns_marker, "overflow_pending"),
        dns_max_pending: runtime_doctor_plan_latest_field(
            summary,
            dns_marker,
            "overflow_max_pending",
        ),
        dns_worker_count: runtime_doctor_plan_latest_field(summary, dns_marker, "worker_count"),
        dns_queue_capacity: runtime_doctor_plan_latest_field(summary, dns_marker, "queue_capacity"),
        state_backlog: runtime_doctor_plan_optional_summary_value(summary.state_save_queue_backlog),
        journal_backlog: runtime_doctor_plan_optional_summary_value(
            summary.continuation_journal_save_backlog,
        ),
        probe_backlog: runtime_doctor_plan_optional_summary_value(
            summary.profile_probe_refresh_backlog,
        ),
        sync_cold_start_jobs: runtime_doctor_plan_optional_field(
            summary,
            "selection_skip_sync_probe",
            "cold_start_jobs",
        ),
        sync_cold_start_profiles: runtime_doctor_plan_optional_field(
            summary,
            "selection_skip_sync_probe",
            "cold_start_profiles",
        ),
    };
    RuntimeDoctorPlanInput {
        operation,
        lane: runtime_doctor_plan_lane(summary),
        compact_exit_pressure: i64::from(
            runtime_doctor_plan_marker_last_field(summary, "compact_final_failure", "exit")
                == Some("pressure"),
        ),
        compact_reason: runtime_doctor_plan_compact_reason(summary),
        quota_stale_risk: i64::from(summary.quota_freshness_pressure == "stale_risk"),
        context_dependent: i64::from(runtime_doctor_plan_context_dependent(summary)),
        counts,
        observations,
        tuning: RuntimeDoctorPlanTuning::default(),
    }
}

fn field<'a>(summary: &'a RuntimeDoctorSummary, marker: &str, name: &str) -> Option<&'a str> {
    summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| fields.get(name))
        .map(String::as_str)
}

fn tuning(snapshot: RuntimeDoctorTuningSnapshot) -> RuntimeDoctorPlanTuning {
    let bounded = |value: u64| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as u64) as i64;
    let bounded_usize = |value: usize| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as usize) as i64;
    RuntimeDoctorPlanTuning {
        active_request_limit: bounded_usize(snapshot.active_request_limit),
        responses_active_limit: bounded_usize(snapshot.lane_limits.responses),
        compact_active_limit: bounded_usize(snapshot.lane_limits.compact),
        websocket_active_limit: bounded_usize(snapshot.lane_limits.websocket),
        standard_active_limit: bounded_usize(snapshot.lane_limits.standard),
        admission_wait_budget_ms: bounded(snapshot.admission_wait_budget_ms),
        pressure_admission_wait_budget_ms: bounded(snapshot.pressure_admission_wait_budget_ms),
        websocket_connect_worker_count: bounded_usize(snapshot.websocket_connect_worker_count),
        websocket_connect_queue_capacity: bounded_usize(snapshot.websocket_connect_queue_capacity),
        websocket_connect_overflow_capacity: bounded_usize(
            snapshot.websocket_connect_overflow_capacity,
        ),
        websocket_dns_worker_count: bounded_usize(snapshot.websocket_dns_worker_count),
        websocket_dns_queue_capacity: bounded_usize(snapshot.websocket_dns_queue_capacity),
        websocket_dns_overflow_capacity: bounded_usize(snapshot.websocket_dns_overflow_capacity),
        profile_inflight_soft_limit: bounded_usize(snapshot.profile_inflight_soft_limit),
        profile_inflight_hard_limit: bounded_usize(snapshot.profile_inflight_hard_limit),
    }
}

const RENDER_POLICY_SUGGESTION_ID: i64 = 15;
const RENDER_POLICY_SUGGESTION_TITLE: i64 = 16;
const RENDER_POLICY_SUGGESTION_MARKERS: i64 = 17;
const RENDER_POLICY_SETTING_KEY: i64 = 18;
const RENDER_POLICY_SETTING_RATIONALE: i64 = 19;
const RENDER_POLICY_SUGGESTION_REASON: i64 = 20;
const RENDER_POLICY_MARKER_NAME: i64 = 21;

fn render_policy_text(operation: i64, detail: i64, values: &[Option<&str>]) -> String {
    prodex_mojo_core::rich::runtime_doctor_render(
        prodex_mojo_core::rich::RuntimeDoctorRenderInput {
            operation,
            detail,
            values,
        },
    )
    .expect("Mojo runtime-doctor policy renderer returned invalid output")
}

fn from_plan(
    summary: &RuntimeDoctorSummary,
    plan: &RuntimeDoctorPlan,
    index: usize,
) -> RuntimeDoctorPolicySuggestion {
    let id = plan.suggestion_ids[index];
    let severity = match plan.suggestion_severities[index] {
        RUNTIME_DOCTOR_PLAN_SEVERITY_LOW => "low",
        _ => "medium",
    };
    let lane = field(summary, "runtime_proxy_lane_limit_reached", "lane").unwrap_or("responses");
    let count_text = plan.suggestion_counts[index].to_string();
    let profile = field(summary, "profile_inflight_saturated", "profile").unwrap_or("unknown");
    let latest_marker_detail = plan.suggestion_markers[index]
        + if id == RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_DNS {
            100
        } else {
            0
        };
    let latest_marker = render_policy_text(RENDER_POLICY_MARKER_NAME, latest_marker_detail, &[]);
    let state_backpressure =
        runtime_doctor_plan_count(summary, "state_save_queue_backpressure").to_string();
    let journal_backpressure =
        runtime_doctor_plan_count(summary, "continuation_journal_queue_backpressure").to_string();
    let route_profile = field(summary, "profile_health", "profile").unwrap_or("unknown");
    let route = field(summary, "profile_health", "route").unwrap_or("unknown");
    let route_reason = field(summary, "profile_health", "reason").unwrap_or("unknown");
    let reason_values = [
        Some(count_text.as_str()),
        Some(lane),
        Some(profile),
        Some(latest_marker.as_str()),
        Some(state_backpressure.as_str()),
        Some(journal_backpressure.as_str()),
        Some(route_profile),
        Some(route),
        Some(route_reason),
    ];
    let reason = render_policy_text(RENDER_POLICY_SUGGESTION_REASON, id, &reason_values);
    let markers = render_policy_text(RENDER_POLICY_SUGGESTION_MARKERS, id, &[])
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();

    let setting_count = plan.suggestion_setting_counts[index] as usize;
    let settings = (0..setting_count)
        .map(|setting| {
            let flat = index * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS + setting;
            let key = plan.setting_keys[flat];
            RuntimeDoctorPolicySettingSuggestion {
                section: "runtime_proxy".to_string(),
                key: render_policy_text(RENDER_POLICY_SETTING_KEY, key, &[]),
                current_value: plan.setting_current_values[flat] as u64,
                suggested_value: plan.setting_suggested_values[flat] as u64,
                rationale: render_policy_text(
                    RENDER_POLICY_SETTING_RATIONALE,
                    id * 100 + key,
                    &[Some(lane)],
                ),
            }
        })
        .collect::<Vec<_>>();

    let mut snippet = vec!["[runtime_proxy]".to_string()];
    for setting in &settings {
        snippet.push(format!("{} = {}", setting.key, setting.suggested_value));
    }

    RuntimeDoctorPolicySuggestion {
        id: render_policy_text(RENDER_POLICY_SUGGESTION_ID, id, &[]),
        title: render_policy_text(RENDER_POLICY_SUGGESTION_TITLE, id, &[]),
        severity: severity.to_string(),
        reason,
        markers,
        settings,
        snippet: snippet.join("\n"),
    }
}

pub(super) fn policy_suggestions(
    summary: &RuntimeDoctorSummary,
    snapshot: RuntimeDoctorTuningSnapshot,
) -> Vec<RuntimeDoctorPolicySuggestion> {
    let mut input = runtime_doctor_plan_input(summary, RUNTIME_DOCTOR_PLAN_OP_POLICY_SUGGESTIONS);
    input.tuning = tuning(snapshot);
    let plan = runtime_doctor_plan(input)
        .expect("Mojo runtime-doctor policy plan returned invalid output");
    (0..plan.suggestion_count as usize)
        .map(|index| from_plan(summary, &plan, index))
        .collect()
}
