//! WebSocket text adaptation for shared Presidio inspection outcomes.

use super::super::await_runtime_proxy_async_task;
use super::engine::{
    InspectionExecutionOutcome, runtime_local_inspection_fail_closed,
    runtime_local_inspection_required, runtime_presidio_redact_body,
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
    runtime_log_presidio_redaction_applied, runtime_log_presidio_redaction_error,
    runtime_presidio_log_websocket_inspection,
};
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use anyhow::{Context, Result, anyhow};
use prodex_application::{ApplicationInspectionPlan, ApplicationInspectionSource};
use prodex_domain::{DataClassification, DetectorRevisionId, InspectionCoverage, TenantId};
use prodex_observability::{InspectionMaskingAction, InspectionOutcome, InspectionStage};
use std::borrow::Cow;
use std::time::Instant;

const RUNTIME_DEFAULT_DETECTOR_REVISION: &str = "runtime-inspection-v1";

pub(crate) struct RuntimePresidioWebSocketInspection<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) inspection: ApplicationInspectionPlan,
}

pub(crate) fn apply_runtime_presidio_redaction_to_websocket_text<'a>(
    request_id: u64,
    text: &'a str,
    shared: &RuntimeRotationProxyShared,
    legacy_local_enabled: bool,
    tenant_id: Option<TenantId>,
) -> Result<RuntimePresidioWebSocketInspection<'a>> {
    let detector_revision = DetectorRevisionId::new(RUNTIME_DEFAULT_DETECTOR_REVISION)
        .context("invalid detector revision")?;
    apply_runtime_presidio_redaction_to_websocket_text_with_rules(
        request_id,
        text,
        shared,
        legacy_local_enabled,
        tenant_id,
        &shared.runtime_config.governance,
        &shared.runtime_config.tenant_detector_patterns,
        &detector_revision,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_runtime_presidio_redaction_to_websocket_text_with_rules<'a>(
    request_id: u64,
    text: &'a str,
    shared: &RuntimeRotationProxyShared,
    legacy_local_enabled: bool,
    tenant_id: Option<TenantId>,
    governance: &prodex_config::GovernanceConfig,
    tenant_detector_patterns: &RuntimeTenantDetectorPatterns,
    detector_revision: &DetectorRevisionId,
) -> Result<RuntimePresidioWebSocketInspection<'a>> {
    let state = runtime_presidio_redaction_for_log_path(&shared.log_path);
    if !runtime_local_inspection_required(
        governance.inspection,
        legacy_local_enabled,
        state.is_some() || tenant_detector_patterns.has_for_tenant(tenant_id),
    ) {
        let result = RuntimePresidioWebSocketInspection {
            text: Cow::Borrowed(text),
            inspection: runtime_presidio_inspection_plan(
                Vec::new(),
                governance.classification_default,
                detector_revision,
            )?,
        };
        runtime_presidio_log_websocket_inspection(request_id, shared, &result.inspection);
        return Ok(result);
    }

    let local_started = Instant::now();
    let local_fail_closed = runtime_local_inspection_fail_closed(
        governance.inspection,
        legacy_local_enabled,
        tenant_detector_patterns.has_for_tenant(tenant_id),
        state.as_ref().map(|state| state.config.fail_closed),
    );
    let (mut body, mut sources) = match runtime_local_inspect_and_mask_for_tenant(
        text.as_bytes().to_vec(),
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
            let source =
                runtime_local_inspection_source(local.coverage, local.findings, local.changed)?;
            (local.body, vec![source])
        }
        Err(failure) => {
            runtime_emit_inspection_metric(
                shared,
                InspectionStage::Local,
                InspectionCoverage::Unsupported,
                &[],
                if local_fail_closed {
                    InspectionMaskingAction::Denied
                } else {
                    InspectionMaskingAction::None
                },
                runtime_inspection_error_outcome(&failure.error),
                runtime_inspection_duration_micros(local_started),
            );
            if state.is_some() {
                runtime_log_presidio_redaction_error(
                    request_id,
                    "websocket",
                    local_fail_closed,
                    shared,
                );
            }
            if local_fail_closed {
                runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
                return Err(failure.error);
            }
            (
                failure.body,
                vec![runtime_presidio_unavailable_source("local.unavailable")?],
            )
        }
    };
    let Some(state) = state else {
        return runtime_local_websocket_inspection(
            request_id,
            text,
            body,
            sources,
            shared,
            governance.classification_default,
            detector_revision,
        );
    };
    let external_started = Instant::now();
    let redaction = await_runtime_proxy_async_task(
        shared,
        "presidio_redact_websocket_text",
        runtime_presidio_redact_body(body, state.clone()),
    );
    match redaction {
        Ok(InspectionExecutionOutcome::Redacted(redaction)) => {
            runtime_emit_inspection_metric(
                shared,
                InspectionStage::External,
                redaction.source.coverage,
                &redaction.source.findings,
                if redaction.source.findings.is_empty() {
                    InspectionMaskingAction::None
                } else {
                    InspectionMaskingAction::Masked
                },
                InspectionOutcome::Allowed,
                runtime_inspection_duration_micros(external_started),
            );
            if !redaction.source.findings.is_empty() {
                runtime_log_presidio_redaction_applied(
                    request_id,
                    "websocket",
                    text.len(),
                    redaction.body.len(),
                    shared,
                );
            }
            body = redaction.body;
            sources.push(redaction.source);
            runtime_local_websocket_inspection(
                request_id,
                text,
                body,
                sources,
                shared,
                governance.classification_default,
                detector_revision,
            )
        }
        Ok(InspectionExecutionOutcome::Failed(failure)) => {
            body = failure.body;
            let fail_closed = state.config.fail_closed;
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
                runtime_inspection_error_outcome(&failure.error),
                runtime_inspection_duration_micros(external_started),
            );
            runtime_log_presidio_redaction_error(request_id, "websocket", fail_closed, shared);
            if fail_closed {
                runtime_emit_inspection_denied_metric(shared, InspectionStage::RequestEnforcement);
                Err(anyhow!("presidio_redaction_failed"))
            } else {
                sources.push(runtime_presidio_unavailable_source("presidio.unavailable")?);
                runtime_local_websocket_inspection(
                    request_id,
                    text,
                    body,
                    sources,
                    shared,
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

fn runtime_local_websocket_inspection<'a>(
    request_id: u64,
    original: &'a str,
    body: Vec<u8>,
    sources: Vec<ApplicationInspectionSource>,
    shared: &RuntimeRotationProxyShared,
    default_classification: DataClassification,
    detector_revision: &DetectorRevisionId,
) -> Result<RuntimePresidioWebSocketInspection<'a>> {
    let masked = String::from_utf8(body).context("inspection returned non-UTF-8 websocket text")?;
    let result = RuntimePresidioWebSocketInspection {
        text: if masked == original {
            Cow::Borrowed(original)
        } else {
            Cow::Owned(masked)
        },
        inspection: runtime_presidio_inspection_plan(
            sources,
            default_classification,
            detector_revision,
        )?,
    };
    runtime_presidio_log_websocket_inspection(request_id, shared, &result.inspection);
    Ok(result)
}
