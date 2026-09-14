use crate::RuntimeDoctorSummary;
use crate::diagnosis::final_summary::runtime_doctor_has_context_dependent_fail_closed;
use crate::diagnosis::marker_accessors::*;
use crate::diagnosis::runtime_doctor_top_facet;
use crate::suggestions::runtime_doctor_plan_input;
use prodex_mojo_core::rich::*;

fn plan(summary: &RuntimeDoctorSummary, operation: i64) -> RuntimeDoctorPlan {
    runtime_doctor_plan(runtime_doctor_plan_input(summary, operation))
        .expect("Mojo runtime-doctor plan returned invalid output")
}

fn render(operation: i64, detail: i64, values: &[Option<&str>]) -> String {
    runtime_doctor_render(RuntimeDoctorRenderInput {
        operation,
        detail,
        values,
    })
    .expect("Mojo runtime-doctor renderer returned invalid output")
}

fn field<'a>(summary: &'a RuntimeDoctorSummary, marker: &str, name: &str) -> Option<&'a str> {
    runtime_doctor_marker_last_field(summary, marker, name)
}

fn selected_marker(marker: i64) -> &'static str {
    match marker {
        RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECTED => "websocket_connect_overflow_rejected",
        RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECT => "websocket_connect_overflow_reject",
        RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_ENQUEUE => "websocket_connect_overflow_enqueue",
        RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_DISPATCH => "websocket_connect_overflow_dispatch",
        RUNTIME_DOCTOR_PLAN_MARKER_AUTH_FAILED => "profile_auth_recovery_failed",
        RUNTIME_DOCTOR_PLAN_MARKER_AUTH_RECOVERED => "profile_auth_recovered",
        _ => "websocket_connect_overflow_dispatch",
    }
}

fn render_sync(summary: &RuntimeDoctorSummary, operation: i64, detail: i64) -> String {
    render(
        operation,
        detail,
        &[
            field(summary, "selection_skip_sync_probe", "route"),
            field(summary, "selection_skip_sync_probe", "reason"),
            field(summary, "selection_skip_sync_probe", "cold_start_jobs"),
            field(summary, "selection_skip_sync_probe", "cold_start_profiles"),
        ],
    )
}

fn render_probe(summary: &RuntimeDoctorSummary, operation: i64, detail: i64) -> String {
    let backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "profile_probe_refresh_backpressure",
        "backlog",
    )
    .or(summary.profile_probe_refresh_backlog)
    .map(|value| value.to_string());
    render(
        operation,
        detail,
        &[
            field(summary, "profile_probe_refresh_backpressure", "profile"),
            backlog.as_deref(),
        ],
    )
}

pub(super) fn previous_response(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PREVIOUS_RESPONSE);
    assert_eq!(
        value.detail == RUNTIME_DOCTOR_PLAN_NEXT_CONTEXT_DEPENDENT,
        runtime_doctor_has_context_dependent_fail_closed(summary),
        "Mojo runtime-doctor plan disagreed with continuation guard"
    );
    render(
        RUNTIME_DOCTOR_PLAN_OP_PREVIOUS_RESPONSE,
        value.detail,
        &[field(
            summary,
            "previous_response_fresh_fallback_blocked",
            "reason",
        )],
    )
}

pub(super) fn compact_final_failure(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_COMPACT_FINAL_FAILURE);
    render(
        RUNTIME_DOCTOR_PLAN_OP_COMPACT_FINAL_FAILURE,
        value.detail,
        &[
            field(summary, "compact_final_failure", "exit"),
            field(summary, "compact_final_failure", "profile"),
        ],
    )
}

pub(super) fn lane_pressure(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_LANE_PRESSURE);
    render(
        RUNTIME_DOCTOR_PLAN_OP_LANE_PRESSURE,
        value.detail,
        &[
            field(summary, "runtime_proxy_lane_limit_reached", "lane"),
            field(summary, "runtime_proxy_lane_limit_reached", "active"),
            field(summary, "runtime_proxy_lane_limit_reached", "limit"),
        ],
    )
}

pub(super) fn active_pressure(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_ACTIVE_PRESSURE);
    render(
        RUNTIME_DOCTOR_PLAN_OP_ACTIVE_PRESSURE,
        value.detail,
        &[
            field(summary, "runtime_proxy_active_limit_reached", "active"),
            field(summary, "runtime_proxy_active_limit_reached", "limit"),
        ],
    )
}

pub(super) fn profile_inflight(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PROFILE_INFLIGHT);
    render(
        RUNTIME_DOCTOR_PLAN_OP_PROFILE_INFLIGHT,
        value.detail,
        &[
            field(summary, "profile_inflight_saturated", "profile"),
            field(summary, "profile_inflight_saturated", "hard_limit"),
        ],
    )
}

pub(super) fn route_health(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_ROUTE_HEALTH);
    render(
        RUNTIME_DOCTOR_PLAN_OP_ROUTE_HEALTH,
        value.detail,
        &[
            field(summary, "profile_health", "profile"),
            field(summary, "profile_health", "route"),
            field(summary, "profile_health", "reason"),
        ],
    )
}

pub(super) fn websocket_connect(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_WEBSOCKET_CONNECT);
    let marker = selected_marker(value.selected_marker);
    render(
        RUNTIME_DOCTOR_PLAN_OP_WEBSOCKET_CONNECT,
        value.detail,
        &[
            field(summary, marker, "reason"),
            field(summary, marker, "overflow_pending"),
            field(summary, marker, "overflow_max_pending"),
            field(summary, marker, "worker_count"),
            field(summary, marker, "queue_capacity"),
        ],
    )
}

pub(super) fn profile_auth(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PROFILE_AUTH);
    let marker = selected_marker(value.selected_marker);
    render(
        RUNTIME_DOCTOR_PLAN_OP_PROFILE_AUTH,
        value.detail,
        &[
            field(summary, marker, "profile"),
            field(summary, marker, "route"),
            field(summary, marker, "error"),
            field(summary, marker, "source"),
            field(summary, marker, "changed"),
        ],
    )
}

pub(super) fn persistence_backpressure(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PERSISTENCE);
    let state_backlog = summary
        .state_save_queue_backlog
        .map(|value| value.to_string());
    let journal_backlog = summary
        .continuation_journal_save_backlog
        .map(|value| value.to_string());
    render(
        RUNTIME_DOCTOR_PLAN_OP_PERSISTENCE,
        value.detail,
        &[
            state_backlog.as_deref(),
            journal_backlog.as_deref(),
            field(summary, "state_save_queue_backpressure", "reason"),
            field(summary, "continuation_journal_queue_backpressure", "reason"),
        ],
    )
}

pub(super) fn sync_probe_skip(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_SYNC_PROBE);
    render_sync(summary, RUNTIME_DOCTOR_PLAN_OP_SYNC_PROBE, value.detail)
}

pub(super) fn probe_refresh_backpressure(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PROBE_REFRESH);
    render_probe(summary, RUNTIME_DOCTOR_PLAN_OP_PROBE_REFRESH, value.detail)
}

pub(super) fn transport_backoff(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_TRANSPORT);
    let mut values = Vec::with_capacity(15);
    for marker in [
        "profile_transport_backoff",
        "profile_transport_failure",
        "stream_read_error",
        "upstream_connect_timeout",
        "upstream_connect_error",
        "upstream_connect_dns_error",
        "upstream_tls_handshake_error",
    ] {
        values.push(field(summary, marker, "profile"));
        values.push(field(summary, marker, "route"));
    }
    let reason = runtime_doctor_top_facet(summary, "reason");
    values.push(reason.as_deref());
    render(RUNTIME_DOCTOR_PLAN_OP_TRANSPORT, value.detail, &values)
}

pub(super) fn quota_pressure(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_QUOTA);
    if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_SYNC {
        return render_sync(summary, RUNTIME_DOCTOR_PLAN_OP_QUOTA, value.detail);
    }
    if value.detail == RUNTIME_DOCTOR_PLAN_NEXT_QUOTA_PROBE {
        return render_probe(summary, RUNTIME_DOCTOR_PLAN_OP_QUOTA, value.detail);
    }
    render(
        RUNTIME_DOCTOR_PLAN_OP_QUOTA,
        value.detail,
        &[
            field(summary, "quota_blocked", "profile"),
            field(summary, "responses_pre_send_skip", "profile"),
            field(summary, "websocket_pre_send_skip", "profile"),
        ],
    )
}

pub(super) fn precommit_budget(summary: &RuntimeDoctorSummary) -> String {
    let value = plan(summary, RUNTIME_DOCTOR_PLAN_OP_PRECOMMIT);
    render(
        RUNTIME_DOCTOR_PLAN_OP_PRECOMMIT,
        value.detail,
        &[
            field(summary, "precommit_budget_exhausted", "route"),
            field(summary, "compact_precommit_budget_exhausted", "route"),
            field(summary, "compact_exit_precommit_budget_exhausted", "route"),
        ],
    )
}
