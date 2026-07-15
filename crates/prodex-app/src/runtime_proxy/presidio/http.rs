//! HTTP request adaptation for shared Presidio inspection outcomes.

use super::super::await_runtime_proxy_async_task;
use super::engine::{
    InspectionExecutionOutcome, runtime_local_inspection_required, runtime_presidio_redact_body,
};
use super::findings::{
    runtime_local_inspection_source, runtime_presidio_inspection_plan,
    runtime_presidio_unavailable_source,
};
use super::local::{RuntimeTenantDetectorPatterns, runtime_local_inspect_and_mask_for_tenant};
use super::registry::runtime_presidio_redaction_for_log_path;
use super::telemetry::{
    runtime_emit_inspection_denied_metric, runtime_emit_inspection_metric,
    runtime_inspection_duration_micros, runtime_inspection_error_outcome,
    runtime_log_local_masking_applied, runtime_log_presidio_redaction_applied,
    runtime_log_presidio_redaction_error,
};
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use crate::shared_types::RuntimeProxyRequest;
use anyhow::{Context, Result, anyhow};
use prodex_application::ApplicationInspectionPlan;
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
    if !runtime_local_inspection_required(
        governance.inspection,
        legacy_local_enabled,
        state.is_some() || tenant_detector_patterns.has_for_tenant(tenant_id),
    ) {
        return runtime_presidio_inspection_plan(
            Vec::new(),
            governance.classification_default,
            detector_revision,
        );
    }

    let original_bytes = request.body.len();
    let local_started = Instant::now();
    let local = match runtime_local_inspect_and_mask_for_tenant(
        std::mem::take(&mut request.body),
        tenant_detector_patterns,
        tenant_id,
    ) {
        Ok(local) => local,
        Err(error) => {
            runtime_emit_inspection_metric(
                shared,
                InspectionStage::Local,
                InspectionCoverage::Unsupported,
                &[],
                InspectionMaskingAction::Denied,
                runtime_inspection_error_outcome(&error),
                runtime_inspection_duration_micros(local_started),
            );
            runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
            return Err(error);
        }
    };
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
    let mut sources = vec![runtime_local_inspection_source(
        local.coverage,
        local.findings,
        local.changed,
    )?];
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

    let presidio_input_bytes = request.body.len();
    let external_started = Instant::now();
    let redaction = await_runtime_proxy_async_task(
        shared,
        "presidio_redact_request_body",
        runtime_presidio_redact_body(std::mem::take(&mut request.body), state.clone()),
    );
    match redaction {
        Ok(InspectionExecutionOutcome::Redacted(redaction)) => {
            let presidio_masked = !redaction.source.findings.is_empty();
            runtime_emit_inspection_metric(
                shared,
                InspectionStage::External,
                redaction.source.coverage,
                &redaction.source.findings,
                if presidio_masked {
                    InspectionMaskingAction::Masked
                } else {
                    InspectionMaskingAction::None
                },
                InspectionOutcome::Allowed,
                runtime_inspection_duration_micros(external_started),
            );
            request.body = redaction.body;
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
                sources,
                governance.classification_default,
                detector_revision,
            )
        }
        Ok(InspectionExecutionOutcome::Failed(failure)) => {
            request.body = failure.body;
            let fail_closed = state.config.fail_closed;
            let failure_outcome = runtime_inspection_error_outcome(&failure.error);
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
                failure_outcome,
                runtime_inspection_duration_micros(external_started),
            );
            runtime_log_presidio_redaction_error(request_id, "http", fail_closed, shared);
            if fail_closed {
                runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
                Err(anyhow!("presidio_redaction_failed"))
            } else {
                sources.push(runtime_presidio_unavailable_source("presidio.unavailable")?);
                runtime_presidio_inspection_plan(
                    sources,
                    governance.classification_default,
                    detector_revision,
                )
            }
        }
        Err(_) => {
            runtime_emit_inspection_metric(
                shared,
                InspectionStage::External,
                InspectionCoverage::Unsupported,
                &[],
                InspectionMaskingAction::Denied,
                InspectionOutcome::Error,
                runtime_inspection_duration_micros(external_started),
            );
            runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
            Err(anyhow!("presidio_redaction_failed"))
        }
    }
}
