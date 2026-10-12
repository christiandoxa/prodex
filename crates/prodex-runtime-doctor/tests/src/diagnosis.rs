use crate::diagnosis::runtime_doctor_finalize_summary;
use crate::{
    RuntimeDoctorProfileSummary, RuntimeDoctorRouteProfileEvent, RuntimeDoctorRouteSummary,
    RuntimeDoctorSummary,
};

#[test]
fn runtime_doctor_finalize_summary_uses_broker_artifact_diagnosis() {
    let mut summary = RuntimeDoctorSummary {
            pointer_exists: true,
            log_exists: true,
            line_count: 1,
            runtime_broker_identities: vec![
                "broker_key=broker-a pid=123 listen_addr=127.0.0.1:1234 status=dead_pid mismatch=none version=0.1.0 path=/opt/prodex sha256=abc123 source=registry stale_leases=2"
                    .to_string(),
            ],
            ..RuntimeDoctorSummary::default()
        };

    runtime_doctor_finalize_summary(&mut summary);

    assert!(summary.diagnosis.contains("broker-a") && summary.diagnosis.contains("dead pid 123"));
    assert!(summary.diagnosis.contains("prodex cleanup"));
}

#[test]
fn runtime_doctor_failure_diagnosis_overrides_routine_selection_markers() {
    for (marker, expected) in [
        (
            "precommit_budget_exhausted",
            "candidate selection exhausted before commit",
        ),
        ("stream_read_error", "stream read failure"),
        ("state_save_error", "state save failures"),
    ] {
        let mut summary = RuntimeDoctorSummary {
            pointer_exists: true,
            log_exists: true,
            line_count: 2,
            marker_counts: [("selection_pick", 1), (marker, 1)]
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
            ..RuntimeDoctorSummary::default()
        };

        runtime_doctor_finalize_summary(&mut summary);

        assert!(
            summary.diagnosis.to_ascii_lowercase().contains(expected),
            "{marker} should outrank routine selection: {}",
            summary.diagnosis
        );
    }
}

#[test]
fn runtime_doctor_diagnosis_preserves_missing_field_defaults() {
    for (marker, expected) in [
        (
            "runtime_proxy_lane_limit_reached",
            "Recent per-lane admission limit was triggered on unknown. Next step: Inspect repeated lane=unknown markers and trim bursty unknown traffic if it is starving responses.",
        ),
        (
            "compact_precommit_budget_exhausted",
            "Recent compact exit paths were logged: precommit_budget=1.",
        ),
        (
            "local_rewrite_provider_model_fallback",
            "Recent provider model fallback was used before commit (- -> -).",
        ),
        (
            "local_rewrite_gemini_compact_fallback",
            "Recent Gemini semantic compact failed before commit, so Prodex preserved continuity with the bounded local fallback. Latest reason: unknown.",
        ),
        (
            "local_rewrite_gemini_live_sidecar_accept_error",
            "No recent overload or stream-failure markers were detected in the sampled runtime tail.",
        ),
    ] {
        let mut summary = RuntimeDoctorSummary {
            pointer_exists: true,
            log_exists: true,
            line_count: 1,
            marker_counts: [(marker.to_string(), 1)].into(),
            ..RuntimeDoctorSummary::default()
        };
        runtime_doctor_finalize_summary(&mut summary);
        assert_eq!(summary.diagnosis, expected, "marker: {marker}");
    }
}

#[test]
fn runtime_doctor_diagnosis_uses_summary_warning_and_provider_fields() {
    let mut warning = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        compat_warning_count: 1,
        top_client: Some("client".to_string()),
        top_client_family: Some("family".to_string()),
        top_compat_warning: Some("warning".to_string()),
        ..RuntimeDoctorSummary::default()
    };
    runtime_doctor_finalize_summary(&mut warning);
    assert_eq!(
        warning.diagnosis,
        "Recent compatibility warnings were observed for client: warning."
    );

    let mut provider = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        marker_counts: [("local_rewrite_provider_model_fallback".to_string(), 1)].into(),
        marker_last_fields: [(
            "local_rewrite_provider_model_fallback".to_string(),
            [
                ("provider".to_string(), "gemini".to_string()),
                ("from_model".to_string(), "model-a".to_string()),
                ("to_model".to_string(), "model-b".to_string()),
            ]
            .into(),
        )]
        .into(),
        ..RuntimeDoctorSummary::default()
    };
    runtime_doctor_finalize_summary(&mut provider);
    assert_eq!(
        provider.diagnosis,
        "Recent gemini model fallback was used before commit (model-a -> model-b)."
    );
}

#[test]
fn runtime_doctor_diagnosis_aggregates_compact_exit_alias_counts_in_stable_order() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 4,
        marker_counts: [
            ("compact_candidate_exhausted".to_string(), 1),
            ("compact_exit_candidate_exhausted".to_string(), 1),
            ("compact_exit_precommit_budget_exhausted".to_string(), 1),
        ]
        .into(),
        ..RuntimeDoctorSummary::default()
    };

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent compact exit paths were logged: candidate_exhausted=2, precommit_budget=1."
    );
}

#[test]
fn runtime_doctor_route_health_caller_preserves_mojo_grouping_order_and_sentinels() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 7,
        route_profile_events: vec![
            RuntimeDoctorRouteProfileEvent {
                profile: Some("beta".to_string()),
                route: Some("responses".to_string()),
                marker: "profile_health".to_string(),
                action: "health".to_string(),
                detail: "reason=first".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
            RuntimeDoctorRouteProfileEvent {
                profile: Some("alpha".to_string()),
                route: Some("responses".to_string()),
                marker: "profile_health".to_string(),
                action: "health".to_string(),
                detail: "reason=first".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
            RuntimeDoctorRouteProfileEvent {
                profile: Some("beta".to_string()),
                route: Some("responses".to_string()),
                marker: "profile_transport_failure".to_string(),
                action: "transport_failure".to_string(),
                detail: "reason=latest".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
            RuntimeDoctorRouteProfileEvent {
                profile: Some("alpha".to_string()),
                route: Some("responses".to_string()),
                marker: "profile_health".to_string(),
                action: "health".to_string(),
                detail: "reason=latest".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
            RuntimeDoctorRouteProfileEvent {
                profile: Some("gamma".to_string()),
                route: Some("standard".to_string()),
                marker: "profile_health".to_string(),
                action: "health".to_string(),
                detail: "reason=only".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
            RuntimeDoctorRouteProfileEvent {
                route: Some("responses".to_string()),
                marker: "profile_health".to_string(),
                action: "health".to_string(),
                detail: "reason=missing_profile".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
            RuntimeDoctorRouteProfileEvent {
                profile: Some("omega".to_string()),
                marker: "profile_health".to_string(),
                action: "health".to_string(),
                detail: "reason=missing_route".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
        ],
        marker_last_fields: [(
            "profile_health".to_string(),
            [
                ("profile".to_string(), "alpha".to_string()),
                ("route".to_string(), "responses".to_string()),
                ("score".to_string(), "42".to_string()),
                ("reason".to_string(), "marker_reason".to_string()),
            ]
            .into(),
        )]
        .into(),
        profiles: vec![RuntimeDoctorProfileSummary {
            profile: "alpha".to_string(),
            routes: vec![
                RuntimeDoctorRouteSummary {
                    route: "responses".to_string(),
                    health_score: 7,
                    circuit_state: "closed".to_string(),
                    ..RuntimeDoctorRouteSummary::default()
                },
                RuntimeDoctorRouteSummary {
                    route: "compact".to_string(),
                    health_score: 3,
                    circuit_state: "half_open".to_string(),
                    ..RuntimeDoctorRouteSummary::default()
                },
            ],
            ..RuntimeDoctorProfileSummary::default()
        }],
        ..RuntimeDoctorSummary::default()
    };

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary
            .route_health
            .iter()
            .map(|route| (
                route.profile.as_str(),
                route.route.as_str(),
                route.event_count
            ))
            .collect::<Vec<_>>(),
        vec![
            ("alpha", "responses", 2),
            ("beta", "responses", 2),
            ("gamma", "standard", 1),
            ("omega", "unknown", 1),
            ("unknown", "responses", 1),
            ("alpha", "compact", 0),
        ]
    );
    assert_eq!(summary.route_health[0].health_score, Some(42));
    assert_eq!(
        summary.route_health[0].latest_reason.as_deref(),
        Some("latest")
    );
    assert_eq!(
        summary.route_health[0].health_reason.as_deref(),
        Some("marker_reason")
    );
    assert_eq!(
        summary.route_health[5].circuit_state.as_deref(),
        Some("half_open")
    );
}

#[test]
fn runtime_doctor_route_health_keeps_latest_reason_when_newer_event_has_none() {
    let mut summary = RuntimeDoctorSummary {
        route_profile_events: vec![
            RuntimeDoctorRouteProfileEvent {
                profile: Some("alpha".to_string()),
                route: Some("responses".to_string()),
                detail: "reason=transport_timeout".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
            RuntimeDoctorRouteProfileEvent {
                profile: Some("alpha".to_string()),
                route: Some("responses".to_string()),
                marker: "profile_transport_failure".to_string(),
                ..RuntimeDoctorRouteProfileEvent::default()
            },
        ],
        ..RuntimeDoctorSummary::default()
    };

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(summary.route_health.len(), 1);
    assert_eq!(summary.route_health[0].event_count, 2);
    assert_eq!(
        summary.route_health[0].latest_marker.as_deref(),
        Some("profile_transport_failure")
    );
    assert_eq!(
        summary.route_health[0].latest_reason.as_deref(),
        Some("transport_timeout")
    );
}
