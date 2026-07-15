use super::*;

pub(super) fn previous_not_found(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    fields
        .push(
            "Prev not found routes",
            diagnosis::runtime_doctor_count_breakdown(
                &summary.previous_response_not_found_by_route,
            ),
        )
        .push(
            "Prev not found xport",
            diagnosis::runtime_doctor_count_breakdown(
                &summary.previous_response_not_found_by_transport,
            ),
        );
}

pub(super) fn previous_fresh_fallback(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
) {
    let marker = "previous_response_fresh_fallback";
    fields
        .push(
            "Legacy fallback shape",
            marker_field(summary, marker, "request_shape"),
        )
        .push(
            "Legacy fallback reason",
            marker_field(summary, marker, "reason"),
        )
        .push(
            "Legacy fallback note",
            "Current runtime should fail closed; restart active prodex/codex sessions if this marker came from a live broker.",
        );
}

pub(super) fn previous_fallback_blocked(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
) {
    let marker = "previous_response_fresh_fallback_blocked";
    fields
        .push(
            "Continuation shape",
            marker_field(summary, marker, "request_shape"),
        )
        .push(
            "Continuation reason",
            marker_field(summary, marker, "reason"),
        )
        .push(
            "Fail-closed shapes",
            diagnosis::runtime_doctor_count_breakdown(
                &summary.previous_response_fresh_fallback_blocked_by_request_shape,
            ),
        )
        .push(
            "Continuation next step",
            diagnosis::runtime_doctor_previous_response_fail_closed_next_step(summary),
        );
}

pub(super) fn stale(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    fields
        .push(
            "Chain retry reasons",
            diagnosis::runtime_doctor_count_breakdown(&summary.chain_retried_owner_by_reason),
        )
        .push(
            "Chain dead reasons",
            diagnosis::runtime_doctor_count_breakdown(
                &summary.chain_dead_upstream_confirmed_by_reason,
            ),
        )
        .push(
            "Stale reasons",
            diagnosis::runtime_doctor_count_breakdown(&summary.stale_continuation_by_reason),
        )
        .push(
            "Latest stale reason",
            summary
                .latest_stale_continuation_reason
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        )
        .push(
            "Latest chain event",
            summary
                .latest_chain_event
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
}

pub(super) fn compact_final_failure(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "compact_final_failure";
    fields
        .push("Compact exit", marker_field(summary, marker, "exit"))
        .push("Compact reason", marker_field(summary, marker, "reason"))
        .push(
            "Compact last fail",
            marker_field(summary, marker, "last_failure"),
        )
        .push(
            "Compact next step",
            diagnosis::runtime_doctor_compact_final_failure_next_step(summary),
        );
}
