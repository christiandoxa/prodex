//! HTTP request adaptation for shared Presidio inspection outcomes.

use super::super::await_runtime_proxy_async_task;
use super::engine::{
    InspectionExecutionOutcome, RuntimePresidioFailClosedPolicy, runtime_local_inspection_required,
    runtime_presidio_redact_body,
};
use super::findings::{
    runtime_local_inspection_source, runtime_presidio_inspection_plan,
    runtime_presidio_unavailable_source,
};
use super::local::{RuntimeTenantDetectorPatterns, runtime_local_inspect_and_mask_for_tenant};
use super::registry::{RuntimePresidioRedactionState, runtime_presidio_redaction_for_log_path};
use super::telemetry::{
    runtime_emit_inspection_denied_metric, runtime_emit_inspection_metric,
    runtime_inspection_duration_micros, runtime_inspection_error_outcome,
    runtime_inspection_failure_type, runtime_log_local_masking_applied,
    runtime_log_presidio_redaction_applied, runtime_log_presidio_redaction_error,
};
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use crate::shared_types::RuntimeProxyRequest;
use anyhow::{Context, Result, anyhow};
use prodex_application::{ApplicationInspectionPlan, ApplicationInspectionSource};
use prodex_domain::{DetectorRevisionId, InspectionCoverage, TenantId};
use prodex_observability::{InspectionMaskingAction, InspectionOutcome, InspectionStage};
use std::time::Instant;

const RUNTIME_DEFAULT_DETECTOR_REVISION: &str = "runtime-inspection-v1";

pub(crate) fn apply_runtime_presidio_redaction_to_request(
    request_id: u64,
    request: &mut RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    legacy_local_enabled: bool,
    tenant_id: Option<TenantId>,
) -> Result<ApplicationInspectionPlan> {
    let detector_revision = DetectorRevisionId::new(RUNTIME_DEFAULT_DETECTOR_REVISION)
        .context("invalid detector revision")?;
    apply_runtime_presidio_redaction_to_request_with_rules(
        request_id,
        request,
        shared,
        legacy_local_enabled,
        tenant_id,
        &shared.runtime_config.governance,
        &shared.runtime_config.tenant_detector_patterns,
        &detector_revision,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_runtime_presidio_redaction_to_request_with_rules(
    request_id: u64,
    request: &mut RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    legacy_local_enabled: bool,
    tenant_id: Option<TenantId>,
    governance: &prodex_config::GovernanceConfig,
    tenant_detector_patterns: &RuntimeTenantDetectorPatterns,
    detector_revision: &DetectorRevisionId,
) -> Result<ApplicationInspectionPlan> {
    let state = runtime_presidio_redaction_for_log_path(&shared.log_path);
    let tenant_detector_enabled = tenant_detector_patterns.has_for_tenant(tenant_id);
    let fail_closed_policy = RuntimePresidioFailClosedPolicy::derive(
        governance.inspection,
        governance.mode,
        legacy_local_enabled,
        tenant_detector_enabled,
        state.as_ref().map(|state| state.config.fail_closed),
    );
    if !runtime_local_inspection_required(
        governance.inspection,
        governance.mode,
        legacy_local_enabled,
        state.is_some() || tenant_detector_enabled,
    ) {
        return runtime_presidio_inspection_plan(
            Vec::new(),
            governance.classification_default,
            detector_revision,
        );
    }

    let mut sources = runtime_apply_local_http_inspection(
        request_id,
        request,
        shared,
        tenant_detector_patterns,
        tenant_id,
        fail_closed_policy,
    )?;
    let Some(state) = state else {
        return runtime_presidio_inspection_plan(
            sources,
            governance.classification_default,
            detector_revision,
        );
    };
    if request.body.is_empty() {
        return runtime_presidio_inspection_plan(
            sources,
            governance.classification_default,
            detector_revision,
        );
    }

    runtime_apply_external_http_redaction(
        request_id,
        request,
        shared,
        governance,
        detector_revision,
        fail_closed_policy,
        &mut sources,
        state,
    )
}

fn runtime_apply_local_http_inspection(
    request_id: u64,
    request: &mut RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    tenant_detector_patterns: &RuntimeTenantDetectorPatterns,
    tenant_id: Option<TenantId>,
    fail_closed_policy: RuntimePresidioFailClosedPolicy,
) -> Result<Vec<ApplicationInspectionSource>> {
    let original_bytes = request.body.len();
    let local_started = Instant::now();
    match runtime_local_inspect_and_mask_for_tenant(
        std::mem::take(&mut request.body),
        tenant_detector_patterns,
        tenant_id,
    ) {
        Ok(local) => {
            runtime_emit_inspection_metric(
                shared,
                InspectionStage::Local,
                local.coverage,
                &local.findings,
                if local.changed {
                    InspectionMaskingAction::Masked
                } else {
                    InspectionMaskingAction::None
                },
                InspectionOutcome::Allowed,
                runtime_inspection_duration_micros(local_started),
            );
            request.body = local.body;
            if local.changed {
                runtime_log_local_masking_applied(
                    request_id,
                    "http",
                    original_bytes,
                    request.body.len(),
                    shared,
                );
            }
            Ok(vec![runtime_local_inspection_source(
                local.coverage,
                local.findings,
                local.changed,
            )?])
        }
        Err(failure) => {
            request.body = failure.body;
            runtime_emit_inspection_metric(
                shared,
                InspectionStage::Local,
                InspectionCoverage::Unsupported,
                &[],
                if fail_closed_policy.is_closed() {
                    InspectionMaskingAction::Denied
                } else {
                    InspectionMaskingAction::None
                },
                runtime_inspection_error_outcome(&failure.error),
                runtime_inspection_duration_micros(local_started),
            );
            runtime_log_presidio_redaction_error(
                request_id,
                "http",
                fail_closed_policy.is_closed(),
                runtime_inspection_failure_type(&failure.error),
                shared,
            );
            if fail_closed_policy.is_closed() {
                runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
                return Err(failure.error);
            }
            Ok(vec![runtime_presidio_unavailable_source(
                "local.unavailable",
            )?])
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn runtime_apply_external_http_redaction(
    request_id: u64,
    request: &mut RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    governance: &prodex_config::GovernanceConfig,
    detector_revision: &DetectorRevisionId,
    fail_closed_policy: RuntimePresidioFailClosedPolicy,
    sources: &mut Vec<ApplicationInspectionSource>,
    state: std::sync::Arc<RuntimePresidioRedactionState>,
) -> Result<ApplicationInspectionPlan> {
    let presidio_input_bytes = request.body.len();
    let original_body = request.body.clone();
    let external_started = Instant::now();
    let redaction = await_runtime_proxy_async_task(
        shared,
        "presidio_redact_request_body",
        runtime_presidio_redact_body(std::mem::take(&mut request.body), state.clone()),
    );
    match redaction {
        Ok(InspectionExecutionOutcome::Redacted(redaction)) => {
            runtime_finish_external_http_redaction(
                (request_id, request),
                (shared, governance, detector_revision),
                fail_closed_policy,
                sources,
                presidio_input_bytes,
                external_started,
                redaction,
            )
        }
        Ok(InspectionExecutionOutcome::Failed(failure)) => {
            request.body = failure.body;
            runtime_finish_external_http_failure(
                (request_id, shared),
                (governance, detector_revision),
                fail_closed_policy,
                sources,
                external_started,
                &failure.error,
            )
        }
        Err(error) => {
            request.body = original_body;
            runtime_finish_external_http_failure(
                (request_id, shared),
                (governance, detector_revision),
                fail_closed_policy,
                sources,
                external_started,
                &error,
            )
        }
    }
}

fn runtime_finish_external_http_redaction(
    request: (u64, &mut RuntimeProxyRequest),
    inspection: (
        &RuntimeRotationProxyShared,
        &prodex_config::GovernanceConfig,
        &DetectorRevisionId,
    ),
    fail_closed_policy: RuntimePresidioFailClosedPolicy,
    sources: &mut Vec<ApplicationInspectionSource>,
    presidio_input_bytes: usize,
    external_started: Instant,
    redaction: super::engine::RedactionOutcome,
) -> Result<ApplicationInspectionPlan> {
    let (request_id, request) = request;
    let (shared, governance, detector_revision) = inspection;
    let presidio_masked = !redaction.source.findings.is_empty();
    let denied = fail_closed_policy.denies_external_coverage(redaction.source.coverage);
    runtime_emit_inspection_metric(
        shared,
        InspectionStage::External,
        redaction.source.coverage,
        &redaction.source.findings,
        if denied {
            InspectionMaskingAction::Denied
        } else if presidio_masked {
            InspectionMaskingAction::Masked
        } else {
            InspectionMaskingAction::None
        },
        if denied {
            InspectionOutcome::Denied
        } else {
            InspectionOutcome::Allowed
        },
        runtime_inspection_duration_micros(external_started),
    );
    request.body = redaction.body;
    if denied {
        runtime_log_presidio_redaction_error(
            request_id,
            "http",
            true,
            "unsupported_coverage",
            shared,
        );
        runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
        return Err(anyhow!("presidio_redaction_failed"));
    }
    if presidio_masked {
        runtime_log_presidio_redaction_applied(
            request_id,
            "http",
            presidio_input_bytes,
            request.body.len(),
            shared,
        );
    }
    sources.push(redaction.source);
    runtime_presidio_inspection_plan(
        std::mem::take(sources),
        governance.classification_default,
        detector_revision,
    )
}

fn runtime_finish_external_http_failure(
    request: (u64, &RuntimeRotationProxyShared),
    inspection: (&prodex_config::GovernanceConfig, &DetectorRevisionId),
    fail_closed_policy: RuntimePresidioFailClosedPolicy,
    sources: &mut Vec<ApplicationInspectionSource>,
    external_started: Instant,
    error: &anyhow::Error,
) -> Result<ApplicationInspectionPlan> {
    let (request_id, shared) = request;
    let (governance, detector_revision) = inspection;
    let fail_closed = fail_closed_policy.is_closed();
    runtime_emit_inspection_metric(
        shared,
        InspectionStage::External,
        InspectionCoverage::Unsupported,
        &[],
        if fail_closed {
            InspectionMaskingAction::Denied
        } else {
            InspectionMaskingAction::None
        },
        runtime_inspection_error_outcome(error),
        runtime_inspection_duration_micros(external_started),
    );
    runtime_log_presidio_redaction_error(
        request_id,
        "http",
        fail_closed,
        runtime_inspection_failure_type(error),
        shared,
    );
    if fail_closed {
        runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
        return Err(anyhow!("presidio_redaction_failed"));
    }
    sources.push(runtime_presidio_unavailable_source("presidio.unavailable")?);
    runtime_presidio_inspection_plan(
        std::mem::take(sources),
        governance.classification_default,
        detector_revision,
    )
}
