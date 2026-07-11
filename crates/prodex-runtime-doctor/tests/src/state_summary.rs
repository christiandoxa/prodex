use super::*;

fn test_config() -> RuntimeDoctorStateSummaryConfig {
    RuntimeDoctorStateSummaryConfig {
        health_decay_seconds: 10,
        bad_pairing_decay_seconds: 5,
        performance_decay_seconds: 20,
        usage_snapshot_stale_grace_seconds: 300,
    }
}

fn backoffs<'a>(
    retry_backoff_until: &'a BTreeMap<String, i64>,
    transport_backoff_until: &'a BTreeMap<String, i64>,
    route_circuit_open_until: &'a BTreeMap<String, i64>,
) -> RuntimeDoctorBackoffMaps<'a> {
    RuntimeDoctorBackoffMaps {
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
    }
}

fn ready_snapshot(checked_at: i64) -> RuntimeDoctorUsageSnapshot {
    RuntimeDoctorUsageSnapshot {
        checked_at,
        five_hour_status: RuntimeDoctorQuotaWindowStatus::Ready,
        five_hour_remaining_percent: 80,
        five_hour_reset_at: i64::MAX,
        weekly_status: RuntimeDoctorQuotaWindowStatus::Ready,
        weekly_remaining_percent: 90,
        weekly_reset_at: i64::MAX,
    }
}

#[derive(Debug, Clone)]
struct TestBinding {
    profile_name: String,
    bound_at: i64,
}

fn test_binding(profile_name: &str, bound_at: i64) -> TestBinding {
    TestBinding {
        profile_name: profile_name.to_string(),
        bound_at,
    }
}

#[test]
fn runtime_doctor_binding_state_summary_counts_sources_and_profiles() {
    let profile_names = vec!["alpha".to_string(), "beta".to_string()];
    let state_response = BTreeMap::from([("resp-state".to_string(), test_binding("alpha", 10))]);
    let state_session =
        BTreeMap::from([("session-state".to_string(), test_binding("missing", 12))]);
    let runtime_response = BTreeMap::from([
        ("resp-runtime".to_string(), test_binding("beta", 20)),
        ("resp-runtime-2".to_string(), test_binding("beta", 22)),
    ]);
    let runtime_turn_state =
        BTreeMap::from([("turn-runtime".to_string(), test_binding("alpha", 24))]);
    let journal_session_id =
        BTreeMap::from([("session-id-journal".to_string(), test_binding("beta", 30))]);
    let merged_response = BTreeMap::from([("resp-merged".to_string(), test_binding("alpha", 40))]);
    let empty = BTreeMap::new();

    let summary = runtime_doctor_binding_state_summary(
        RuntimeDoctorBindingStateInput {
            active_profile: Some("alpha"),
            profile_names: &profile_names,
            last_run_selected_profiles: 2,
            state: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &state_response,
                session_profile_bindings: &state_session,
                turn_state_bindings: &empty,
                session_id_bindings: &empty,
            },
            runtime_continuations: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &runtime_response,
                session_profile_bindings: &empty,
                turn_state_bindings: &runtime_turn_state,
                session_id_bindings: &empty,
            },
            continuation_journal: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &empty,
                session_profile_bindings: &empty,
                turn_state_bindings: &empty,
                session_id_bindings: &journal_session_id,
            },
            merged_continuations: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &merged_response,
                session_profile_bindings: &empty,
                turn_state_bindings: &empty,
                session_id_bindings: &empty,
            },
        },
        |binding| binding.profile_name.as_str(),
        |binding| binding.bound_at,
    );

    assert_eq!(summary.active_profile.as_deref(), Some("alpha"));
    assert_eq!(summary.profile_count, 2);
    assert_eq!(summary.last_run_selected_profiles, 2);
    assert_eq!(summary.state.response_bindings, 1);
    assert_eq!(summary.state.session_bindings, 1);
    assert_eq!(summary.state.total_bindings, 2);
    assert_eq!(summary.state.missing_profile_bindings, 1);
    assert_eq!(
        summary.state.missing_profile_binding_samples,
        vec!["session:session-state->missing".to_string()]
    );
    assert_eq!(summary.state.oldest_bound_at, Some(10));
    assert_eq!(summary.state.newest_bound_at, Some(12));
    assert_eq!(summary.runtime_continuations.response_bindings, 2);
    assert_eq!(summary.runtime_continuations.turn_state_bindings, 1);
    assert_eq!(
        summary.runtime_continuations.profiles[0],
        RuntimeDoctorBindingProfileSummary {
            profile: "beta".to_string(),
            response_bindings: 2,
            total_bindings: 2,
            ..RuntimeDoctorBindingProfileSummary::default()
        }
    );
    assert_eq!(summary.continuation_journal.session_id_bindings, 1);
    assert_eq!(summary.merged_continuations.response_bindings, 1);
}

#[test]
fn runtime_doctor_degraded_routes_formats_neutral_maps() {
    let now = 1_000;
    let mut retry = BTreeMap::new();
    retry.insert("gamma".to_string(), 1_007);
    let mut transport = BTreeMap::new();
    transport.insert(
        "__route_transport_backoff__:websocket:alpha".to_string(),
        1_010,
    );
    transport.insert("beta".to_string(), 1_005);
    let mut circuits = BTreeMap::new();
    circuits.insert("__route_circuit__:responses:alpha".to_string(), 1_001);
    circuits.insert("__route_circuit__:compact:beta".to_string(), 999);
    let mut scores = BTreeMap::new();
    scores.insert(
        "__route_health__:responses:alpha".to_string(),
        RuntimeDoctorHealthScore {
            score: 5,
            updated_at: 980,
        },
    );
    scores.insert(
        "__route_bad_pairing__:compact:beta".to_string(),
        RuntimeDoctorHealthScore {
            score: 4,
            updated_at: 995,
        },
    );
    scores.insert(
        "__route_health__:standard:ignored".to_string(),
        RuntimeDoctorHealthScore {
            score: 1,
            updated_at: 900,
        },
    );

    let routes = runtime_doctor_degraded_routes(
        backoffs(&retry, &transport, &circuits),
        &scores,
        now,
        test_config(),
    );

    assert_eq!(
        routes,
        vec![
            "alpha/responses circuit=open until=1001",
            "alpha/responses health=3",
            "alpha/websocket transport_backoff until=1010",
            "beta/compact bad_pairing=3",
            "beta/compact circuit=half-open until=999",
            "beta/transport transport_backoff until=1005",
            "gamma/retry retry_backoff until=1007",
        ]
    );
}

#[test]
fn runtime_doctor_quota_freshness_label_preserves_hold_rules() {
    let now = 1_000;
    assert_eq!(
        runtime_doctor_quota_freshness_label(None, now, 300),
        "missing"
    );
    assert_eq!(
        runtime_doctor_quota_freshness_label(Some(&ready_snapshot(900)), now, 300),
        "fresh"
    );
    assert_eq!(
        runtime_doctor_quota_freshness_label(Some(&ready_snapshot(600)), now, 300),
        "stale"
    );

    let active_hold = RuntimeDoctorUsageSnapshot {
        checked_at: 0,
        five_hour_status: RuntimeDoctorQuotaWindowStatus::Exhausted,
        five_hour_remaining_percent: 0,
        five_hour_reset_at: 1_100,
        ..ready_snapshot(0)
    };
    assert_eq!(
        runtime_doctor_quota_freshness_label(Some(&active_hold), now, 300),
        "fresh"
    );

    let expired_hold = RuntimeDoctorUsageSnapshot {
        checked_at: 999,
        five_hour_status: RuntimeDoctorQuotaWindowStatus::Exhausted,
        five_hour_remaining_percent: 0,
        five_hour_reset_at: 999,
        ..ready_snapshot(999)
    };
    assert_eq!(
        runtime_doctor_quota_freshness_label(Some(&expired_hold), now, 300),
        "stale"
    );
}

#[test]
fn runtime_doctor_route_circuit_state_labels_all_states() {
    assert_eq!(
        runtime_doctor_route_circuit_state(Some(1_001), 1_000),
        "open"
    );
    assert_eq!(
        runtime_doctor_route_circuit_state(Some(1_000), 1_000),
        "half_open"
    );
    assert_eq!(runtime_doctor_route_circuit_state(None, 1_000), "closed");
}

#[test]
fn runtime_doctor_profile_summaries_build_route_rows() {
    let now = 1_000;
    let profile_names = vec!["alpha".to_string(), "beta".to_string()];
    let mut snapshots = BTreeMap::new();
    snapshots.insert(
        "alpha".to_string(),
        RuntimeDoctorUsageSnapshot {
            checked_at: 900,
            five_hour_status: RuntimeDoctorQuotaWindowStatus::Thin,
            five_hour_remaining_percent: 12,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeDoctorQuotaWindowStatus::Ready,
            weekly_remaining_percent: 90,
            weekly_reset_at: i64::MAX,
        },
    );
    let mut retry = BTreeMap::new();
    retry.insert("alpha".to_string(), 1_020);
    let mut transport = BTreeMap::new();
    transport.insert("alpha".to_string(), 1_030);
    transport.insert(
        "__route_transport_backoff__:responses:alpha".to_string(),
        1_040,
    );
    let mut circuits = BTreeMap::new();
    circuits.insert("__route_circuit__:responses:alpha".to_string(), 1_050);
    let mut scores = BTreeMap::new();
    scores.insert(
        "__route_health__:responses:alpha".to_string(),
        RuntimeDoctorHealthScore {
            score: 6,
            updated_at: 980,
        },
    );
    scores.insert(
        "__route_bad_pairing__:responses:alpha".to_string(),
        RuntimeDoctorHealthScore {
            score: 4,
            updated_at: 995,
        },
    );
    scores.insert(
        "__route_performance__:responses:alpha".to_string(),
        RuntimeDoctorHealthScore {
            score: 7,
            updated_at: 960,
        },
    );

    let profiles = runtime_doctor_profile_summaries(
        &profile_names,
        &snapshots,
        &scores,
        backoffs(&retry, &transport, &circuits),
        now,
        test_config(),
    );

    let alpha = &profiles[0];
    assert_eq!(alpha.profile, "alpha");
    assert_eq!(alpha.quota_freshness, "fresh");
    assert_eq!(alpha.quota_age_seconds, 100);
    assert_eq!(alpha.retry_backoff_until, Some(1_020));
    assert_eq!(alpha.transport_backoff_until, Some(1_040));
    assert_eq!(
        alpha
            .routes
            .iter()
            .map(|route| route.route.as_str())
            .collect::<Vec<_>>(),
        vec!["responses", "websocket", "compact", "standard"]
    );
    let responses = &alpha.routes[0];
    assert_eq!(responses.circuit_state, "open");
    assert_eq!(responses.circuit_until, Some(1_050));
    assert_eq!(responses.transport_backoff_until, Some(1_040));
    assert_eq!(responses.health_score, 4);
    assert_eq!(responses.bad_pairing_score, 3);
    assert_eq!(responses.performance_score, 5);
    assert_eq!(responses.quota_band, "quota_thin");
    assert_eq!(responses.five_hour_status, "thin");
    assert_eq!(responses.weekly_status, "ready");

    let beta = &profiles[1];
    assert_eq!(beta.quota_freshness, "missing");
    assert_eq!(beta.quota_age_seconds, i64::MAX);
    assert_eq!(beta.routes[0].quota_band, "quota_unknown");
    assert_eq!(beta.routes[0].five_hour_status, "unknown");
}

#[test]
fn runtime_doctor_runtime_broker_mismatch_reason_prioritizes_identity_fields() {
    let current = RuntimeDoctorBinaryIdentity {
        prodex_version: Some("0.5.0".to_string()),
        executable_path: Some("/usr/bin/prodex".to_string()),
        executable_sha256: Some("aaa".to_string()),
    };

    assert_eq!(
        runtime_doctor_runtime_broker_mismatch_reason(
            &current,
            &RuntimeDoctorBinaryIdentity {
                executable_sha256: Some("bbb".to_string()),
                ..current.clone()
            },
        ),
        "sha256_mismatch"
    );
    assert_eq!(
        runtime_doctor_runtime_broker_mismatch_reason(
            &RuntimeDoctorBinaryIdentity {
                executable_sha256: None,
                ..current.clone()
            },
            &RuntimeDoctorBinaryIdentity {
                prodex_version: Some("0.4.0".to_string()),
                executable_sha256: None,
                ..current.clone()
            },
        ),
        "version_mismatch"
    );
    assert_eq!(
        runtime_doctor_runtime_broker_mismatch_reason(
            &RuntimeDoctorBinaryIdentity::default(),
            &RuntimeDoctorBinaryIdentity {
                executable_path: Some("/tmp/prodex".to_string()),
                ..RuntimeDoctorBinaryIdentity::default()
            },
        ),
        "identity_mismatch"
    );
    assert_eq!(
        runtime_doctor_runtime_broker_mismatch_reason(
            &current,
            &RuntimeDoctorBinaryIdentity::default(),
        ),
        "none"
    );
}
