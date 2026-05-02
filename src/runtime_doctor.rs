use super::*;

mod state;

#[allow(unused_imports)]
pub(crate) use prodex_runtime_doctor::RuntimeDoctorRequestTimelineEvent;
pub(crate) use prodex_runtime_doctor::{
    RuntimeDoctorProfileSummary, RuntimeDoctorSummary, read_runtime_log_tail,
    runtime_doctor_fields_for_summary, runtime_doctor_json_value,
    runtime_doctor_policy_suggestion_lines, summarize_runtime_log_tail,
};
#[cfg(test)]
pub(crate) use prodex_runtime_doctor::RuntimeDoctorRouteSummary;
pub(crate) use state::collect_runtime_doctor_summary_with_tail_bytes;

#[cfg(test)]
pub(crate) use prodex_runtime_doctor::diagnosis::{
    runtime_doctor_finalize_summary, runtime_doctor_marker_count, runtime_doctor_top_facet,
};
#[cfg(test)]
pub(crate) use state::{collect_runtime_doctor_state, runtime_doctor_degraded_routes};

fn runtime_doctor_tuning_snapshot() -> prodex_runtime_doctor::RuntimeDoctorTuningSnapshot {
    let snapshot = collect_runtime_tuning_snapshot();
    prodex_runtime_doctor::RuntimeDoctorTuningSnapshot {
        active_request_limit: snapshot.active_request_limit,
        lane_limits: prodex_runtime_doctor::RuntimeDoctorTuningLaneLimits {
            responses: snapshot.lane_limits.responses,
            compact: snapshot.lane_limits.compact,
            websocket: snapshot.lane_limits.websocket,
            standard: snapshot.lane_limits.standard,
        },
        admission_wait_budget_ms: snapshot.admission_wait_budget_ms,
        pressure_admission_wait_budget_ms: snapshot.pressure_admission_wait_budget_ms,
        websocket_connect_worker_count: snapshot.websocket_connect_worker_count,
        websocket_connect_queue_capacity: snapshot.websocket_connect_queue_capacity,
        websocket_connect_overflow_capacity: snapshot.websocket_connect_overflow_capacity,
        websocket_dns_worker_count: snapshot.websocket_dns_worker_count,
        websocket_dns_queue_capacity: snapshot.websocket_dns_queue_capacity,
        websocket_dns_overflow_capacity: snapshot.websocket_dns_overflow_capacity,
        profile_inflight_soft_limit: snapshot.profile_inflight_soft_limit,
        profile_inflight_hard_limit: snapshot.profile_inflight_hard_limit,
    }
}

pub(crate) fn runtime_doctor_json_value_with_policy_suggestions(
    summary: &RuntimeDoctorSummary,
) -> serde_json::Value {
    prodex_runtime_doctor::runtime_doctor_json_value_with_policy_suggestions(
        summary,
        runtime_doctor_tuning_snapshot(),
    )
}

pub(crate) fn runtime_doctor_policy_suggestions(
    summary: &RuntimeDoctorSummary,
) -> Vec<prodex_runtime_doctor::RuntimeDoctorPolicySuggestion> {
    prodex_runtime_doctor::runtime_doctor_policy_suggestions(
        summary,
        runtime_doctor_tuning_snapshot(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_runtime_log_tail_understands_json_lines() {
        let tail = br#"{"timestamp":"2026-04-08 10:00:00.000 +00:00","message":"request=7 profile_health profile=main route=responses score=4","fields":{"request":"7","profile":"main","route":"responses","score":"4"}}"#;
        let summary = summarize_runtime_log_tail(tail);

        assert_eq!(summary.line_count, 1);
        assert_eq!(
            summary.marker_counts.get("profile_health").copied(),
            Some(1)
        );
        assert_eq!(
            summary.first_timestamp.as_deref(),
            Some("2026-04-08 10:00:00.000 +00:00")
        );
        assert_eq!(
            summary
                .marker_last_fields
                .get("profile_health")
                .and_then(|fields| fields.get("profile"))
                .map(String::as_str),
            Some("main")
        );
    }
}
