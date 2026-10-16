use std::collections::BTreeMap;

use crate::{RuntimeDoctorRequestTimelineEvent, RuntimeDoctorSummary};
use prodex_mojo_core::rich::{
    RUNTIME_DOCTOR_RENDER_REQUEST_TIMELINE_DETAIL, RuntimeDoctorRenderInput,
};

// ABI slots stay aligned with the field labels owned by the Mojo renderer.
const RUNTIME_DOCTOR_REQUEST_TIMELINE_DETAIL_FIELDS: [&str; 16] = [
    "profile",
    "route",
    "transport",
    "reason",
    "status",
    "code",
    "exit",
    "outcome",
    "quota_source",
    "affinity",
    "request_shape",
    "retry_index",
    "attempt",
    "response_id",
    "previous_response_id",
    "session_id",
];

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeDoctorRequestTimelineBuilder {
    last_index: usize,
    events: Vec<RuntimeDoctorRequestTimelineEvent>,
}

fn runtime_doctor_request_id(fields: &BTreeMap<String, String>) -> Option<String> {
    fields
        .get("request_id")
        .or_else(|| fields.get("request"))
        .filter(|request_id| !request_id.is_empty())
        .cloned()
}

fn runtime_doctor_request_timeline_phase(marker: &str) -> Option<&'static str> {
    let semantics = prodex_mojo_core::rich::runtime_doctor_marker_semantics(marker)
        .expect("Mojo runtime-doctor marker semantics returned invalid output");
    match semantics.timeline_phase {
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_PHASE_SELECTION => Some("selection"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_PHASE_PRE_SEND => Some("pre_send"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_PHASE_UPSTREAM => Some("upstream"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_PHASE_COMMIT => Some("commit"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_PHASE_FAIL => Some("fail"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_PHASE_NONE => None,
        _ => unreachable!("validated Mojo timeline phase"),
    }
}

pub(super) fn runtime_doctor_request_timeline_detail(fields: &BTreeMap<String, String>) -> String {
    let values = RUNTIME_DOCTOR_REQUEST_TIMELINE_DETAIL_FIELDS
        .map(|key| fields.get(key).map(String::as_str));
    prodex_mojo_core::rich::runtime_doctor_render(RuntimeDoctorRenderInput {
        operation: RUNTIME_DOCTOR_RENDER_REQUEST_TIMELINE_DETAIL,
        detail: 0,
        values: &values,
    })
    .expect("Mojo runtime-doctor timeline detail renderer returned invalid output")
}

pub(super) fn runtime_doctor_record_request_timeline_event(
    request_timelines: &mut BTreeMap<String, RuntimeDoctorRequestTimelineBuilder>,
    line_index: usize,
    timestamp: Option<&str>,
    marker: &str,
    fields: &BTreeMap<String, String>,
) {
    let Some(phase) = runtime_doctor_request_timeline_phase(marker) else {
        return;
    };
    let Some(request_id) = runtime_doctor_request_id(fields) else {
        return;
    };
    let builder = request_timelines.entry(request_id).or_default();
    builder.last_index = line_index;
    let evict_oldest =
        prodex_mojo_core::rich::runtime_doctor_timeline_evict_oldest(builder.events.len())
            .expect("Mojo timeline event retention plan returned invalid output");
    builder.events.push(RuntimeDoctorRequestTimelineEvent {
        timestamp: timestamp.map(ToString::to_string),
        phase: phase.to_string(),
        marker: marker.to_string(),
        detail: runtime_doctor_request_timeline_detail(fields),
    });
    if evict_oldest {
        builder.events.remove(0);
    }
}

pub(super) fn runtime_doctor_set_latest_request_timeline(
    summary: &mut RuntimeDoctorSummary,
    request_timelines: BTreeMap<String, RuntimeDoctorRequestTimelineBuilder>,
) {
    let ordered = request_timelines.into_iter().collect::<Vec<_>>();
    let last_indices = ordered
        .iter()
        .map(|(_, builder)| builder.last_index)
        .collect::<Vec<_>>();
    let Some(index) = prodex_mojo_core::rich::runtime_doctor_latest_timeline_index(&last_indices)
        .expect("Mojo latest request timeline plan returned invalid output")
    else {
        return;
    };
    let (request_id, builder) = ordered
        .into_iter()
        .nth(index)
        .expect("Mojo latest request timeline index is bounded");
    summary.latest_request_id = Some(request_id);
    summary.latest_request_timeline = builder.events;
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod expected_timeline_phases {
    use super::runtime_doctor_request_timeline_phase;

    #[test]
    fn timeline_phase_matches_fixed_cases() {
        for (phase, markers) in [
            (
                "selection",
                &[
                    "selection_keep_affinity",
                    "selection_keep_current",
                    "selection_plan",
                    "selection_pick",
                    "selection_skip_current",
                    "selection_skip_affinity",
                    "selection_skip_sync_probe",
                    "local_selection_blocked",
                ][..],
            ),
            (
                "pre_send",
                &[
                    "responses_pre_send_skip",
                    "websocket_pre_send_skip",
                    "quota_critical_floor_before_send",
                    "compact_pre_send_allow_quota_exhausted",
                ][..],
            ),
            (
                "upstream",
                &[
                    "upstream_connect_timeout",
                    "upstream_connect_dns_error",
                    "upstream_tls_handshake_error",
                    "upstream_connect_error",
                    "upstream_connect_http",
                    "upstream_overload_passthrough",
                    "upstream_overloaded",
                    "upstream_read_error",
                    "upstream_send_error",
                    "upstream_stream_error",
                    "upstream_close_before_completed",
                    "upstream_connection_closed",
                    "upstream_usage_limit_passthrough",
                    "first_upstream_chunk",
                ][..],
            ),
            (
                "commit",
                &[
                    "first_local_chunk",
                    "previous_response_owner",
                    "compact_committed",
                    "compact_committed_owner",
                    "compact_followup_owner",
                    "compact_exit_committed",
                    "compact_exit_committed_owner",
                    "compact_exit_followup_owner",
                ][..],
            ),
            (
                "fail",
                &[
                    "runtime_proxy_queue_overloaded",
                    "runtime_proxy_active_limit_reached",
                    "runtime_proxy_lane_limit_reached",
                    "runtime_proxy_overload_backoff",
                    "runtime_proxy_admission_wait_exhausted",
                    "runtime_proxy_queue_wait_exhausted",
                    "profile_inflight_saturated",
                    "precommit_budget_exhausted",
                    "profile_retry_backoff",
                    "profile_transport_backoff",
                    "profile_transport_failure",
                    "profile_circuit_open",
                    "profile_bad_pairing",
                    "profile_auth_recovery_failed",
                    "previous_response_not_found",
                    "previous_response_negative_cache",
                    "previous_response_fresh_fallback_blocked",
                    "compact_fresh_fallback_blocked",
                    "compact_pressure_shed",
                    "compact_precommit_budget_exhausted",
                    "compact_candidate_exhausted",
                    "compact_retryable_failure",
                    "compact_transport_failure",
                    "compact_final_failure",
                    "compact_exit_fresh_fallback_blocked",
                    "compact_exit_pressure_shed",
                    "compact_exit_precommit_budget_exhausted",
                    "compact_exit_candidate_exhausted",
                    "compact_exit_retryable_failure",
                    "websocket_precommit_frame_timeout",
                    "websocket_precommit_hold_timeout",
                    "websocket_dns_resolve_timeout",
                    "websocket_dns_overflow_reject",
                    "websocket_connect_overflow_reject",
                    "websocket_connect_overflow_rejected",
                    "stream_read_error",
                    "local_writer_error",
                    "chain_dead_upstream_confirmed",
                    "stale_continuation",
                    "quota_blocked",
                ][..],
            ),
        ] {
            for marker in markers {
                assert_eq!(
                    runtime_doctor_request_timeline_phase(marker),
                    Some(phase),
                    "{marker}"
                );
            }
        }
        for marker in [
            "selection_bucket",
            "first_upstream_header",
            "unknown_marker",
        ] {
            assert_eq!(
                runtime_doctor_request_timeline_phase(marker),
                None,
                "{marker}"
            );
        }
    }
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod timeline_detail_tests {
    use super::runtime_doctor_request_timeline_detail;
    use std::collections::BTreeMap;

    #[test]
    fn request_timeline_detail_preserves_field_order_cap_and_unicode() {
        let route = "🧭".repeat(49);
        let fields = BTreeMap::from([
            ("profile".to_string(), "東京".to_string()),
            ("route".to_string(), route),
            ("reason".to_string(), "timeout".to_string()),
            ("status".to_string(), "429".to_string()),
            ("code".to_string(), "ignored".to_string()),
            ("session_id".to_string(), "later".to_string()),
        ]);

        assert_eq!(
            runtime_doctor_request_timeline_detail(&fields),
            format!(
                "profile=東京 route={}... reason=timeout status=429 code=ignored",
                "🧭".repeat(45)
            )
        );
    }
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod timeline_retention_tests {
    use super::*;

    #[test]
    fn compiled_mojo_preserves_twelve_events_and_latest_lexical_tie() {
        let mut timelines = BTreeMap::new();
        for index in 0..15 {
            let fields = BTreeMap::from([
                ("request_id".to_owned(), "z-request".to_owned()),
                ("profile".to_owned(), format!("p{index:02}")),
            ]);
            runtime_doctor_record_request_timeline_event(
                &mut timelines,
                index,
                Some("2026-10-16T09:00Z"),
                "selection_pick",
                &fields,
            );
        }
        let z_events = &timelines["z-request"].events;
        assert_eq!(z_events.len(), 12);
        assert!(z_events[0].detail.contains("p03"), "{z_events:?}");
        assert!(z_events[11].detail.contains("p14"), "{z_events:?}");

        let a_fields = BTreeMap::from([
            ("request".to_owned(), "a-request".to_owned()),
            ("profile".to_owned(), "other".to_owned()),
        ]);
        runtime_doctor_record_request_timeline_event(
            &mut timelines,
            14,
            None,
            "selection_pick",
            &a_fields,
        );
        let mut summary = RuntimeDoctorSummary::default();
        runtime_doctor_set_latest_request_timeline(&mut summary, timelines.clone());
        assert_eq!(summary.latest_request_id.as_deref(), Some("z-request"));
        assert_eq!(summary.latest_request_timeline.len(), 12);

        runtime_doctor_record_request_timeline_event(
            &mut timelines,
            15,
            None,
            "selection_pick",
            &a_fields,
        );
        runtime_doctor_set_latest_request_timeline(&mut summary, timelines);
        assert_eq!(summary.latest_request_id.as_deref(), Some("a-request"));
        assert_eq!(summary.latest_request_timeline.len(), 2);
    }
}
