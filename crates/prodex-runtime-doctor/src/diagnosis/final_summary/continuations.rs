use crate::RuntimeDoctorSummary;

use super::super::marker_accessors::*;

pub(super) fn runtime_doctor_previous_response_continuation_label(
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

pub(in crate::diagnosis) fn runtime_doctor_has_context_dependent_fail_closed(
    summary: &RuntimeDoctorSummary,
) -> bool {
    runtime_doctor_previous_response_fresh_fallback_blocked_shape_count(
        summary,
        "continuation_only",
    ) > 0
}
