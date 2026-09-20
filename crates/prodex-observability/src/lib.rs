#![forbid(unsafe_code)]
//! Minimal operational metric planning for the core Prodex runtime.
//!
//! Deterministic metric names and labels are owned by Mojo. Rust only exposes
//! the typed boundary consumed by the runtime and validates the resulting
//! bounded metric labels.

#[cfg(feature = "mojo")]
mod mojo;
#[cfg(not(feature = "mojo"))]
mod rust;

use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

fn metric_name(plan: usize, slot: usize) -> &'static str {
    #[cfg(feature = "mojo")]
    {
        return mojo::metric_name(plan, slot);
    }
    #[cfg(not(feature = "mojo"))]
    rust::metric_name(plan, slot)
}

fn planned_metric_label(
    plan: usize,
    slot: usize,
    value: i64,
) -> Result<TelemetryAttribute, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let (key, value) = {
        let spec = prodex_mojo_core::observability::plan_label_spec(plan as i64, slot as i64)
            .expect("Mojo observability plan-label metadata returned invalid output");
        let key = mojo::label_key(
            usize::try_from(spec.key).expect("Mojo observability label-key index is non-negative"),
        );
        let value = prodex_mojo_core::observability::label(spec.kind, value)
            .expect("Mojo observability plan-label value returned invalid output");
        (key, value)
    };
    #[cfg(not(feature = "mojo"))]
    let (key, value) = rust::planned_metric_label(plan, slot, value);

    let label = TelemetryAttribute::metric_label(key, value);
    label.as_metric_label()?;
    Ok(label)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiRouteKind {
    Responses,
    Compact,
    Websocket,
    ControlPlane,
    Health,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiStatusClass {
    Informational,
    Success,
    Redirection,
    ClientError,
    ServerError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiAdmissionResult {
    Accepted,
    GlobalLimitReached,
    RouteLimitReached,
    QueueFull,
    Draining,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiRedMetricPlan {
    pub request_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub route_label: TelemetryAttribute,
    pub status_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiAdmissionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub route_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

pub fn plan_api_red_metric(
    route: ApiRouteKind,
    status_class: ApiStatusClass,
    duration_ms: u64,
) -> Result<ApiRedMetricPlan, TelemetryAttributeError> {
    Ok(ApiRedMetricPlan {
        request_count_metric_name: metric_name(17, 0),
        duration_metric_name: metric_name(17, 1),
        increment: 1,
        duration_ms,
        route_label: planned_metric_label(17, 0, route as i64)?,
        status_label: planned_metric_label(17, 1, status_class as i64)?,
    })
}

pub fn plan_api_admission_metric(
    route: ApiRouteKind,
    result: ApiAdmissionResult,
) -> Result<ApiAdmissionMetricPlan, TelemetryAttributeError> {
    Ok(ApiAdmissionMetricPlan {
        metric_name: metric_name(7, 0),
        increment: 1,
        route_label: planned_metric_label(7, 0, route as i64)?,
        result_label: planned_metric_label(7, 1, result as i64)?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Gemini,
    Local,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderResultClass {
    Success,
    RateLimited,
    Overloaded,
    ProviderError,
    TransportError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderMetricPlan {
    pub request_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub provider_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

pub fn plan_provider_metric(
    provider: ProviderKind,
    result: ProviderResultClass,
    duration_ms: u64,
) -> Result<ProviderMetricPlan, TelemetryAttributeError> {
    Ok(ProviderMetricPlan {
        request_count_metric_name: metric_name(50, 0),
        duration_metric_name: metric_name(50, 1),
        increment: 1,
        duration_ms,
        provider_label: planned_metric_label(50, 0, provider as i64)?,
        result_label: planned_metric_label(50, 1, result as i64)?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretProviderBackend {
    File,
    Keyring,
    ExternalManager,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretProviderOperation {
    Read,
    Write,
    Delete,
    RevisionLookup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretProviderResult {
    Success,
    NotFound,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretProviderMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub backend_label: TelemetryAttribute,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

pub fn plan_secret_provider_metric(
    backend: SecretProviderBackend,
    operation: SecretProviderOperation,
    result: SecretProviderResult,
) -> Result<SecretProviderMetricPlan, TelemetryAttributeError> {
    Ok(SecretProviderMetricPlan {
        metric_name: metric_name(44, 0),
        increment: 1,
        backend_label: planned_metric_label(44, 0, backend as i64)?,
        operation_label: planned_metric_label(44, 1, operation as i64)?,
        result_label: planned_metric_label(44, 2, result as i64)?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionStage {
    Local,
    External,
    Merge,
    RequestEnforcement,
    ResponseEnforcement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionCoverageClass {
    Full,
    Partial,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionFindingCategory {
    None,
    PersonalData,
    Credential,
    Financial,
    Multiple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionMaskingAction {
    None,
    Masked,
    Denied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionOutcome {
    Allowed,
    Denied,
    Timeout,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectionMetricPlan {
    pub event_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_micros: u64,
    pub stage_label: TelemetryAttribute,
    pub coverage_label: TelemetryAttribute,
    pub finding_category_label: TelemetryAttribute,
    pub masking_action_label: TelemetryAttribute,
    pub outcome_label: TelemetryAttribute,
}

pub fn plan_inspection_metric(
    stage: InspectionStage,
    coverage: InspectionCoverageClass,
    finding_category: InspectionFindingCategory,
    masking_action: InspectionMaskingAction,
    outcome: InspectionOutcome,
    duration_micros: u64,
) -> Result<InspectionMetricPlan, TelemetryAttributeError> {
    Ok(InspectionMetricPlan {
        event_metric_name: metric_name(63, 0),
        duration_metric_name: metric_name(63, 1),
        increment: 1,
        duration_micros: duration_micros.min(120_000_000),
        stage_label: planned_metric_label(63, 0, stage as i64)?,
        coverage_label: planned_metric_label(63, 1, coverage as i64)?,
        finding_category_label: planned_metric_label(63, 2, finding_category as i64)?,
        masking_action_label: planned_metric_label(63, 3, masking_action as i64)?,
        outcome_label: planned_metric_label(63, 4, outcome as i64)?,
    })
}
