use std::collections::BTreeMap;

use crate::{RuntimeDoctorRouteProfileEvent, RuntimeDoctorSummary};

use super::request_timeline::runtime_doctor_request_timeline_detail;

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
    let action = runtime_doctor_route_profile_action(marker);
    let profile = fields
        .get("profile")
        .filter(|value| !value.is_empty())
        .cloned();
    let route = fields
        .get("route")
        .filter(|value| !value.is_empty())
        .cloned();
    let plan = prodex_mojo_core::rich::runtime_doctor_route_event_plan(
        action.is_some(),
        profile.is_some(),
        route.is_some(),
        summary.route_profile_events.len(),
    )
    .expect("compiled Mojo route-profile event retention plan returned invalid output");
    if !plan.accept {
        return;
    }
    let action = action.expect("Mojo accepted only known marker actions");
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
    if plan.evict_oldest {
        summary.route_profile_events.remove(0);
    }
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod event_retention_tests {
    use super::*;

    #[test]
    fn mojo_route_profile_retains_twenty_newest_rows_and_skips_empty_events() {
        let mut summary = RuntimeDoctorSummary::default();
        let empty = BTreeMap::<String, String>::new();
        runtime_doctor_record_route_profile_event(&mut summary, None, "selection_pick", &empty);
        runtime_doctor_record_route_profile_event(
            &mut summary,
            None,
            "unknown_marker",
            &BTreeMap::from([("profile".to_owned(), "alpha".to_owned())]),
        );
        assert!(summary.route_profile_events.is_empty());

        for index in 0..25 {
            let fields = BTreeMap::from([
                ("profile".to_owned(), format!("profile-東京-{index:02}")),
                ("route".to_owned(), "responses".to_owned()),
            ]);
            runtime_doctor_record_route_profile_event(
                &mut summary,
                None,
                "selection_pick",
                &fields,
            );
        }
        assert_eq!(summary.route_profile_events.len(), 20);
        assert_eq!(
            summary
                .route_profile_events
                .first()
                .unwrap()
                .profile
                .as_deref(),
            Some("profile-東京-05"),
        );
        assert_eq!(
            summary
                .route_profile_events
                .last()
                .unwrap()
                .profile
                .as_deref(),
            Some("profile-東京-24"),
        );

        // A valid action plus only a route is still a retained event.
        runtime_doctor_record_route_profile_event(
            &mut summary,
            Some("2026-10-16T12:00Z"),
            "profile_health",
            &BTreeMap::from([("route".to_owned(), "websocket".to_owned())]),
        );
        assert_eq!(summary.route_profile_events.len(), 20);
        assert_eq!(
            summary.route_profile_events[0].profile.as_deref(),
            Some("profile-東京-06")
        );
        assert_eq!(summary.route_profile_events.last().unwrap().profile, None);
        assert_eq!(
            summary
                .route_profile_events
                .last()
                .unwrap()
                .route
                .as_deref(),
            Some("websocket")
        );
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
