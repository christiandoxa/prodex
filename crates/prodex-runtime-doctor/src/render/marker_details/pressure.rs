use super::*;

pub(super) fn active_limit(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    fields.push(
        "Active next step",
        diagnosis::runtime_doctor_active_pressure_next_step(summary),
    );
}

pub(super) fn lane_limit(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    fields.push(
        "Lane next step",
        diagnosis::runtime_doctor_lane_pressure_next_step(summary),
    );
}

pub(super) fn profile_inflight(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "profile_inflight_saturated";
    if diagnosis::runtime_doctor_marker_count(summary, marker) == 0 {
        return;
    }
    fields
        .push(
            "In-flight profile",
            marker_field(summary, marker, "profile"),
        )
        .push(
            "In-flight hard limit",
            marker_field(summary, marker, "hard_limit"),
        )
        .push(
            "In-flight next step",
            diagnosis::runtime_doctor_profile_inflight_saturated_next_step(summary),
        );
}

pub(super) fn overload_backoff(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    fields.push(
        "Connect failures",
        (diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_timeout")
            + diagnosis::runtime_doctor_marker_count(summary, "upstream_connect_error"))
        .to_string(),
    );
}

pub(super) fn websocket_overflow(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    let rejected =
        diagnosis::runtime_doctor_marker_count(summary, "websocket_connect_overflow_rejected") > 0;
    let reject =
        diagnosis::runtime_doctor_marker_count(summary, "websocket_connect_overflow_reject") > 0;
    let enqueue =
        diagnosis::runtime_doctor_marker_count(summary, "websocket_connect_overflow_enqueue") > 0;
    if marker != "websocket_connect_overflow_rejected"
        && (marker != "websocket_connect_overflow_reject" || rejected)
        && (marker != "websocket_connect_overflow_enqueue" || rejected || reject)
        && (marker != "websocket_connect_overflow_dispatch" || rejected || reject || enqueue)
    {
        return;
    }
    let pending = summary.marker_last_fields.get(marker).map_or_else(
        || "-".to_string(),
        |fields| {
            format!(
                "{}/{}",
                fields
                    .get("overflow_pending")
                    .map(String::as_str)
                    .unwrap_or("-"),
                fields
                    .get("overflow_max_pending")
                    .map(String::as_str)
                    .unwrap_or("-")
            )
        },
    );
    fields
        .push(
            "WS overflow reason",
            marker_field(summary, marker, "reason"),
        )
        .push("WS overflow pending", pending)
        .push(
            "WS overflow next step",
            diagnosis::runtime_doctor_websocket_connect_overflow_next_step(summary),
        );
}

pub(super) fn profile_health(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "profile_health";
    fields
        .push("Health route", marker_field(summary, marker, "route"))
        .push("Health profile", marker_field(summary, marker, "profile"))
        .push("Health score", marker_field(summary, marker, "score"))
        .push("Health reason", marker_field(summary, marker, "reason"))
        .push(
            "Health next step",
            diagnosis::runtime_doctor_route_health_next_step(summary),
        );
}
