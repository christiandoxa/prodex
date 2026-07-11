use prodex_domain::{
    HealthCheck, HealthProbeKind, HealthSnapshot, HealthState, PolicyRevisionId,
    plan_health_probe_response,
};

#[test]
fn livez_depends_only_on_process_liveness() {
    let snapshot = HealthSnapshot::new(false, false, false, None, vec![]);
    assert_eq!(snapshot.livez(), HealthState::Failing);

    let snapshot = HealthSnapshot::new(true, false, false, None, vec![]);
    assert_eq!(snapshot.livez(), HealthState::Passing);
}

#[test]
fn startupz_requires_startup_complete() {
    let snapshot = HealthSnapshot::new(true, false, false, None, vec![]);
    assert_eq!(snapshot.startupz(), HealthState::Failing);

    let snapshot = HealthSnapshot::new(true, true, false, None, vec![]);
    assert_eq!(snapshot.startupz(), HealthState::Passing);
}

#[test]
fn readyz_requires_active_policy_revision() {
    let snapshot = HealthSnapshot::new(true, true, false, None, vec![]);

    assert_eq!(snapshot.readyz(), HealthState::Failing);
}

#[test]
fn readyz_is_false_while_draining() {
    let snapshot = HealthSnapshot::new(true, true, true, Some(PolicyRevisionId::new()), vec![]);

    assert_eq!(snapshot.readyz(), HealthState::Failing);
}

#[test]
fn readyz_reports_degraded_when_noncritical_check_is_degraded() {
    let snapshot = HealthSnapshot::new(
        true,
        true,
        false,
        Some(PolicyRevisionId::new()),
        vec![HealthCheck::new(
            "provider",
            HealthState::Degraded,
            Some("slow"),
        )],
    );

    assert_eq!(snapshot.readyz(), HealthState::Degraded);
}

#[test]
fn readyz_fails_when_any_check_fails() {
    let snapshot = HealthSnapshot::new(
        true,
        true,
        false,
        Some(PolicyRevisionId::new()),
        vec![HealthCheck::new(
            "database",
            HealthState::Failing,
            Some("down"),
        )],
    );

    assert_eq!(snapshot.readyz(), HealthState::Failing);
}

#[test]
fn health_check_debug_output_is_stable_and_redacted() {
    let check = HealthCheck::new(
        "database",
        HealthState::Failing,
        Some("dsn=postgres://secret@db/internal"),
    );

    let rendered = format!("{check:?}");
    assert!(rendered.contains("name: \"database\""));
    assert!(rendered.contains("state: Failing"));
    assert!(!rendered.contains("postgres"));
    assert!(!rendered.contains("secret"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn health_snapshot_debug_output_is_stable_and_redacted() {
    let policy_revision = PolicyRevisionId::new();
    let snapshot = HealthSnapshot::new(
        true,
        true,
        false,
        Some(policy_revision),
        vec![HealthCheck::new(
            "database",
            HealthState::Failing,
            Some("dsn=postgres://secret@db/internal"),
        )],
    );

    let rendered = format!("{snapshot:?}");
    assert!(!rendered.contains(&policy_revision.to_string()));
    assert!(!rendered.contains("database"));
    assert!(!rendered.contains("postgres"));
    assert!(!rendered.contains("secret"));
    assert!(rendered.contains("active_policy_revision: Some(\"<redacted>\")"));
    assert!(rendered.contains("check_count: 1"));
}

#[test]
fn readyz_passes_when_live_started_not_draining_policy_active_and_checks_pass() {
    let snapshot = HealthSnapshot::new(
        true,
        true,
        false,
        Some(PolicyRevisionId::new()),
        vec![HealthCheck::new(
            "database",
            HealthState::Passing,
            None::<String>,
        )],
    );

    assert_eq!(snapshot.readyz(), HealthState::Passing);
}

#[test]
fn health_probe_response_plan_is_stable_and_redacted() {
    let policy_revision = PolicyRevisionId::new();
    let snapshot = HealthSnapshot::new(
        true,
        true,
        false,
        Some(policy_revision),
        vec![HealthCheck::new(
            "database",
            HealthState::Failing,
            Some("dsn=postgres://secret@db/internal"),
        )],
    );

    let readyz = plan_health_probe_response(HealthProbeKind::Readyz, &snapshot);

    assert_eq!(readyz.probe, HealthProbeKind::Readyz);
    assert_eq!(readyz.state, HealthState::Failing);
    assert!(!readyz.ready);
    assert_eq!(readyz.active_policy_revision, Some(policy_revision));
    assert_eq!(readyz.code, "health_readyz_failing");
    assert_eq!(readyz.message, "service is not ready");
    assert!(!readyz.message.contains("database"));
    assert!(!readyz.message.contains("postgres"));
    assert!(!readyz.message.contains(&policy_revision.to_string()));
    assert_eq!(
        serde_json::to_value(&readyz).unwrap(),
        serde_json::json!({
            "probe": "readyz",
            "state": "failing",
            "ready": false,
            "active_policy_revision": policy_revision,
            "code": "health_readyz_failing",
            "message": "service is not ready"
        })
    );
    let rendered = format!("{readyz:?}");
    assert!(!rendered.contains(&policy_revision.to_string()));
    assert!(rendered.contains("active_policy_revision: Some(\"<redacted>\")"));

    let livez = plan_health_probe_response(HealthProbeKind::Livez, &snapshot);
    assert_eq!(livez.state, HealthState::Passing);
    assert!(livez.ready);
    assert_eq!(livez.active_policy_revision, Some(policy_revision));
    assert_eq!(livez.code, "health_livez_passing");
    assert_eq!(livez.message, "process is live");
}

#[test]
fn health_probe_response_plan_exposes_active_policy_revision_as_typed_metadata() {
    let policy_revision = PolicyRevisionId::new();
    let snapshot = HealthSnapshot::new(true, true, false, Some(policy_revision), vec![]);

    let readyz = plan_health_probe_response(HealthProbeKind::Readyz, &snapshot);

    assert_eq!(readyz.state, HealthState::Passing);
    assert!(readyz.ready);
    assert_eq!(readyz.active_policy_revision, Some(policy_revision));
    assert!(!readyz.message.contains(&policy_revision.to_string()));
}
