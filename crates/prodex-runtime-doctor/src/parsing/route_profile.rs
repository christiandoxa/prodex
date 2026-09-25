use std::collections::BTreeMap;

use crate::{RuntimeDoctorRouteProfileEvent, RuntimeDoctorSummary};

use super::request_timeline::runtime_doctor_request_timeline_detail;

const RUNTIME_DOCTOR_ROUTE_PROFILE_MAX_EVENTS: usize = 20;

fn runtime_doctor_route_profile_action(marker: &str) -> Option<&'static str> {
    let semantics = prodex_mojo_core::rich::runtime_doctor_marker_semantics(marker)
        .expect("Mojo runtime-doctor marker semantics returned invalid output");
    match semantics.route_action {
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_SELECTED => Some("selected"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_SELECTION_SKIP => {
            Some("selection_skip")
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_BLOCKED => Some("blocked"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_HEALTH => Some("health"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_HEALTH => {
            Some("transport_health")
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_FAILURE => {
            Some("transport_failure")
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_QUOTA => Some("quota"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_ROUTE_NONE => None,
        _ => unreachable!("validated Mojo route-profile action"),
    }
}

pub(super) fn runtime_doctor_record_route_profile_event(
    summary: &mut RuntimeDoctorSummary,
    timestamp: Option<&str>,
    marker: &str,
    fields: &BTreeMap<String, String>,
) {
    let Some(action) = runtime_doctor_route_profile_action(marker) else {
        return;
    };
    let profile = fields
        .get("profile")
        .filter(|value| !value.is_empty())
        .cloned();
    let route = fields
        .get("route")
        .filter(|value| !value.is_empty())
        .cloned();
    if profile.is_none() && route.is_none() {
        return;
    }
    summary
        .route_profile_events
        .push(RuntimeDoctorRouteProfileEvent {
            timestamp: timestamp.map(ToString::to_string),
            marker: marker.to_string(),
            action: action.to_string(),
            profile,
            route,
            detail: runtime_doctor_request_timeline_detail(fields),
        });
    if summary.route_profile_events.len() > RUNTIME_DOCTOR_ROUTE_PROFILE_MAX_EVENTS {
        summary.route_profile_events.remove(0);
    }
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod expected_route_profile_actions {
    use super::runtime_doctor_route_profile_action;

    #[test]
    fn route_profile_action_matches_fixed_cases() {
        for (action, markers) in [
            (
                "selected",
                &[
                    "selection_keep_affinity",
                    "selection_keep_current",
                    "selection_pick",
                ][..],
            ),
            (
                "selection_skip",
                &[
                    "selection_skip_current",
                    "selection_skip_affinity",
                    "selection_skip_sync_probe",
                ][..],
            ),
            (
                "blocked",
                &[
                    "local_selection_blocked",
                    "responses_pre_send_skip",
                    "websocket_pre_send_skip",
                    "quota_critical_floor_before_send",
                ][..],
            ),
            (
                "health",
                &["profile_health", "profile_latency", "profile_bad_pairing"][..],
            ),
            (
                "transport_health",
                &[
                    "profile_transport_backoff",
                    "profile_transport_failure",
                    "profile_circuit_open",
                    "profile_circuit_half_open_probe",
                ][..],
            ),
            (
                "transport_failure",
                &[
                    "stream_read_error",
                    "upstream_connect_timeout",
                    "upstream_connect_dns_error",
                    "upstream_tls_handshake_error",
                    "upstream_connect_error",
                    "upstream_connect_http",
                    "upstream_close_before_completed",
                    "upstream_connection_closed",
                    "upstream_read_error",
                    "upstream_send_error",
                    "upstream_stream_error",
                    "compact_transport_failure",
                    "websocket_precommit_frame_timeout",
                    "websocket_precommit_hold_timeout",
                ][..],
            ),
            (
                "quota",
                &[
                    "quota_blocked",
                    "profile_retry_backoff",
                    "profile_quota_quarantine",
                    "compact_retryable_failure",
                    "compact_quota_unclassified",
                ][..],
            ),
        ] {
            for marker in markers {
                assert_eq!(
                    runtime_doctor_route_profile_action(marker),
                    Some(action),
                    "{marker}"
                );
            }
        }
        for marker in ["selection_plan", "first_upstream_chunk", "unknown_marker"] {
            assert_eq!(
                runtime_doctor_route_profile_action(marker),
                None,
                "{marker}"
            );
        }
    }
}
