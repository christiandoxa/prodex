use prodex_domain::{
    AlertDecision, AlertSeverity, SliKind, SliMeasurement, SloAlertResponseStatus, SloObjective,
    ThresholdDirection, evaluate_slo, minimum_enterprise_slo_objectives, plan_slo_alert_response,
};

#[test]
fn availability_below_target_alerts() {
    let objective = SloObjective::new(
        "availability",
        SliKind::Availability,
        99.9,
        ThresholdDirection::AtLeast,
        AlertSeverity::Critical,
    );

    assert_eq!(
        evaluate_slo(
            &objective,
            &SliMeasurement::new(SliKind::Availability, 99.8)
        ),
        Some(AlertDecision {
            objective_name: "availability".to_string(),
            sli: SliKind::Availability,
            severity: AlertSeverity::Critical,
            observed: 99.8,
            target: 99.9,
        })
    );
}

#[test]
fn latency_above_target_alerts() {
    let objective = SloObjective::new(
        "p95_latency_ms",
        SliKind::LatencyP95,
        2_000.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Warning,
    );

    assert!(
        evaluate_slo(
            &objective,
            &SliMeasurement::new(SliKind::LatencyP95, 2_001.0)
        )
        .is_some()
    );
}

#[test]
fn quota_correctness_below_target_alerts() {
    let objective = SloObjective::new(
        "quota_correctness",
        SliKind::QuotaCorrectness,
        100.0,
        ThresholdDirection::AtLeast,
        AlertSeverity::Critical,
    );

    assert!(
        evaluate_slo(
            &objective,
            &SliMeasurement::new(SliKind::QuotaCorrectness, 99.99)
        )
        .is_some()
    );
}

#[test]
fn provider_degradation_above_target_alerts() {
    let objective = SloObjective::new(
        "provider_error_rate",
        SliKind::ProviderDegradation,
        5.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Warning,
    );

    assert!(
        evaluate_slo(
            &objective,
            &SliMeasurement::new(SliKind::ProviderDegradation, 6.0)
        )
        .is_some()
    );
}

#[test]
fn persistence_failure_above_target_alerts() {
    let objective = SloObjective::new(
        "persistence_failures",
        SliKind::PersistenceFailure,
        0.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Critical,
    );

    assert!(
        evaluate_slo(
            &objective,
            &SliMeasurement::new(SliKind::PersistenceFailure, 1.0)
        )
        .is_some()
    );
}

#[test]
fn healthy_measurement_does_not_alert() {
    let objective = SloObjective::new(
        "error_rate",
        SliKind::ErrorRate,
        1.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Warning,
    );

    assert_eq!(
        evaluate_slo(&objective, &SliMeasurement::new(SliKind::ErrorRate, 0.5)),
        None
    );
}

#[test]
fn mismatched_or_non_finite_measurement_does_not_alert() {
    let objective = SloObjective::new(
        "error_rate",
        SliKind::ErrorRate,
        1.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Warning,
    );

    assert_eq!(
        evaluate_slo(&objective, &SliMeasurement::new(SliKind::Availability, 0.0)),
        None
    );
    assert_eq!(
        evaluate_slo(
            &objective,
            &SliMeasurement::new(SliKind::ErrorRate, f64::NAN)
        ),
        None
    );
}

#[test]
fn negative_slo_measurement_or_target_is_not_accepted() {
    let objective = SloObjective::new(
        "error_rate",
        SliKind::ErrorRate,
        1.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Warning,
    );

    assert_eq!(
        evaluate_slo(&objective, &SliMeasurement::new(SliKind::ErrorRate, -1.0)),
        None
    );

    let negative_target = SloObjective::new(
        "persistence_failures",
        SliKind::PersistenceFailure,
        -1.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Critical,
    );
    assert_eq!(
        evaluate_slo(
            &negative_target,
            &SliMeasurement::new(SliKind::PersistenceFailure, 0.0)
        ),
        None
    );
}

#[test]
fn minimum_enterprise_slo_baseline_covers_required_operational_signals() {
    let objectives = minimum_enterprise_slo_objectives();

    assert_eq!(objectives.len(), 6);
    assert!(objectives.iter().any(|objective| {
        objective.sli == SliKind::Availability
            && objective.direction == ThresholdDirection::AtLeast
            && objective.severity == AlertSeverity::Critical
    }));
    assert!(objectives.iter().any(|objective| {
        objective.sli == SliKind::LatencyP95
            && objective.direction == ThresholdDirection::AtMost
            && objective.severity == AlertSeverity::Warning
    }));
    assert!(objectives.iter().any(|objective| {
        objective.sli == SliKind::ErrorRate
            && objective.direction == ThresholdDirection::AtMost
            && objective.severity == AlertSeverity::Warning
    }));
    assert!(objectives.iter().any(|objective| {
        objective.sli == SliKind::QuotaCorrectness
            && objective.direction == ThresholdDirection::AtLeast
            && objective.severity == AlertSeverity::Critical
    }));
    assert!(objectives.iter().any(|objective| {
        objective.sli == SliKind::ProviderDegradation
            && objective.direction == ThresholdDirection::AtMost
            && objective.severity == AlertSeverity::Warning
    }));
    assert!(objectives.iter().any(|objective| {
        objective.sli == SliKind::PersistenceFailure
            && objective.direction == ThresholdDirection::AtMost
            && objective.severity == AlertSeverity::Critical
    }));
}

#[test]
fn slo_debug_output_is_stable_and_redacted() {
    let objective = SloObjective::new(
        "tenant-a-p95-latency-ms",
        SliKind::LatencyP95,
        2_000.0,
        ThresholdDirection::AtMost,
        AlertSeverity::Warning,
    );
    let measurement = SliMeasurement::new(SliKind::LatencyP95, 9_999.0);
    let decision = evaluate_slo(&objective, &measurement).unwrap();

    let rendered = format!("{objective:?} {measurement:?} {decision:?}");
    for sensitive in ["tenant-a", "2000", "9999"] {
        assert!(
            !rendered.contains(sensitive),
            "SLO debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("sli: LatencyP95"));
    assert!(rendered.contains("severity: Warning"));
    assert!(rendered.contains("objective_name: \"<redacted>\""));
    assert!(rendered.contains("observed: \"<redacted>\""));
}

#[test]
fn slo_alert_responses_are_stable_and_redacted() {
    let warning = AlertDecision {
        objective_name: "tenant-a-p95-latency-ms".to_string(),
        sli: SliKind::LatencyP95,
        severity: AlertSeverity::Warning,
        observed: 9_999.0,
        target: 2_000.0,
    };
    let response = plan_slo_alert_response(&warning);

    assert_eq!(response.status, SloAlertResponseStatus::Degraded);
    assert_eq!(response.code, "slo_latency_breached");
    assert_eq!(response.message, "service objective breached");

    let critical = AlertDecision {
        objective_name: "tenant-b-quota-correctness".to_string(),
        sli: SliKind::QuotaCorrectness,
        severity: AlertSeverity::Critical,
        observed: 99.0,
        target: 100.0,
    };
    let critical_response = plan_slo_alert_response(&critical);
    assert_eq!(
        critical_response.status,
        SloAlertResponseStatus::Unavailable
    );
    assert_eq!(critical_response.code, "slo_quota_correctness_breached");

    let rendered = format!("{response:?} {critical_response:?}");
    for sensitive in [
        "tenant-a",
        "tenant-b",
        "9999",
        "2000",
        "99.0",
        "100.0",
        "objective_name",
        "observed",
        "target",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "SLO response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
