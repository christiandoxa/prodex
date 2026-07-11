use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SliKind {
    Availability,
    LatencyP95,
    ErrorRate,
    QuotaCorrectness,
    ProviderDegradation,
    PersistenceFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdDirection {
    AtLeast,
    AtMost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Warning,
    Critical,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SloObjective {
    pub name: String,
    pub sli: SliKind,
    pub target: f64,
    pub direction: ThresholdDirection,
    pub severity: AlertSeverity,
}

impl fmt::Debug for SloObjective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SloObjective")
            .field("name", &"<redacted>")
            .field("sli", &self.sli)
            .field("target", &"<redacted>")
            .field("direction", &self.direction)
            .field("severity", &self.severity)
            .finish()
    }
}

impl SloObjective {
    pub fn new(
        name: impl Into<String>,
        sli: SliKind,
        target: f64,
        direction: ThresholdDirection,
        severity: AlertSeverity,
    ) -> Self {
        Self {
            name: name.into(),
            sli,
            target,
            direction,
            severity,
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SliMeasurement {
    pub sli: SliKind,
    pub value: f64,
}

impl fmt::Debug for SliMeasurement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SliMeasurement")
            .field("sli", &self.sli)
            .field("value", &"<redacted>")
            .finish()
    }
}

impl SliMeasurement {
    pub fn new(sli: SliKind, value: f64) -> Self {
        Self { sli, value }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertDecision {
    pub objective_name: String,
    pub sli: SliKind,
    pub severity: AlertSeverity,
    pub observed: f64,
    pub target: f64,
}

impl fmt::Debug for AlertDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AlertDecision")
            .field("objective_name", &"<redacted>")
            .field("sli", &self.sli)
            .field("severity", &self.severity)
            .field("observed", &"<redacted>")
            .field("target", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SloAlertResponseStatus {
    Degraded,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SloAlertResponsePlan {
    pub status: SloAlertResponseStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_slo_alert_response(decision: &AlertDecision) -> SloAlertResponsePlan {
    let status = match decision.severity {
        AlertSeverity::Warning => SloAlertResponseStatus::Degraded,
        AlertSeverity::Critical => SloAlertResponseStatus::Unavailable,
    };

    let code = match decision.sli {
        SliKind::Availability => "slo_availability_breached",
        SliKind::LatencyP95 => "slo_latency_breached",
        SliKind::ErrorRate => "slo_error_rate_breached",
        SliKind::QuotaCorrectness => "slo_quota_correctness_breached",
        SliKind::ProviderDegradation => "slo_provider_degradation_breached",
        SliKind::PersistenceFailure => "slo_persistence_failure_breached",
    };

    SloAlertResponsePlan {
        status,
        code,
        message: "service objective breached",
    }
}

pub fn evaluate_slo(
    objective: &SloObjective,
    measurement: &SliMeasurement,
) -> Option<AlertDecision> {
    if objective.sli != measurement.sli
        || !measurement.value.is_finite()
        || !objective.target.is_finite()
        || measurement.value.is_sign_negative()
        || objective.target.is_sign_negative()
    {
        return None;
    }
    let breached = match objective.direction {
        ThresholdDirection::AtLeast => measurement.value < objective.target,
        ThresholdDirection::AtMost => measurement.value > objective.target,
    };
    breached.then(|| AlertDecision {
        objective_name: objective.name.clone(),
        sli: objective.sli,
        severity: objective.severity,
        observed: measurement.value,
        target: objective.target,
    })
}

pub fn minimum_enterprise_slo_objectives() -> Vec<SloObjective> {
    vec![
        SloObjective::new(
            "availability",
            SliKind::Availability,
            99.9,
            ThresholdDirection::AtLeast,
            AlertSeverity::Critical,
        ),
        SloObjective::new(
            "p95_latency_ms",
            SliKind::LatencyP95,
            2_000.0,
            ThresholdDirection::AtMost,
            AlertSeverity::Warning,
        ),
        SloObjective::new(
            "error_rate_percent",
            SliKind::ErrorRate,
            1.0,
            ThresholdDirection::AtMost,
            AlertSeverity::Warning,
        ),
        SloObjective::new(
            "quota_correctness",
            SliKind::QuotaCorrectness,
            100.0,
            ThresholdDirection::AtLeast,
            AlertSeverity::Critical,
        ),
        SloObjective::new(
            "provider_degradation_percent",
            SliKind::ProviderDegradation,
            5.0,
            ThresholdDirection::AtMost,
            AlertSeverity::Warning,
        ),
        SloObjective::new(
            "persistence_failures",
            SliKind::PersistenceFailure,
            0.0,
            ThresholdDirection::AtMost,
            AlertSeverity::Critical,
        ),
    ]
}
