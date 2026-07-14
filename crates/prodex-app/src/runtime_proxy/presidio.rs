use super::await_runtime_proxy_async_task;
use crate::presidio_runtime::PresidioLanguageMode;
use crate::runtime_core_shared::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use crate::runtime_proxy_log;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use crate::shared_types::RuntimeProxyRequest;
use crate::{RuntimePresidioRedactionConfig, read_async_response_body_with_limit};
use anyhow::{Context, Result, anyhow};
use arc_swap::ArcSwap;
use prodex_application::{
    ApplicationInspectionPlan, ApplicationInspectionRequest, ApplicationInspectionSource,
    plan_application_request_inspection,
};
use prodex_domain::{
    ContentLocation, DataClassification, DetectorId, DetectorRevisionId, FindingKind,
    InspectionCoverage, InspectionFinding, InspectionLimits, InspectionReasonCode, InspectionTag,
    MAX_INSPECTION_FINDINGS, TenantId,
};
use prodex_observability::{
    InspectionCoverageClass, InspectionFindingCategory, InspectionMaskingAction,
    InspectionMetricPlan, InspectionOutcome, InspectionStage, plan_inspection_metric,
};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

mod analyzer;
mod json_body;
pub(crate) mod local;

use analyzer::{detect_presidio_language, merge_presidio_analyzer_results};
use json_body::{
    PresidioJsonString, collect_json_content, presidio_json_value_separator,
    replace_json_string_values,
};
use local::runtime_local_inspect_and_mask_for_tenant;

static RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH: OnceLock<RuntimePresidioRedactionRegistry> =
    OnceLock::new();
const MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES: usize = 128;
const RUNTIME_DEFAULT_DETECTOR_REVISION: &str = "runtime-inspection-v1";

struct RuntimePresidioRedactionRegistry {
    current: ArcSwap<BTreeMap<PathBuf, Arc<RuntimePresidioRedactionState>>>,
    update: Mutex<()>,
}

impl Default for RuntimePresidioRedactionRegistry {
    fn default() -> Self {
        Self {
            current: ArcSwap::from_pointee(BTreeMap::new()),
            update: Mutex::new(()),
        }
    }
}

struct RuntimePresidioRedactionState {
    config: RuntimePresidioRedactionConfig,
    client: reqwest::Client,
    slots: Arc<tokio::sync::Semaphore>,
}

impl RuntimePresidioRedactionState {
    fn new(config: RuntimePresidioRedactionConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("failed to build Presidio runtime HTTP client")?;
        Ok(Self {
            slots: Arc::new(tokio::sync::Semaphore::new(config.max_concurrency)),
            config,
            client,
        })
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct PresidioAnalyzerResult {
    start: usize,
    end: usize,
    score: f64,
    entity_type: String,
    #[serde(default)]
    language: String,
}

#[derive(Debug, serde::Deserialize)]
struct PresidioAnonymizeResponse {
    text: String,
}

struct RuntimePresidioTextRedaction {
    text: String,
    findings: Vec<PresidioAnalyzerResult>,
}

struct RuntimePresidioRedaction {
    body: Vec<u8>,
    source: ApplicationInspectionSource,
}

#[derive(Debug)]
struct RuntimePresidioRedactionFailure {
    body: Vec<u8>,
    error: anyhow::Error,
}

enum RuntimePresidioRedactionAttempt {
    Success(RuntimePresidioRedaction),
    Failure(RuntimePresidioRedactionFailure),
}

pub(crate) struct RuntimePresidioWebSocketInspection<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) inspection: ApplicationInspectionPlan,
}

pub(crate) fn register_runtime_presidio_redaction_proxy_state(
    log_path: &Path,
    config: Option<RuntimePresidioRedactionConfig>,
) -> Result<()> {
    let state = config
        .map(RuntimePresidioRedactionState::new)
        .transpose()?
        .map(Arc::new);
    let registry = RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH.get_or_init(Default::default);
    let _update = registry
        .update
        .lock()
        .map_err(|_| anyhow!("Presidio registry update lock was poisoned"))?;
    let mut snapshot = (**registry.current.load()).clone();
    if let Some(state) = state {
        validate_runtime_presidio_registry_insert(snapshot.len(), snapshot.contains_key(log_path))?;
        snapshot.insert(log_path.to_path_buf(), state);
    } else {
        snapshot.remove(log_path);
    }
    registry.current.store(Arc::new(snapshot));
    Ok(())
}

fn validate_runtime_presidio_registry_insert(entry_count: usize, replacing: bool) -> Result<()> {
    if !replacing && entry_count >= MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES {
        anyhow::bail!("Presidio registry entry limit reached");
    }
    Ok(())
}

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
    tenant_detector_patterns: &local::RuntimeTenantDetectorPatterns,
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
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "local_inspection_masking_applied",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("original_bytes", original_bytes.to_string()),
                    runtime_proxy_log_field("masked_bytes", request.body.len().to_string()),
                ],
            ),
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
        Ok(RuntimePresidioRedactionAttempt::Success(redaction)) => {
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
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "presidio_redaction_applied",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field(
                                "original_bytes",
                                presidio_input_bytes.to_string(),
                            ),
                            runtime_proxy_log_field(
                                "redacted_bytes",
                                request.body.len().to_string(),
                            ),
                        ],
                    ),
                );
            }
            sources.push(redaction.source);
            runtime_presidio_inspection_plan(
                sources,
                governance.classification_default,
                detector_revision,
            )
        }
        Ok(RuntimePresidioRedactionAttempt::Failure(failure)) => {
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
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "presidio_redaction_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field(
                            "fail_mode",
                            if fail_closed { "closed" } else { "open" },
                        ),
                        runtime_proxy_log_field("reason", "presidio_redaction_failed"),
                    ],
                ),
            );
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
    tenant_detector_patterns: &local::RuntimeTenantDetectorPatterns,
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
    let local = match runtime_local_inspect_and_mask_for_tenant(
        text.as_bytes().to_vec(),
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
    let mut body = local.body;
    let mut sources = vec![runtime_local_inspection_source(
        local.coverage,
        local.findings,
        local.changed,
    )?];
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
        Ok(RuntimePresidioRedactionAttempt::Success(redaction)) => {
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
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "presidio_redaction_applied",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("original_bytes", text.len().to_string()),
                            runtime_proxy_log_field(
                                "redacted_bytes",
                                redaction.body.len().to_string(),
                            ),
                        ],
                    ),
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
        Ok(RuntimePresidioRedactionAttempt::Failure(failure)) => {
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
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "presidio_redaction_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field(
                            "fail_mode",
                            if state.config.fail_closed {
                                "closed"
                            } else {
                                "open"
                            },
                        ),
                        runtime_proxy_log_field("reason", "presidio_redaction_failed"),
                    ],
                ),
            );
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

const fn runtime_local_inspection_required(
    rollout: prodex_config::GovernanceRolloutMode,
    legacy_local_enabled: bool,
    configured_detector_enabled: bool,
) -> bool {
    !matches!(rollout, prodex_config::GovernanceRolloutMode::Off)
        || legacy_local_enabled
        || configured_detector_enabled
}

fn runtime_inspection_coverage_class(coverage: InspectionCoverage) -> InspectionCoverageClass {
    match coverage {
        InspectionCoverage::Full => InspectionCoverageClass::Full,
        InspectionCoverage::Partial => InspectionCoverageClass::Partial,
        InspectionCoverage::Unsupported => InspectionCoverageClass::Unsupported,
    }
}

fn runtime_inspection_finding_category(
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

fn runtime_inspection_duration_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn runtime_inspection_error_outcome(error: &anyhow::Error) -> InspectionOutcome {
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

fn runtime_inspection_metric_message(metric: &InspectionMetricPlan) -> Result<String> {
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

fn runtime_emit_inspection_metric(
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

fn runtime_emit_inspection_denied_metric(
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

fn runtime_presidio_log_websocket_inspection(
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

fn runtime_presidio_redaction_for_log_path(
    log_path: &Path,
) -> Option<Arc<RuntimePresidioRedactionState>> {
    RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH
        .get()
        .and_then(|registry| registry.current.load().get(log_path).cloned())
}

async fn runtime_presidio_redact_body(
    body: Vec<u8>,
    state: Arc<RuntimePresidioRedactionState>,
) -> Result<RuntimePresidioRedactionAttempt> {
    let _permit = match state.slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return Ok(RuntimePresidioRedactionAttempt::Failure(
                RuntimePresidioRedactionFailure {
                    body,
                    error: anyhow!("presidio_concurrency_limit_reached"),
                },
            ));
        }
    };
    let text = match String::from_utf8(body) {
        Ok(text) => text,
        Err(error) => {
            return Ok(RuntimePresidioRedactionAttempt::Failure(
                RuntimePresidioRedactionFailure {
                    body: error.into_bytes(),
                    error: anyhow!("request body is not UTF-8"),
                },
            ));
        }
    };
    Ok(
        match runtime_presidio_redact_text_body(&text, &state).await {
            Ok(redaction) => RuntimePresidioRedactionAttempt::Success(redaction),
            Err(error) => {
                RuntimePresidioRedactionAttempt::Failure(RuntimePresidioRedactionFailure {
                    body: text.into_bytes(),
                    error,
                })
            }
        },
    )
}

async fn runtime_presidio_redact_text_body(
    text: &str,
    state: &RuntimePresidioRedactionState,
) -> Result<RuntimePresidioRedaction> {
    let config = &state.config;
    let client = &state.client;

    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(text) {
        let content = collect_json_content(&json)?;
        if content.values.is_empty() {
            return Ok(RuntimePresidioRedaction {
                body: text.as_bytes().to_vec(),
                source: runtime_presidio_inspection_source(content.coverage, Vec::new(), false)?,
            });
        }

        let separator = presidio_json_value_separator(&content.values);
        let combined = content
            .values
            .iter()
            .map(|value| value.text.as_str())
            .collect::<Vec<_>>()
            .join(&separator);
        let redacted = runtime_presidio_redact_text(client, &combined, config).await?;
        let findings = runtime_presidio_findings(&content.values, &separator, &redacted.findings)?;
        if redacted.text == combined {
            return Ok(RuntimePresidioRedaction {
                body: text.as_bytes().to_vec(),
                source: runtime_presidio_inspection_source(content.coverage, findings, false)?,
            });
        }

        let redacted_values = redacted.text.split(&separator).collect::<Vec<_>>();
        if redacted_values.len() != content.values.len() {
            anyhow::bail!(
                "Presidio changed JSON value separator count: expected {}, got {}",
                content.values.len(),
                redacted_values.len()
            );
        }
        let mut redacted_values = redacted_values.into_iter();
        replace_json_string_values(&mut json, false, &mut redacted_values);
        return Ok(RuntimePresidioRedaction {
            body: serde_json::to_vec(&json)
                .context("failed to serialize redacted JSON request body")?,
            source: runtime_presidio_inspection_source(content.coverage, findings, true)?,
        });
    }

    let redacted = runtime_presidio_redact_text(client, text, config).await?;
    let findings = runtime_presidio_findings(
        &[PresidioJsonString {
            path: "$".to_string(),
            text: text.to_string(),
            sensitive_kind: None,
        }],
        "",
        &redacted.findings,
    )?;
    let changed = redacted.text != text;
    Ok(RuntimePresidioRedaction {
        body: redacted.text.into_bytes(),
        source: runtime_presidio_inspection_source(InspectionCoverage::Full, findings, changed)?,
    })
}

async fn runtime_presidio_redact_text(
    client: &reqwest::Client,
    text: &str,
    config: &RuntimePresidioRedactionConfig,
) -> Result<RuntimePresidioTextRedaction> {
    let languages = &config.languages;
    let language_mode = config.language_mode;

    let mut all_analyzer_results = Vec::new();

    match language_mode {
        PresidioLanguageMode::Fixed => {
            let results = presidio_analyze_async(
                client,
                &config.analyzer_url,
                text,
                &languages[0],
                config.max_response_bytes,
            )
            .await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Auto => {
            let detected_lang =
                detect_presidio_language(text, languages).unwrap_or_else(|| languages[0].clone());
            let results = presidio_analyze_async(
                client,
                &config.analyzer_url,
                text,
                &detected_lang,
                config.max_response_bytes,
            )
            .await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Multi => {
            for lang in languages {
                let results = presidio_analyze_async(
                    client,
                    &config.analyzer_url,
                    text,
                    lang,
                    config.max_response_bytes,
                )
                .await?;
                all_analyzer_results.extend(results.into_iter().map(|mut r| {
                    r.language = lang.clone();
                    r
                }));
            }
            all_analyzer_results = merge_presidio_analyzer_results(all_analyzer_results);
        }
    }

    if all_analyzer_results.is_empty() {
        return Ok(RuntimePresidioTextRedaction {
            text: text.to_string(),
            findings: Vec::new(),
        });
    }
    if all_analyzer_results.len() > MAX_INSPECTION_FINDINGS {
        anyhow::bail!("Presidio finding count exceeded safe limit");
    }

    let anonymized_response = client
        .post(presidio_endpoint(&config.anonymizer_url, "anonymize"))
        .json(&serde_json::json!({
            "text": text,
            "analyzer_results": all_analyzer_results,
        }))
        .send()
        .await
        .context("failed to call Presidio Anonymizer")?
        .error_for_status()
        .context("Presidio Anonymizer returned an error")?;
    let anonymized_body = read_async_response_body_with_limit(
        anonymized_response,
        config.max_response_bytes,
        "failed to read Presidio Anonymizer response",
    )
    .await?;
    let anonymized = serde_json::from_slice::<PresidioAnonymizeResponse>(&anonymized_body)
        .context("failed to parse Presidio Anonymizer response")?;
    Ok(RuntimePresidioTextRedaction {
        text: anonymized.text,
        findings: all_analyzer_results,
    })
}

fn runtime_presidio_inspection_source(
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
    masked: bool,
) -> Result<ApplicationInspectionSource> {
    runtime_inspection_source(coverage, findings, masked, "presidio.finding")
}

fn runtime_local_inspection_source(
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
    masked: bool,
) -> Result<ApplicationInspectionSource> {
    runtime_inspection_source(coverage, findings, masked, "local.finding")
}

fn runtime_inspection_source(
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
    masked: bool,
    finding_reason: &'static str,
) -> Result<ApplicationInspectionSource> {
    let has_findings = !findings.is_empty();
    let masked_findings = if masked {
        let mut kinds = findings
            .iter()
            .map(|finding| finding.kind())
            .collect::<Vec<_>>();
        kinds.sort();
        kinds.dedup();
        kinds
    } else {
        Vec::new()
    };
    Ok(ApplicationInspectionSource {
        coverage,
        findings,
        masked_findings,
        tags: has_findings
            .then(|| InspectionTag::new("sensitive-data"))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("invalid inspection tag")?,
        reason_codes: has_findings
            .then(|| InspectionReasonCode::new(finding_reason))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("invalid inspection reason code")?,
    })
}

fn runtime_presidio_unavailable_source(
    reason: &'static str,
) -> Result<ApplicationInspectionSource> {
    Ok(ApplicationInspectionSource {
        coverage: InspectionCoverage::Unsupported,
        findings: Vec::new(),
        masked_findings: Vec::new(),
        tags: Vec::new(),
        reason_codes: vec![
            InspectionReasonCode::new(reason).context("invalid inspection reason code")?,
        ],
    })
}

fn runtime_presidio_inspection_plan(
    sources: Vec<ApplicationInspectionSource>,
    default_classification: DataClassification,
    detector_revision: &DetectorRevisionId,
) -> Result<ApplicationInspectionPlan> {
    plan_application_request_inspection(ApplicationInspectionRequest {
        sources,
        default_classification,
        trusted_label: None,
        prior_classification: None,
        detector_revision: detector_revision.clone(),
        limits: InspectionLimits::default(),
    })
    .context("failed to build inspection plan")
}

fn runtime_presidio_findings(
    values: &[PresidioJsonString],
    separator: &str,
    findings: &[PresidioAnalyzerResult],
) -> Result<Vec<InspectionFinding>> {
    let separator_chars = separator.chars().count();
    let mut ranges = Vec::with_capacity(values.len());
    let mut cursor = 0usize;
    for value in values {
        let end = cursor
            .checked_add(value.text.chars().count())
            .context("Presidio content offset overflow")?;
        ranges.push((cursor, end));
        cursor = end
            .checked_add(separator_chars)
            .context("Presidio content offset overflow")?;
    }

    findings
        .iter()
        .map(|finding| {
            let (value_index, (value_start, _)) = ranges
                .iter()
                .copied()
                .enumerate()
                .find(|(_, (start, end))| finding.start >= *start && finding.end <= *end)
                .context("Presidio finding crossed a content-field boundary")?;
            let value = &values[value_index];
            let start_char = finding.start - value_start;
            let end_char = finding.end - value_start;
            let start_byte = unicode_scalar_offset_to_byte(&value.text, start_char)
                .context("Presidio finding start offset was invalid")?;
            let end_byte = unicode_scalar_offset_to_byte(&value.text, end_char)
                .context("Presidio finding end offset was invalid")?;
            let confidence = presidio_confidence_basis_points(finding.score)?;
            InspectionFinding::new(
                presidio_finding_kind(&finding.entity_type),
                ContentLocation::new(&value.path, start_byte, end_byte)?,
                confidence,
                DetectorId::new("presidio")?,
            )
            .map_err(anyhow::Error::from)
        })
        .collect()
}

fn unicode_scalar_offset_to_byte(value: &str, offset: usize) -> Option<usize> {
    if offset == value.chars().count() {
        return Some(value.len());
    }
    value.char_indices().nth(offset).map(|(index, _)| index)
}

fn presidio_confidence_basis_points(score: f64) -> Result<u16> {
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        anyhow::bail!("Presidio returned an invalid confidence score");
    }
    Ok((score * 10_000.0).round() as u16)
}

fn presidio_finding_kind(entity_type: &str) -> FindingKind {
    match entity_type {
        "EMAIL_ADDRESS" => FindingKind::EmailAddress,
        "PHONE_NUMBER" => FindingKind::PhoneNumber,
        "PERSON" => FindingKind::PersonName,
        "LOCATION" => FindingKind::PhysicalAddress,
        "CREDIT_CARD" => FindingKind::PaymentCard,
        "IBAN_CODE" | "CRYPTO" => FindingKind::FinancialAccount,
        "API_KEY" => FindingKind::ApiKey,
        "ACCESS_TOKEN" => FindingKind::AccessToken,
        "PRIVATE_KEY" => FindingKind::PrivateKey,
        "PASSWORD" => FindingKind::Password,
        value if value.ends_with("_ID") || value.ends_with("_SSN") => FindingKind::GovernmentId,
        _ => FindingKind::TenantSensitive,
    }
}

async fn presidio_analyze_async(
    client: &reqwest::Client,
    analyzer_url: &str,
    text: &str,
    language: &str,
    max_response_bytes: usize,
) -> Result<Vec<PresidioAnalyzerResult>> {
    let response = client
        .post(presidio_endpoint(analyzer_url, "analyze"))
        .json(&serde_json::json!({
            "text": text,
            "language": language,
        }))
        .send()
        .await
        .context("failed to call Presidio Analyzer")?
        .error_for_status()
        .context("Presidio Analyzer returned an error")?;
    let body = read_async_response_body_with_limit(
        response,
        max_response_bytes,
        "failed to read Presidio Analyzer response",
    )
    .await?;
    let results = serde_json::from_slice::<Vec<PresidioAnalyzerResult>>(&body)
        .context("failed to parse Presidio Analyzer response")?;
    Ok(results)
}

fn presidio_endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

#[cfg(test)]
mod tests {
    use super::{
        PresidioAnalyzerResult, PresidioJsonString, PresidioLanguageMode,
        RuntimePresidioRedactionAttempt, RuntimePresidioRedactionConfig,
        RuntimePresidioRedactionState, runtime_inspection_error_outcome,
        runtime_inspection_metric_message, runtime_local_inspection_required,
        runtime_presidio_findings, runtime_presidio_inspection_plan, runtime_presidio_redact_body,
        validate_runtime_presidio_registry_insert,
    };
    use prodex_observability::{
        InspectionCoverageClass, InspectionFindingCategory, InspectionMaskingAction,
        InspectionOutcome, InspectionStage, plan_inspection_metric,
    };
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};
    use tokio::runtime::Builder as TokioRuntimeBuilder;

    fn start_presidio_fixture(
        response_body: &'static str,
        expected_path: &'static str,
        expected_snippet: &'static str,
    ) -> (String, thread::JoinHandle<()>) {
        start_presidio_fixture_with_delay(
            response_body,
            expected_path,
            expected_snippet,
            Duration::ZERO,
        )
    }

    fn start_presidio_fixture_with_delay(
        response_body: &'static str,
        expected_path: &'static str,
        expected_snippet: &'static str,
        delay: Duration,
    ) -> (String, thread::JoinHandle<()>) {
        let server = TinyServer::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        let handle = thread::spawn(move || {
            let mut request = server.recv().unwrap();
            assert_eq!(request.url(), expected_path);
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            assert!(body.contains(expected_snippet), "{body}");
            thread::sleep(delay);
            let response = TinyResponse::from_string(response_body)
                .with_header(TinyHeader::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn runtime_presidio_redact_body_anonymizes_request_payload() {
        let (analyzer_url, analyzer_handle) = start_presidio_fixture(
            r#"[{"start":8,"end":24,"score":0.99,"entity_type":"EMAIL_ADDRESS"}]"#,
            "/analyze",
            "user@example.com",
        );
        let (anonymizer_url, anonymizer_handle) = start_presidio_fixture(
            r#"{"text":"contact <EMAIL_ADDRESS>"}"#,
            "/anonymize",
            "EMAIL_ADDRESS",
        );
        let config = RuntimePresidioRedactionConfig {
            analyzer_url,
            anonymizer_url,
            languages: vec!["en".to_string()],
            language_mode: PresidioLanguageMode::Fixed,
            fail_closed: true,
            trusted_hosts: Vec::new(),
            timeout_ms: 10_000,
            max_response_bytes: 4 * 1024 * 1024,
            max_concurrency: 8,
        };
        let state = Arc::new(RuntimePresidioRedactionState::new(config).unwrap());
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let attempt = runtime
            .block_on(runtime_presidio_redact_body(
                br#"{"type":"response.create","input":"contact user@example.com"}"#.to_vec(),
                state,
            ))
            .unwrap();
        let RuntimePresidioRedactionAttempt::Success(redacted) = attempt else {
            panic!("Presidio fixture should succeed");
        };
        assert_eq!(redacted.source.findings.len(), 1);
        assert_eq!(
            redacted.source.masked_findings,
            vec![prodex_domain::FindingKind::EmailAddress]
        );
        let text = String::from_utf8(redacted.body).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(!text.contains("user@example.com"));
        assert_eq!(json["type"], "response.create");
        assert_eq!(json["input"], "contact <EMAIL_ADDRESS>");
        analyzer_handle.join().unwrap();
        anonymizer_handle.join().unwrap();
    }

    #[test]
    fn runtime_presidio_detector_failure_matrix_is_bounded_and_content_preserving() {
        enum FailureCase {
            Timeout,
            Unavailable,
            Malformed,
            NonUtf8,
        }

        for case in [
            FailureCase::Timeout,
            FailureCase::Unavailable,
            FailureCase::Malformed,
            FailureCase::NonUtf8,
        ] {
            let (body, analyzer_url, timeout_ms, handle, expected_outcome, expected_error) =
                match case {
                    FailureCase::Timeout => {
                        let (url, handle) = start_presidio_fixture_with_delay(
                            "[]",
                            "/analyze",
                            "synthetic-timeout",
                            Duration::from_millis(100),
                        );
                        (
                            br#"{"input":"synthetic-timeout"}"#.to_vec(),
                            url,
                            10,
                            Some(handle),
                            InspectionOutcome::Timeout,
                            "failed to call Presidio Analyzer",
                        )
                    }
                    FailureCase::Unavailable => {
                        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                        let url = format!("http://{}", listener.local_addr().unwrap());
                        drop(listener);
                        (
                            br#"{"input":"synthetic-unavailable"}"#.to_vec(),
                            url,
                            1_000,
                            None,
                            InspectionOutcome::Error,
                            "failed to call Presidio Analyzer",
                        )
                    }
                    FailureCase::Malformed => {
                        let (url, handle) =
                            start_presidio_fixture("not-json", "/analyze", "synthetic-malformed");
                        (
                            br#"{"input":"synthetic-malformed"}"#.to_vec(),
                            url,
                            1_000,
                            Some(handle),
                            InspectionOutcome::Error,
                            "failed to parse Presidio Analyzer response",
                        )
                    }
                    FailureCase::NonUtf8 => (
                        vec![0xff, 0xfe],
                        "http://127.0.0.1:1".to_string(),
                        1_000,
                        None,
                        InspectionOutcome::Error,
                        "request body is not UTF-8",
                    ),
                };
            let original = body.clone();
            let state = Arc::new(
                RuntimePresidioRedactionState::new(RuntimePresidioRedactionConfig {
                    analyzer_url: analyzer_url.clone(),
                    anonymizer_url: analyzer_url,
                    languages: vec!["en".to_string()],
                    language_mode: PresidioLanguageMode::Fixed,
                    fail_closed: false,
                    trusted_hosts: Vec::new(),
                    timeout_ms,
                    max_response_bytes: 1024,
                    max_concurrency: 1,
                })
                .unwrap(),
            );
            let runtime = TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let attempt = runtime
                .block_on(runtime_presidio_redact_body(body, state))
                .unwrap();
            let RuntimePresidioRedactionAttempt::Failure(failure) = attempt else {
                panic!("detector failure fixture must not succeed");
            };
            assert_eq!(failure.body, original);
            assert_eq!(
                runtime_inspection_error_outcome(&failure.error),
                expected_outcome
            );
            assert!(failure.error.to_string().contains(expected_error));
            if let Some(handle) = handle {
                handle.join().unwrap();
            }
        }
    }

    #[test]
    fn presidio_unicode_scalar_offsets_become_field_byte_offsets() {
        let findings = runtime_presidio_findings(
            &[PresidioJsonString {
                path: "$.tools[0].arguments.*".to_string(),
                text: "é user@example.com".to_string(),
                sensitive_kind: None,
            }],
            "",
            &[PresidioAnalyzerResult {
                start: 2,
                end: 18,
                score: 0.99,
                entity_type: "EMAIL_ADDRESS".to_string(),
                language: "en".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].location().byte_range(), 3..19);
        assert_eq!(
            findings[0].location().field_path(),
            "$.tools[0].arguments.*"
        );
        let detected_only = super::runtime_presidio_inspection_source(
            prodex_domain::InspectionCoverage::Full,
            findings,
            false,
        )
        .unwrap();
        assert!(detected_only.masked_findings.is_empty());
    }

    #[test]
    fn presidio_rejects_malformed_finding_metadata() {
        let error = runtime_presidio_findings(
            &[PresidioJsonString {
                path: "$.input".to_string(),
                text: "safe".to_string(),
                sensitive_kind: None,
            }],
            "",
            &[PresidioAnalyzerResult {
                start: 0,
                end: 4,
                score: f64::NAN,
                entity_type: "PERSON".to_string(),
                language: "en".to_string(),
            }],
        )
        .unwrap_err();

        assert!(error.to_string().contains("invalid confidence score"));
    }

    #[test]
    fn disabled_personal_inspection_preserves_compatibility() {
        assert!(!runtime_local_inspection_required(
            prodex_config::GovernanceRolloutMode::Off,
            false,
            false,
        ));
        assert!(runtime_local_inspection_required(
            prodex_config::GovernanceRolloutMode::Off,
            true,
            false,
        ));
        assert!(runtime_local_inspection_required(
            prodex_config::GovernanceRolloutMode::Observe,
            false,
            false,
        ));
    }

    #[test]
    fn inspection_plan_pins_selected_detector_revision() {
        let revision = prodex_domain::DetectorRevisionId::new("tenant-rules-42").unwrap();

        let plan = runtime_presidio_inspection_plan(
            Vec::new(),
            prodex_domain::DataClassification::Internal,
            &revision,
        )
        .unwrap();

        assert_eq!(plan.result.detector_revision().as_str(), "tenant-rules-42");
    }

    #[test]
    fn presidio_registry_rejects_unbounded_unique_paths_but_allows_replacement() {
        assert!(
            validate_runtime_presidio_registry_insert(
                super::MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES,
                false
            )
            .is_err()
        );
        validate_runtime_presidio_registry_insert(
            super::MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES,
            true,
        )
        .unwrap();
    }

    #[test]
    fn inspection_metric_log_has_only_bounded_content_free_dimensions() {
        let metric = plan_inspection_metric(
            InspectionStage::External,
            InspectionCoverageClass::Partial,
            InspectionFindingCategory::Multiple,
            InspectionMaskingAction::Masked,
            InspectionOutcome::Timeout,
            u64::MAX,
        )
        .unwrap();
        let message = runtime_inspection_metric_message(&metric).unwrap();

        assert!(message.contains("event_metric_name=prodex_inspection_events_total"));
        assert!(message.contains("inspection_stage=external"));
        assert!(message.contains("inspection_coverage=partial"));
        assert!(message.contains("inspection_finding_category=multiple"));
        assert!(message.contains("inspection_masking_action=masked"));
        assert!(message.contains("inspection_outcome=timeout"));
        assert!(message.contains("duration_micros=120000000"));
        for forbidden in [
            "payload-secret-sentinel",
            "tenant_id",
            "request_id",
            "field_path",
            "detector_id",
        ] {
            assert!(!message.contains(forbidden), "{message}");
        }
    }
}
