//! Bounded, content-free Presidio inspection telemetry.

use crate::runtime_core_shared::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use crate::runtime_proxy_log;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use anyhow::{Result, anyhow};
use prodex_application::ApplicationInspectionPlan;
use prodex_domain::{FindingKind, InspectionCoverage, InspectionFinding};
use prodex_observability::{
    InspectionCoverageClass, InspectionFindingCategory, InspectionMaskingAction,
    InspectionMetricPlan, InspectionOutcome, InspectionStage, plan_inspection_metric,
};
use std::time::Instant;

pub(super) fn runtime_inspection_coverage_class(
    coverage: InspectionCoverage,
) -> InspectionCoverageClass {
    match coverage {
        InspectionCoverage::Full => InspectionCoverageClass::Full,
        InspectionCoverage::Partial => InspectionCoverageClass::Partial,
        InspectionCoverage::Unsupported => InspectionCoverageClass::Unsupported,
    }
}

pub(super) fn runtime_inspection_finding_category(
    findings: &[InspectionFinding],
) -> InspectionFindingCategory {
    let mut personal = false;
    let mut credential = false;
    let mut financial = false;
    for finding in findings {
        match finding.kind() {
            FindingKind::EmailAddress
            | FindingKind::PhoneNumber
            | FindingKind::PersonName
            | FindingKind::PhysicalAddress
            | FindingKind::GovernmentId
            | FindingKind::TenantSensitive => personal = true,
            FindingKind::AccessToken
            | FindingKind::ApiKey
            | FindingKind::PrivateKey
            | FindingKind::Password => credential = true,
            FindingKind::FinancialAccount | FindingKind::PaymentCard => financial = true,
        }
    }
    match (personal, credential, financial) {
        (false, false, false) => InspectionFindingCategory::None,
        (true, false, false) => InspectionFindingCategory::PersonalData,
        (false, true, false) => InspectionFindingCategory::Credential,
        (false, false, true) => InspectionFindingCategory::Financial,
        _ => InspectionFindingCategory::Multiple,
    }
}

pub(super) fn runtime_inspection_duration_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

pub(super) fn runtime_inspection_error_outcome(error: &anyhow::Error) -> InspectionOutcome {
    if error.chain().any(|cause| {
        cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(reqwest::Error::is_timeout)
    }) || error.to_string().to_ascii_lowercase().contains("timeout")
    {
        InspectionOutcome::Timeout
    } else {
        InspectionOutcome::Error
    }
}

pub(super) fn runtime_inspection_metric_message(metric: &InspectionMetricPlan) -> Result<String> {
    let labels = [
        &metric.stage_label,
        &metric.coverage_label,
        &metric.finding_category_label,
        &metric.masking_action_label,
        &metric.outcome_label,
    ]
    .map(|label| label.as_metric_label())
    .into_iter()
    .collect::<std::result::Result<Vec<_>, _>>()
    .map_err(|_| anyhow!("invalid inspection metric label"))?;
    Ok(runtime_proxy_structured_log_message(
        "runtime_inspection_metric",
        [
            runtime_proxy_log_field("event_metric_name", metric.event_metric_name),
            runtime_proxy_log_field("duration_metric_name", metric.duration_metric_name),
            runtime_proxy_log_field("increment", metric.increment.to_string()),
            runtime_proxy_log_field("duration_micros", metric.duration_micros.to_string()),
            runtime_proxy_log_field(labels[0].0, labels[0].1),
            runtime_proxy_log_field(labels[1].0, labels[1].1),
            runtime_proxy_log_field(labels[2].0, labels[2].1),
            runtime_proxy_log_field(labels[3].0, labels[3].1),
            runtime_proxy_log_field(labels[4].0, labels[4].1),
        ],
    ))
}

pub(super) fn runtime_emit_inspection_metric(
    shared: &RuntimeRotationProxyShared,
    stage: InspectionStage,
    coverage: InspectionCoverage,
    findings: &[InspectionFinding],
    masking_action: InspectionMaskingAction,
    outcome: InspectionOutcome,
    duration_micros: u64,
) {
    let Ok(metric) = plan_inspection_metric(
        stage,
        runtime_inspection_coverage_class(coverage),
        runtime_inspection_finding_category(findings),
        masking_action,
        outcome,
        duration_micros,
    ) else {
        return;
    };
    if let Ok(message) = runtime_inspection_metric_message(&metric) {
        runtime_proxy_log(shared, message);
    }
}

pub(super) fn runtime_emit_inspection_denied_metric(
    shared: &RuntimeRotationProxyShared,
    stage: InspectionStage,
) {
    runtime_emit_inspection_metric(
        shared,
        stage,
        InspectionCoverage::Unsupported,
        &[],
        InspectionMaskingAction::Denied,
        InspectionOutcome::Denied,
        0,
    );
}

pub(super) fn runtime_log_local_masking_applied(
    request_id: u64,
    transport: &'static str,
    original_bytes: usize,
    masked_bytes: usize,
    shared: &RuntimeRotationProxyShared,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "local_inspection_masking_applied",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("original_bytes", original_bytes.to_string()),
                runtime_proxy_log_field("masked_bytes", masked_bytes.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_log_presidio_redaction_applied(
    request_id: u64,
    transport: &'static str,
    original_bytes: usize,
    redacted_bytes: usize,
    shared: &RuntimeRotationProxyShared,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "presidio_redaction_applied",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("original_bytes", original_bytes.to_string()),
                runtime_proxy_log_field("redacted_bytes", redacted_bytes.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_log_presidio_redaction_error(
    request_id: u64,
    transport: &'static str,
    fail_closed: bool,
    shared: &RuntimeRotationProxyShared,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "presidio_redaction_error",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("fail_mode", if fail_closed { "closed" } else { "open" }),
                runtime_proxy_log_field("reason", "presidio_redaction_failed"),
            ],
        ),
    );
}

pub(super) fn runtime_presidio_log_websocket_inspection(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    inspection: &ApplicationInspectionPlan,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "gateway_request_inspection",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field("coverage", inspection.result.coverage().as_str()),
                runtime_proxy_log_field(
                    "classification",
                    inspection.result.classification().as_str(),
                ),
                runtime_proxy_log_field(
                    "finding_count",
                    inspection.result.findings().len().to_string(),
                ),
            ],
        ),
    );
}
