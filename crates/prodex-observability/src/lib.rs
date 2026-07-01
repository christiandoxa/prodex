#![forbid(unsafe_code)]
//! Observability boundary primitives.
//!
//! This crate assembles trace propagation, span descriptors, and metric label
//! validation without depending on OpenTelemetry SDKs, HTTP frameworks, storage,
//! filesystem, or async runtimes.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    CorrelationContext, GatewaySpanDescriptor, GatewaySpanKind, JwksCacheSnapshot,
    JwksRefreshDecision, PolicyCacheStatus, PolicyRefreshDecision, PolicySnapshot,
    TelemetryAttribute, TelemetryAttributeError, TraceId, TraceIdError, evaluate_jwks_refresh,
    evaluate_policy_refresh,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: String,
    pub trace_flags: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceContextError {
    InvalidTraceId(TraceIdError),
    InvalidSpanId,
    InvalidFlags,
    MalformedTraceparent,
    InvalidTracestate,
    InvalidBaggage,
}

impl fmt::Display for TraceContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTraceId(err) => err.fmt(f),
            Self::InvalidSpanId => write!(f, "trace context is invalid"),
            Self::InvalidFlags => write!(f, "trace context is invalid"),
            Self::MalformedTraceparent => write!(f, "trace context is invalid"),
            Self::InvalidTracestate => write!(f, "trace context is invalid"),
            Self::InvalidBaggage => write!(f, "trace context is invalid"),
        }
    }
}

impl Error for TraceContextError {}

impl TraceContext {
    pub fn new(
        trace_id: impl Into<String>,
        span_id: impl Into<String>,
        trace_flags: impl Into<String>,
    ) -> Result<Self, TraceContextError> {
        let trace_id = TraceId::new(trace_id).map_err(TraceContextError::InvalidTraceId)?;
        let span_id = span_id.into().trim().to_ascii_lowercase();
        if span_id.len() != 16 || !span_id.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(TraceContextError::InvalidSpanId);
        }
        let trace_flags = trace_flags.into().trim().to_ascii_lowercase();
        if trace_flags.len() != 2 || !trace_flags.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(TraceContextError::InvalidFlags);
        }
        Ok(Self {
            trace_id,
            span_id,
            trace_flags,
        })
    }

    pub fn parse_traceparent(value: &str) -> Result<Self, TraceContextError> {
        let parts = value.trim().split('-').collect::<Vec<_>>();
        if parts.len() != 4 || parts[0] != "00" {
            return Err(TraceContextError::MalformedTraceparent);
        }
        Self::new(parts[1], parts[2], parts[3])
    }

    pub fn traceparent(&self) -> String {
        format!(
            "00-{}-{}-{}",
            self.trace_id.as_str(),
            self.span_id,
            self.trace_flags
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TracePropagationPlan {
    pub traceparent: String,
    pub tracestate: Option<String>,
    pub baggage: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TracePropagationCarrier {
    Traceparent,
    Tracestate,
    Baggage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TracePropagationResult {
    Propagated,
    Rejected,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TracePropagationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub carrier_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

pub fn plan_trace_propagation(
    trace_context: &TraceContext,
    tracestate: Option<&str>,
    baggage: Option<&str>,
) -> Result<TracePropagationPlan, TraceContextError> {
    let tracestate = tracestate.map(validate_tracestate).transpose()?;
    let baggage = baggage.map(validate_baggage).transpose()?;
    Ok(TracePropagationPlan {
        traceparent: trace_context.traceparent(),
        tracestate,
        baggage,
    })
}

fn validate_tracestate(value: &str) -> Result<String, TraceContextError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 512
        || value
            .chars()
            .any(|character| !character.is_ascii_graphic() && character != ' ')
    {
        return Err(TraceContextError::InvalidTracestate);
    }
    Ok(value.to_string())
}

fn validate_baggage(value: &str) -> Result<String, TraceContextError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 8192
        || value
            .chars()
            .any(|character| !character.is_ascii_graphic() && character != ' ')
    {
        return Err(TraceContextError::InvalidBaggage);
    }
    Ok(value.to_string())
}

pub fn plan_trace_propagation_metric(
    carrier: TracePropagationCarrier,
    result: TracePropagationResult,
) -> Result<TracePropagationMetricPlan, TelemetryAttributeError> {
    let carrier_label =
        TelemetryAttribute::metric_label("trace_carrier", trace_propagation_carrier_label(carrier));
    let result_label = TelemetryAttribute::metric_label(
        "trace_propagation_result",
        trace_propagation_result_label(result),
    );
    carrier_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TracePropagationMetricPlan {
        metric_name: "prodex_trace_propagation_events_total",
        increment: 1,
        carrier_label,
        result_label,
    })
}

fn trace_propagation_carrier_label(carrier: TracePropagationCarrier) -> &'static str {
    match carrier {
        TracePropagationCarrier::Traceparent => "traceparent",
        TracePropagationCarrier::Tracestate => "tracestate",
        TracePropagationCarrier::Baggage => "baggage",
    }
}

fn trace_propagation_result_label(result: TracePropagationResult) -> &'static str {
    match result {
        TracePropagationResult::Propagated => "propagated",
        TracePropagationResult::Rejected => "rejected",
        TracePropagationResult::Missing => "missing",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanPlan {
    pub descriptor: GatewaySpanDescriptor,
    pub trace_context: Option<TraceContext>,
    pub correlation: CorrelationContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuredLogCorrelationPlan {
    pub fields: Vec<TelemetryAttribute>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterpriseIdKind {
    Tenant,
    Principal,
    Request,
    Call,
    Reservation,
    VirtualKey,
    PolicyRevision,
    AuditEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterpriseIdResult {
    Generated,
    Parsed,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnterpriseIdMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub kind_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JwksCacheAgeMetricPlan {
    pub metric_name: &'static str,
    pub age_ms: Option<u64>,
    pub state_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicySnapshotAgeMetricPlan {
    pub metric_name: &'static str,
    pub age_ms: Option<u64>,
    pub state_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JwksRefreshOutcome {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcRefreshOperation {
    DiscoverIssuer,
    FetchJwks,
    ValidateSnapshot,
    WriteCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcRefreshResult {
    Success,
    SkippedFresh,
    Backoff,
    InvalidSnapshot,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRefreshOutcome {
    Success,
    Failure,
    LastKnownGoodFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JwksRefreshOutcomeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcRefreshMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRefreshOutcomeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRollbackOperation {
    ActivateLastKnownGood,
    RejectCandidate,
    Rollback,
    Verify,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRollbackResult {
    Success,
    Failed,
    Blocked,
    Noop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRollbackMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigActivationSource {
    PublishedRevision,
    LastKnownGood,
    Rollback,
    InvalidationFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigActivationResult {
    Activated,
    Rejected,
    MissingLastKnownGood,
    InvalidRevision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigActivationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub source_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationDeliveryTarget {
    GatewayCacheRefresh,
    RuntimePolicyReload,
    AuditProjection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationDeliveryResult {
    Delivered,
    Failed,
    Skipped,
    RetryScheduled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationDeliveryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub target_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigCacheInvalidationTarget {
    GatewayPolicyCache,
    RuntimePolicyCache,
    RedisPolicyCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigCacheInvalidationResult {
    Invalidated,
    ReloadScheduled,
    NotFound,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigCacheInvalidationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub target_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetryDropReason {
    QueueFull,
    ExporterUnavailable,
    Shutdown,
    InvalidPayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedTelemetryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub reason_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueDepthKind {
    Responses,
    Compact,
    Websocket,
    Telemetry,
    Persistence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueDepthMetricPlan {
    pub metric_name: &'static str,
    pub depth: u64,
    pub capacity: u64,
    pub queue_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPoolKind {
    Postgres,
    Redis,
    ProviderHttp,
    OidcHttp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionPoolSaturationMetricPlan {
    pub metric_name: &'static str,
    pub in_use: u64,
    pub capacity: u64,
    pub pool_label: TelemetryAttribute,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiRedMetricPlan {
    pub request_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub route_label: TelemetryAttribute,
    pub status_label: TelemetryAttribute,
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
pub struct ApiAdmissionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub route_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiSchemaSurface {
    Request,
    Response,
    OpenApi,
    ErrorEnvelope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiSchemaValidationResult {
    Valid,
    Invalid,
    MissingSchema,
    Incompatible,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiSchemaValidationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiDeprecationSurface {
    DataPlane,
    ControlPlane,
    Scim,
    Health,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiDeprecationSignal {
    Notice,
    Sunset,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiDeprecationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub signal_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiPaginationSurface {
    ControlPlane,
    Scim,
    AuditExport,
    Quota,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiPaginationResult {
    PageReturned,
    EmptyPage,
    InvalidCursor,
    ExpiredCursor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiPaginationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiPreconditionSurface {
    Tenant,
    Principal,
    VirtualKey,
    Policy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiPreconditionResult {
    Matched,
    Missing,
    Mismatched,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiPreconditionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiIdempotencySurface {
    TenantMutation,
    PrincipalMutation,
    VirtualKeyMutation,
    PolicyMutation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiIdempotencyResult {
    Accepted,
    Replayed,
    Conflict,
    Missing,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiIdempotencyMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotencyRecordBackend {
    Postgres,
    Sqlite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotencyRecordOperation {
    PendingInsert,
    Complete,
    Lookup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotencyRecordResult {
    Recorded,
    Replayed,
    Conflict,
    NotFound,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdempotencyRecordMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub backend_label: TelemetryAttribute,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCompatibilitySurface {
    DataPlane,
    ControlPlane,
    Scim,
    ErrorEnvelope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCompatibilityResult {
    Compatible,
    AdditiveChange,
    DeprecatedChange,
    BreakingChange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiCompatibilityMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiMutationAuditSurface {
    Tenant,
    Principal,
    VirtualKey,
    Policy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiMutationAuditResult {
    Required,
    Persisted,
    Missing,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiMutationAuditMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiVersionSurface {
    DataPlane,
    ControlPlane,
    Scim,
    Health,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiVersionResult {
    Accepted,
    Defaulted,
    Deprecated,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiVersionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiSpecSurface {
    GatewayOpenApi,
    ControlPlaneOpenApi,
    ScimSchema,
    ErrorEnvelope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiSpecPublicationResult {
    Generated,
    Validated,
    Published,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiSpecPublicationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiErrorEnvelopeSurface {
    DataPlane,
    ControlPlane,
    Scim,
    Health,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiErrorEnvelopeResult {
    Emitted,
    Redacted,
    ValidationFailed,
    CompatibilityRejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiErrorEnvelopeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiBodyLimitSurface {
    DataPlane,
    ControlPlane,
    Scim,
    Upload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiBodyLimitResult {
    Accepted,
    RejectedTooLarge,
    UnknownLength,
    Truncated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiBodyLimitMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiTimeoutBudgetSurface {
    DataPlane,
    ControlPlane,
    Provider,
    Persistence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiTimeoutBudgetResult {
    Accepted,
    Expired,
    Exhausted,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiTimeoutBudgetMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCancellationSurface {
    DataPlane,
    ControlPlane,
    ProviderStream,
    Persistence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCancellationSource {
    ClientDisconnect,
    TimeoutBudget,
    ShutdownDrain,
    UpstreamAbort,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiCancellationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub source_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiStreamBackpressureSurface {
    DataPlaneStream,
    ProviderStream,
    Websocket,
    AuditExport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiStreamBackpressureState {
    Ready,
    Paused,
    Dropped,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiStreamBackpressureMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub state_label: TelemetryAttribute,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCapabilityKind {
    ResponsesApi,
    Streaming,
    Tools,
    Vision,
    JsonMode,
    RemoteCompact,
    WebSocket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCapabilityResult {
    Compatible,
    Incompatible,
    NoCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCapabilityNegotiationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub capability_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryAttemptStage {
    BeforeDispatch,
    BeforeFirstByte,
    AfterFirstByte,
    AfterCancellation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryOutcome {
    Allowed,
    DeniedCommitted,
    DeniedBudgetExhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRetryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub stage_label: TelemetryAttribute,
    pub outcome_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCircuitBreakerDecision {
    Closed,
    Open,
    HalfOpenProbe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCircuitBreakerEvent {
    Success,
    Failure,
    Probe,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCircuitBreakerMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub decision_label: TelemetryAttribute,
    pub event_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderDegradationSignal {
    ErrorRate,
    Latency,
    Overload,
    Transport,
    CircuitOpen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderDegradationSeverity {
    Warning,
    Critical,
    Recovered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderDegradationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub signal_label: TelemetryAttribute,
    pub severity_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamTransportKind {
    Responses,
    Websocket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamOutcome {
    Completed,
    Cancelled,
    Interrupted,
    GuardrailBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamingLifecycleMetricPlan {
    pub event_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub transport_label: TelemetryAttribute,
    pub outcome_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingLaneKind {
    Responses,
    Compact,
    Websocket,
    ControlPlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingDecisionOutcome {
    Selected,
    Fallback,
    Rejected,
    NoCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingDecisionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub lane_label: TelemetryAttribute,
    pub outcome_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditOperation {
    Emit,
    Persist,
    Export,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditResult {
    Success,
    Failure,
    Dropped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditQueryLifecycleOperation {
    PlanQuery,
    PageQuery,
    PlanExport,
    SerializeExport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditQueryLifecycleResult {
    Planned,
    PageReturned,
    Empty,
    Denied,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditQueryLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditChainOperation {
    Append,
    VerifyLink,
    VerifyRange,
    ExportProof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditChainResult {
    Success,
    Conflict,
    DigestInvalid,
    GapDetected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditChainMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditRetentionPurgeOperation {
    SelectCandidates,
    ApplyLegalHold,
    DeleteBatch,
    VerifyChain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditRetentionPurgeResult {
    Success,
    Protected,
    Empty,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRetentionPurgeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityDecisionKind {
    Authentication,
    TenantResolution,
    Authorization,
    CredentialScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityDecisionResult {
    Allowed,
    Denied,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityDecisionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub decision_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthnTokenValidationStage {
    Decode,
    Signature,
    Claims,
    TenantClaim,
    RoleClaim,
    JwksCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthnTokenValidationResult {
    Accepted,
    Malformed,
    InvalidSignature,
    Expired,
    UnknownKey,
    MissingTenant,
    RoleDenied,
    CacheUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthnTokenValidationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub stage_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthzBoundaryKind {
    DataPlaneInference,
    DataPlaneQuota,
    ControlPlaneRead,
    ControlPlaneMutation,
    ControlPlaneBilling,
    BreakGlass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthzDecisionResult {
    Allowed,
    CredentialScopeDenied,
    RoleDenied,
    TenantDenied,
    ResourceDenied,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthzDecisionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub boundary_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialScopeMismatchDirection {
    DataPlaneToControlPlane,
    ControlPlaneToDataPlane,
    BreakGlassToDataPlane,
    BreakGlassToControlPlane,
    MissingCredential,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialScopeMismatchResult {
    Rejected,
    Audited,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialScopeMismatchMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub direction_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantIsolationSurface {
    Authentication,
    Authorization,
    StoragePredicate,
    CacheKey,
    AuditQuery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantIsolationResult {
    Enforced,
    CrossTenantDenied,
    MissingTenantDenied,
    MismatchRejected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantIsolationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresTenantContextOperation {
    SetContext,
    VerifyContext,
    ApplyRlsPolicy,
    ExecuteTenantDml,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresTenantContextResult {
    Applied,
    Missing,
    MismatchRejected,
    RlsDenied,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresTenantContextMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityContextSurface {
    Authentication,
    Authorization,
    Audit,
    ControlPlane,
    DataPlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityContextResult {
    Consistent,
    MissingPrincipal,
    MissingTenant,
    TenantMismatch,
    CorrelationMissing,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityContextMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub surface_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakGlassLifecycleOperation {
    Request,
    Approve,
    Activate,
    Revoke,
    Expire,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakGlassLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Expired,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakGlassLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserLifecycleOperation {
    Invite,
    ScimCreate,
    ScimUpdate,
    ScimDelete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceIdentityLifecycleOperation {
    Create,
    RotateSecret,
    Disable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceIdentityLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceIdentityLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleBindingLifecycleOperation {
    Grant,
    Revoke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleBindingLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleBindingLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCredentialLifecycleOperation {
    Rotate,
    ValidateReference,
    PersistReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCredentialLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCredentialLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualKeyLifecycleOperation {
    Create,
    RotateSecret,
    PersistReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualKeyLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualKeyLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetPolicyLifecycleOperation {
    Update,
    ValidateScope,
    PersistPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetPolicyLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetPolicyLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyLifecycleOperation {
    Create,
    Update,
    Publish,
    Invalidate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Published,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantLifecycleOperation {
    Create,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantLifecycleResult {
    Authorized,
    Denied,
    Persisted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservationRecoveryOperation {
    ScanExpired,
    AcquireLease,
    ReleaseBudget,
    WriteLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservationRecoveryResult {
    Recovered,
    Skipped,
    LeaseUnavailable,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReservationRecoveryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountingOperation {
    Reservation,
    Commit,
    Release,
    Expire,
    Reconciliation,
    BudgetRejection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountingResult {
    Accepted,
    Rejected,
    Committed,
    Released,
    Expired,
    Reconciled,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountingMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillingLedgerOperation {
    ReserveAppend,
    CommitAppend,
    ReleaseAppend,
    ReconciliationAppend,
    Query,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillingLedgerResult {
    Written,
    Read,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BillingLedgerMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetRejectionReason {
    TenantBudgetExceeded,
    VirtualKeyBudgetExceeded,
    RateLimited,
    ReservationUnavailable,
    PolicyDenied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetRejectionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub reason_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateLimitScope {
    Tenant,
    VirtualKey,
    Principal,
    Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allowed,
    Delayed,
    Rejected,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitDecisionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub scope_label: TelemetryAttribute,
    pub decision_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisCoordinationOperation {
    RateLimitCheck,
    RateLimitCommit,
    RecoveryLeaseAcquire,
    RecoveryLeaseRelease,
    CacheRead,
    CacheWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisCoordinationResult {
    Success,
    Limited,
    LeaseUnavailable,
    CacheMiss,
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisCoordinationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaCorrectnessEvent {
    ReservationOvershoot,
    DuplicateChargePrevented,
    MissingCommitRecovered,
    MissingReleaseRecovered,
    LedgerMismatchDetected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotaCorrectnessMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub event_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SloAlertSli {
    Availability,
    LatencyP95,
    ErrorRate,
    QuotaCorrectness,
    ProviderDegradation,
    PersistenceFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SloAlertSeverity {
    Warning,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SloAlertMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub sli_label: TelemetryAttribute,
    pub severity_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownLifecycleEvent {
    SignalReceived,
    DrainingStarted,
    ReadinessDisabled,
    InflightDrained,
    TimeoutElapsed,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownLifecycleResult {
    Success,
    Timeout,
    Forced,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShutdownLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub event_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthProbeKind {
    Live,
    Ready,
    Startup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthProbeResult {
    Passing,
    Degraded,
    Failing,
    Draining,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthProbeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub probe_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretRotationScope {
    ProviderCredential,
    OidcClient,
    SigningKey,
    StorageCredential,
    WebhookSecret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretRotationResult {
    Success,
    Failed,
    Skipped,
    Rollback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretRotationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub scope_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupRestoreOperation {
    Backup,
    Restore,
    Verify,
    Drill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupRestoreResult {
    Success,
    Failed,
    Partial,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupRestoreMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentRolloutOperation {
    Apply,
    Verify,
    Promote,
    Rollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentRolloutResult {
    Success,
    Failed,
    Degraded,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploymentRolloutMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadSoakScenarioKind {
    Load,
    Soak,
    Spike,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadSoakResult {
    Passed,
    Failed,
    Aborted,
    ThresholdBreached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadSoakMetricPlan {
    pub event_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub scenario_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultInjectionTarget {
    Postgres,
    Redis,
    Idp,
    Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultInjectionResult {
    Injected,
    Recovered,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultInjectionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub target_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationLifecycleOperation {
    StatusCheck,
    CompatibilityCheck,
    Apply,
    Rollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationLifecycleResult {
    Compatible,
    Applied,
    Blocked,
    Failed,
    RolledBack,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceOperation {
    Read,
    Write,
    Commit,
    Rollback,
    HealthCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceResult {
    Success,
    Conflict,
    Timeout,
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistenceMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanPlanError {
    MissingCorrelationTrace,
    MetricLabel(TelemetryAttributeError),
}

impl fmt::Display for SpanPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCorrelationTrace => write!(f, "span requires propagated trace context"),
            Self::MetricLabel(_) => write!(f, "invalid metric label"),
        }
    }
}

impl Error for SpanPlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservabilityErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservabilityErrorResponsePlan {
    pub status: ObservabilityErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_span_error_response(_error: &SpanPlanError) -> ObservabilityErrorResponsePlan {
    ObservabilityErrorResponsePlan {
        status: ObservabilityErrorStatus::ServiceUnavailable,
        code: "telemetry_unavailable",
        message: "telemetry planning is temporarily unavailable",
    }
}

pub fn plan_trace_context_error_response(
    _error: &TraceContextError,
) -> ObservabilityErrorResponsePlan {
    ObservabilityErrorResponsePlan {
        status: ObservabilityErrorStatus::ServiceUnavailable,
        code: "invalid_trace_context",
        message: "trace context is required and must be valid",
    }
}

pub fn plan_gateway_span(
    kind: GatewaySpanKind,
    name: impl Into<String>,
    correlation: CorrelationContext,
    trace_context: Option<TraceContext>,
    attributes: Vec<TelemetryAttribute>,
) -> Result<SpanPlan, SpanPlanError> {
    if correlation.trace_id.is_none() && trace_context.is_none() {
        return Err(SpanPlanError::MissingCorrelationTrace);
    }
    let mut descriptor = GatewaySpanDescriptor::new(kind, name);
    for attribute in attributes {
        if attribute.scope == prodex_domain::TelemetryAttributeScope::MetricLabel {
            attribute
                .as_metric_label()
                .map_err(SpanPlanError::MetricLabel)?;
        }
        descriptor = descriptor.with_attribute(attribute);
    }
    Ok(SpanPlan {
        descriptor,
        trace_context,
        correlation,
    })
}

pub fn plan_structured_log_correlation(
    correlation: &CorrelationContext,
) -> StructuredLogCorrelationPlan {
    let mut fields = vec![TelemetryAttribute::trace_only(
        "request_id",
        correlation.request_id.to_string(),
    )];
    if let Some(call_id) = correlation.call_id {
        fields.push(TelemetryAttribute::trace_only(
            "call_id",
            call_id.to_string(),
        ));
    }
    if let Some(trace_id) = &correlation.trace_id {
        fields.push(TelemetryAttribute::trace_only(
            "trace_id",
            trace_id.as_str(),
        ));
    }
    if let Some(tenant_id) = correlation.tenant_id {
        fields.push(TelemetryAttribute::trace_only(
            "tenant_id",
            tenant_id.to_string(),
        ));
    }
    if let Some(audit_event_id) = correlation.audit_event_id {
        fields.push(TelemetryAttribute::trace_only(
            "audit_event_id",
            audit_event_id.to_string(),
        ));
    }
    StructuredLogCorrelationPlan { fields }
}

pub fn plan_enterprise_id_metric(
    kind: EnterpriseIdKind,
    result: EnterpriseIdResult,
) -> Result<EnterpriseIdMetricPlan, TelemetryAttributeError> {
    let kind_label =
        TelemetryAttribute::metric_label("enterprise_id_kind", enterprise_id_kind_label(kind));
    let result_label = TelemetryAttribute::metric_label(
        "enterprise_id_result",
        enterprise_id_result_label(result),
    );
    kind_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(EnterpriseIdMetricPlan {
        metric_name: "prodex_enterprise_id_events_total",
        increment: 1,
        kind_label,
        result_label,
    })
}

pub fn plan_jwks_cache_age_metric(
    snapshot: Option<&JwksCacheSnapshot>,
    now_unix_ms: u64,
) -> Result<JwksCacheAgeMetricPlan, TelemetryAttributeError> {
    let decision = evaluate_jwks_refresh(snapshot, now_unix_ms);
    let state_label =
        TelemetryAttribute::metric_label("jwks_cache_state", jwks_refresh_decision_label(decision));
    state_label.as_metric_label()?;
    Ok(JwksCacheAgeMetricPlan {
        metric_name: "prodex_jwks_cache_age_ms",
        age_ms: snapshot.map(|snapshot| now_unix_ms.saturating_sub(snapshot.fetched_at_unix_ms)),
        state_label,
    })
}

pub fn plan_policy_snapshot_age_metric<T>(
    snapshot: Option<&PolicySnapshot<T>>,
    status: &PolicyCacheStatus,
    now_unix_ms: u64,
) -> Result<PolicySnapshotAgeMetricPlan, TelemetryAttributeError> {
    let decision = evaluate_policy_refresh(status, now_unix_ms);
    let state_label = TelemetryAttribute::metric_label(
        "policy_cache_state",
        policy_refresh_decision_label(decision),
    );
    state_label.as_metric_label()?;
    Ok(PolicySnapshotAgeMetricPlan {
        metric_name: "prodex_policy_snapshot_age_ms",
        age_ms: snapshot.map(|snapshot| now_unix_ms.saturating_sub(snapshot.issued_at_unix_ms)),
        state_label,
    })
}

pub fn plan_jwks_refresh_outcome_metric(
    outcome: JwksRefreshOutcome,
) -> Result<JwksRefreshOutcomeMetricPlan, TelemetryAttributeError> {
    let result_label = TelemetryAttribute::metric_label(
        "jwks_refresh_result",
        jwks_refresh_outcome_label(outcome),
    );
    result_label.as_metric_label()?;
    Ok(JwksRefreshOutcomeMetricPlan {
        metric_name: "prodex_jwks_refresh_total",
        increment: 1,
        result_label,
    })
}

pub fn plan_oidc_refresh_metric(
    operation: OidcRefreshOperation,
    result: OidcRefreshResult,
) -> Result<OidcRefreshMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "oidc_refresh_operation",
        oidc_refresh_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("oidc_refresh_result", oidc_refresh_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(OidcRefreshMetricPlan {
        metric_name: "prodex_oidc_refresh_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_policy_refresh_outcome_metric(
    outcome: PolicyRefreshOutcome,
) -> Result<PolicyRefreshOutcomeMetricPlan, TelemetryAttributeError> {
    let result_label = TelemetryAttribute::metric_label(
        "policy_refresh_result",
        policy_refresh_outcome_label(outcome),
    );
    result_label.as_metric_label()?;
    Ok(PolicyRefreshOutcomeMetricPlan {
        metric_name: "prodex_policy_refresh_total",
        increment: 1,
        result_label,
    })
}

pub fn plan_policy_rollback_metric(
    operation: PolicyRollbackOperation,
    result: PolicyRollbackResult,
) -> Result<PolicyRollbackMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "policy_rollback_operation",
        policy_rollback_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "policy_rollback_result",
        policy_rollback_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PolicyRollbackMetricPlan {
        metric_name: "prodex_policy_rollback_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_config_activation_metric(
    source: ConfigActivationSource,
    result: ConfigActivationResult,
) -> Result<ConfigActivationMetricPlan, TelemetryAttributeError> {
    let source_label = TelemetryAttribute::metric_label(
        "config_activation_source",
        config_activation_source_label(source),
    );
    let result_label = TelemetryAttribute::metric_label(
        "config_activation_result",
        config_activation_result_label(result),
    );
    source_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ConfigActivationMetricPlan {
        metric_name: "prodex_config_activation_events_total",
        increment: 1,
        source_label,
        result_label,
    })
}

pub fn plan_config_publication_delivery_metric(
    target: ConfigPublicationDeliveryTarget,
    result: ConfigPublicationDeliveryResult,
) -> Result<ConfigPublicationDeliveryMetricPlan, TelemetryAttributeError> {
    let target_label = TelemetryAttribute::metric_label(
        "config_publication_target",
        config_publication_delivery_target_label(target),
    );
    let result_label = TelemetryAttribute::metric_label(
        "config_publication_result",
        config_publication_delivery_result_label(result),
    );
    target_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ConfigPublicationDeliveryMetricPlan {
        metric_name: "prodex_config_publication_delivery_total",
        increment: 1,
        target_label,
        result_label,
    })
}

pub fn plan_config_cache_invalidation_metric(
    target: ConfigCacheInvalidationTarget,
    result: ConfigCacheInvalidationResult,
) -> Result<ConfigCacheInvalidationMetricPlan, TelemetryAttributeError> {
    let target_label = TelemetryAttribute::metric_label(
        "config_invalidation_target",
        config_cache_invalidation_target_label(target),
    );
    let result_label = TelemetryAttribute::metric_label(
        "config_invalidation_result",
        config_cache_invalidation_result_label(result),
    );
    target_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ConfigCacheInvalidationMetricPlan {
        metric_name: "prodex_config_cache_invalidation_events_total",
        increment: 1,
        target_label,
        result_label,
    })
}

pub fn plan_dropped_telemetry_metric(
    reason: TelemetryDropReason,
) -> Result<DroppedTelemetryMetricPlan, TelemetryAttributeError> {
    let reason_label = TelemetryAttribute::metric_label(
        "telemetry_drop_reason",
        telemetry_drop_reason_label(reason),
    );
    reason_label.as_metric_label()?;
    Ok(DroppedTelemetryMetricPlan {
        metric_name: "prodex_telemetry_dropped_total",
        increment: 1,
        reason_label,
    })
}

pub fn plan_queue_depth_metric(
    kind: QueueDepthKind,
    depth: u64,
    capacity: u64,
) -> Result<QueueDepthMetricPlan, TelemetryAttributeError> {
    let queue_label = TelemetryAttribute::metric_label("queue_kind", queue_depth_kind_label(kind));
    queue_label.as_metric_label()?;
    Ok(QueueDepthMetricPlan {
        metric_name: "prodex_queue_depth",
        depth,
        capacity,
        queue_label,
    })
}

pub fn plan_connection_pool_saturation_metric(
    kind: ConnectionPoolKind,
    in_use: u64,
    capacity: u64,
) -> Result<ConnectionPoolSaturationMetricPlan, TelemetryAttributeError> {
    let pool_label =
        TelemetryAttribute::metric_label("pool_kind", connection_pool_kind_label(kind));
    pool_label.as_metric_label()?;
    Ok(ConnectionPoolSaturationMetricPlan {
        metric_name: "prodex_connection_pool_in_use",
        in_use,
        capacity,
        pool_label,
    })
}

pub fn plan_api_red_metric(
    route: ApiRouteKind,
    status_class: ApiStatusClass,
    duration_ms: u64,
) -> Result<ApiRedMetricPlan, TelemetryAttributeError> {
    let route_label = TelemetryAttribute::metric_label("api_route", api_route_kind_label(route));
    let status_label =
        TelemetryAttribute::metric_label("status_class", api_status_class_label(status_class));
    route_label.as_metric_label()?;
    status_label.as_metric_label()?;
    Ok(ApiRedMetricPlan {
        request_count_metric_name: "prodex_api_requests_total",
        duration_metric_name: "prodex_api_request_duration_ms",
        increment: 1,
        duration_ms,
        route_label,
        status_label,
    })
}

pub fn plan_api_admission_metric(
    route: ApiRouteKind,
    result: ApiAdmissionResult,
) -> Result<ApiAdmissionMetricPlan, TelemetryAttributeError> {
    let route_label =
        TelemetryAttribute::metric_label("api_admission_route", api_route_kind_label(route));
    let result_label = TelemetryAttribute::metric_label(
        "api_admission_result",
        api_admission_result_label(result),
    );
    route_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiAdmissionMetricPlan {
        metric_name: "prodex_api_admission_decisions_total",
        increment: 1,
        route_label,
        result_label,
    })
}

pub fn plan_api_schema_validation_metric(
    surface: ApiSchemaSurface,
    result: ApiSchemaValidationResult,
) -> Result<ApiSchemaValidationMetricPlan, TelemetryAttributeError> {
    let surface_label =
        TelemetryAttribute::metric_label("api_schema_surface", api_schema_surface_label(surface));
    let result_label = TelemetryAttribute::metric_label(
        "api_schema_result",
        api_schema_validation_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiSchemaValidationMetricPlan {
        metric_name: "prodex_api_schema_validation_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_deprecation_metric(
    surface: ApiDeprecationSurface,
    signal: ApiDeprecationSignal,
) -> Result<ApiDeprecationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_deprecation_surface",
        api_deprecation_surface_label(surface),
    );
    let signal_label = TelemetryAttribute::metric_label(
        "api_deprecation_signal",
        api_deprecation_signal_label(signal),
    );
    surface_label.as_metric_label()?;
    signal_label.as_metric_label()?;
    Ok(ApiDeprecationMetricPlan {
        metric_name: "prodex_api_deprecation_events_total",
        increment: 1,
        surface_label,
        signal_label,
    })
}

pub fn plan_api_pagination_metric(
    surface: ApiPaginationSurface,
    result: ApiPaginationResult,
) -> Result<ApiPaginationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_pagination_surface",
        api_pagination_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_pagination_result",
        api_pagination_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiPaginationMetricPlan {
        metric_name: "prodex_api_pagination_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_precondition_metric(
    surface: ApiPreconditionSurface,
    result: ApiPreconditionResult,
) -> Result<ApiPreconditionMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_precondition_surface",
        api_precondition_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_precondition_result",
        api_precondition_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiPreconditionMetricPlan {
        metric_name: "prodex_api_precondition_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_idempotency_metric(
    surface: ApiIdempotencySurface,
    result: ApiIdempotencyResult,
) -> Result<ApiIdempotencyMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_idempotency_surface",
        api_idempotency_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_idempotency_result",
        api_idempotency_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiIdempotencyMetricPlan {
        metric_name: "prodex_api_idempotency_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_idempotency_record_metric(
    backend: IdempotencyRecordBackend,
    operation: IdempotencyRecordOperation,
    result: IdempotencyRecordResult,
) -> Result<IdempotencyRecordMetricPlan, TelemetryAttributeError> {
    let backend_label = TelemetryAttribute::metric_label(
        "idempotency_record_backend",
        idempotency_record_backend_label(backend),
    );
    let operation_label = TelemetryAttribute::metric_label(
        "idempotency_record_operation",
        idempotency_record_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "idempotency_record_result",
        idempotency_record_result_label(result),
    );
    backend_label.as_metric_label()?;
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(IdempotencyRecordMetricPlan {
        metric_name: "prodex_idempotency_record_events_total",
        increment: 1,
        backend_label,
        operation_label,
        result_label,
    })
}

pub fn plan_api_compatibility_metric(
    surface: ApiCompatibilitySurface,
    result: ApiCompatibilityResult,
) -> Result<ApiCompatibilityMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_compatibility_surface",
        api_compatibility_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_compatibility_result",
        api_compatibility_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiCompatibilityMetricPlan {
        metric_name: "prodex_api_compatibility_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_mutation_audit_metric(
    surface: ApiMutationAuditSurface,
    result: ApiMutationAuditResult,
) -> Result<ApiMutationAuditMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_mutation_audit_surface",
        api_mutation_audit_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_mutation_audit_result",
        api_mutation_audit_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiMutationAuditMetricPlan {
        metric_name: "prodex_api_mutation_audit_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_version_metric(
    surface: ApiVersionSurface,
    result: ApiVersionResult,
) -> Result<ApiVersionMetricPlan, TelemetryAttributeError> {
    let surface_label =
        TelemetryAttribute::metric_label("api_version_surface", api_version_surface_label(surface));
    let result_label =
        TelemetryAttribute::metric_label("api_version_result", api_version_result_label(result));
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiVersionMetricPlan {
        metric_name: "prodex_api_version_negotiation_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_spec_publication_metric(
    surface: ApiSpecSurface,
    result: ApiSpecPublicationResult,
) -> Result<ApiSpecPublicationMetricPlan, TelemetryAttributeError> {
    let surface_label =
        TelemetryAttribute::metric_label("api_spec_surface", api_spec_surface_label(surface));
    let result_label = TelemetryAttribute::metric_label(
        "api_spec_publication_result",
        api_spec_publication_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiSpecPublicationMetricPlan {
        metric_name: "prodex_api_spec_publication_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_error_envelope_metric(
    surface: ApiErrorEnvelopeSurface,
    result: ApiErrorEnvelopeResult,
) -> Result<ApiErrorEnvelopeMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_error_envelope_surface",
        api_error_envelope_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_error_envelope_result",
        api_error_envelope_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiErrorEnvelopeMetricPlan {
        metric_name: "prodex_api_error_envelope_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_body_limit_metric(
    surface: ApiBodyLimitSurface,
    result: ApiBodyLimitResult,
) -> Result<ApiBodyLimitMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_body_limit_surface",
        api_body_limit_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_body_limit_result",
        api_body_limit_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiBodyLimitMetricPlan {
        metric_name: "prodex_api_body_limit_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_timeout_budget_metric(
    surface: ApiTimeoutBudgetSurface,
    result: ApiTimeoutBudgetResult,
) -> Result<ApiTimeoutBudgetMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_timeout_budget_surface",
        api_timeout_budget_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_timeout_budget_result",
        api_timeout_budget_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiTimeoutBudgetMetricPlan {
        metric_name: "prodex_api_timeout_budget_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_cancellation_metric(
    surface: ApiCancellationSurface,
    source: ApiCancellationSource,
) -> Result<ApiCancellationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_cancellation_surface",
        api_cancellation_surface_label(surface),
    );
    let source_label = TelemetryAttribute::metric_label(
        "api_cancellation_source",
        api_cancellation_source_label(source),
    );
    surface_label.as_metric_label()?;
    source_label.as_metric_label()?;
    Ok(ApiCancellationMetricPlan {
        metric_name: "prodex_api_cancellation_events_total",
        increment: 1,
        surface_label,
        source_label,
    })
}

pub fn plan_api_stream_backpressure_metric(
    surface: ApiStreamBackpressureSurface,
    state: ApiStreamBackpressureState,
) -> Result<ApiStreamBackpressureMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_stream_backpressure_surface",
        api_stream_backpressure_surface_label(surface),
    );
    let state_label = TelemetryAttribute::metric_label(
        "api_stream_backpressure_state",
        api_stream_backpressure_state_label(state),
    );
    surface_label.as_metric_label()?;
    state_label.as_metric_label()?;
    Ok(ApiStreamBackpressureMetricPlan {
        metric_name: "prodex_api_stream_backpressure_events_total",
        increment: 1,
        surface_label,
        state_label,
    })
}

pub fn plan_provider_metric(
    provider: ProviderKind,
    result: ProviderResultClass,
    duration_ms: u64,
) -> Result<ProviderMetricPlan, TelemetryAttributeError> {
    let provider_label =
        TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let result_label =
        TelemetryAttribute::metric_label("provider_result", provider_result_class_label(result));
    provider_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ProviderMetricPlan {
        request_count_metric_name: "prodex_provider_requests_total",
        duration_metric_name: "prodex_provider_request_duration_ms",
        increment: 1,
        duration_ms,
        provider_label,
        result_label,
    })
}

pub fn plan_provider_capability_negotiation_metric(
    provider: ProviderKind,
    capability: ProviderCapabilityKind,
    result: ProviderCapabilityResult,
) -> Result<ProviderCapabilityNegotiationMetricPlan, TelemetryAttributeError> {
    let provider_label =
        TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let capability_label = TelemetryAttribute::metric_label(
        "provider_capability",
        provider_capability_kind_label(capability),
    );
    let result_label = TelemetryAttribute::metric_label(
        "provider_capability_result",
        provider_capability_result_label(result),
    );
    provider_label.as_metric_label()?;
    capability_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ProviderCapabilityNegotiationMetricPlan {
        metric_name: "prodex_provider_capability_negotiation_events_total",
        increment: 1,
        provider_label,
        capability_label,
        result_label,
    })
}

pub fn plan_provider_retry_metric(
    provider: ProviderKind,
    stage: ProviderRetryAttemptStage,
    outcome: ProviderRetryOutcome,
) -> Result<ProviderRetryMetricPlan, TelemetryAttributeError> {
    let provider_label =
        TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let stage_label = TelemetryAttribute::metric_label(
        "provider_retry_stage",
        provider_retry_attempt_stage_label(stage),
    );
    let outcome_label = TelemetryAttribute::metric_label(
        "provider_retry_outcome",
        provider_retry_outcome_label(outcome),
    );
    provider_label.as_metric_label()?;
    stage_label.as_metric_label()?;
    outcome_label.as_metric_label()?;
    Ok(ProviderRetryMetricPlan {
        metric_name: "prodex_provider_retry_events_total",
        increment: 1,
        provider_label,
        stage_label,
        outcome_label,
    })
}

pub fn plan_provider_circuit_breaker_metric(
    provider: ProviderKind,
    decision: ProviderCircuitBreakerDecision,
    event: ProviderCircuitBreakerEvent,
) -> Result<ProviderCircuitBreakerMetricPlan, TelemetryAttributeError> {
    let provider_label =
        TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let decision_label = TelemetryAttribute::metric_label(
        "provider_circuit_breaker_decision",
        provider_circuit_breaker_decision_label(decision),
    );
    let event_label = TelemetryAttribute::metric_label(
        "provider_circuit_breaker_event",
        provider_circuit_breaker_event_label(event),
    );
    provider_label.as_metric_label()?;
    decision_label.as_metric_label()?;
    event_label.as_metric_label()?;
    Ok(ProviderCircuitBreakerMetricPlan {
        metric_name: "prodex_provider_circuit_breaker_events_total",
        increment: 1,
        provider_label,
        decision_label,
        event_label,
    })
}

pub fn plan_provider_degradation_metric(
    provider: ProviderKind,
    signal: ProviderDegradationSignal,
    severity: ProviderDegradationSeverity,
) -> Result<ProviderDegradationMetricPlan, TelemetryAttributeError> {
    let provider_label =
        TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let signal_label = TelemetryAttribute::metric_label(
        "provider_degradation_signal",
        provider_degradation_signal_label(signal),
    );
    let severity_label = TelemetryAttribute::metric_label(
        "provider_degradation_severity",
        provider_degradation_severity_label(severity),
    );
    provider_label.as_metric_label()?;
    signal_label.as_metric_label()?;
    severity_label.as_metric_label()?;
    Ok(ProviderDegradationMetricPlan {
        metric_name: "prodex_provider_degradation_events_total",
        increment: 1,
        provider_label,
        signal_label,
        severity_label,
    })
}

pub fn plan_streaming_lifecycle_metric(
    transport: StreamTransportKind,
    outcome: StreamOutcome,
    duration_ms: u64,
) -> Result<StreamingLifecycleMetricPlan, TelemetryAttributeError> {
    let transport_label = TelemetryAttribute::metric_label(
        "stream_transport",
        stream_transport_kind_label(transport),
    );
    let outcome_label =
        TelemetryAttribute::metric_label("stream_outcome", stream_outcome_label(outcome));
    transport_label.as_metric_label()?;
    outcome_label.as_metric_label()?;
    Ok(StreamingLifecycleMetricPlan {
        event_count_metric_name: "prodex_streaming_lifecycle_total",
        duration_metric_name: "prodex_streaming_lifecycle_duration_ms",
        increment: 1,
        duration_ms,
        transport_label,
        outcome_label,
    })
}

pub fn plan_routing_decision_metric(
    lane: RoutingLaneKind,
    outcome: RoutingDecisionOutcome,
) -> Result<RoutingDecisionMetricPlan, TelemetryAttributeError> {
    let lane_label =
        TelemetryAttribute::metric_label("routing_lane", routing_lane_kind_label(lane));
    let outcome_label = TelemetryAttribute::metric_label(
        "routing_outcome",
        routing_decision_outcome_label(outcome),
    );
    lane_label.as_metric_label()?;
    outcome_label.as_metric_label()?;
    Ok(RoutingDecisionMetricPlan {
        metric_name: "prodex_routing_decisions_total",
        increment: 1,
        lane_label,
        outcome_label,
    })
}

pub fn plan_audit_metric(
    operation: AuditOperation,
    result: AuditResult,
) -> Result<AuditMetricPlan, TelemetryAttributeError> {
    let operation_label =
        TelemetryAttribute::metric_label("audit_operation", audit_operation_label(operation));
    let result_label = TelemetryAttribute::metric_label("audit_result", audit_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditMetricPlan {
        metric_name: "prodex_audit_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_query_lifecycle_metric(
    operation: AuditQueryLifecycleOperation,
    result: AuditQueryLifecycleResult,
) -> Result<AuditQueryLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "audit_query_operation",
        audit_query_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "audit_query_result",
        audit_query_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditQueryLifecycleMetricPlan {
        metric_name: "prodex_audit_query_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_chain_metric(
    operation: AuditChainOperation,
    result: AuditChainResult,
) -> Result<AuditChainMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "audit_chain_operation",
        audit_chain_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("audit_chain_result", audit_chain_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditChainMetricPlan {
        metric_name: "prodex_audit_chain_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_retention_purge_metric(
    operation: AuditRetentionPurgeOperation,
    result: AuditRetentionPurgeResult,
) -> Result<AuditRetentionPurgeMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "audit_retention_operation",
        audit_retention_purge_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "audit_retention_result",
        audit_retention_purge_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditRetentionPurgeMetricPlan {
        metric_name: "prodex_audit_retention_purge_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_security_decision_metric(
    decision: SecurityDecisionKind,
    result: SecurityDecisionResult,
) -> Result<SecurityDecisionMetricPlan, TelemetryAttributeError> {
    let decision_label = TelemetryAttribute::metric_label(
        "security_decision",
        security_decision_kind_label(decision),
    );
    let result_label =
        TelemetryAttribute::metric_label("security_result", security_decision_result_label(result));
    decision_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecurityDecisionMetricPlan {
        metric_name: "prodex_security_decisions_total",
        increment: 1,
        decision_label,
        result_label,
    })
}

pub fn plan_authn_token_validation_metric(
    stage: AuthnTokenValidationStage,
    result: AuthnTokenValidationResult,
) -> Result<AuthnTokenValidationMetricPlan, TelemetryAttributeError> {
    let stage_label = TelemetryAttribute::metric_label(
        "authn_validation_stage",
        authn_token_validation_stage_label(stage),
    );
    let result_label = TelemetryAttribute::metric_label(
        "authn_validation_result",
        authn_token_validation_result_label(result),
    );
    stage_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuthnTokenValidationMetricPlan {
        metric_name: "prodex_authn_token_validation_events_total",
        increment: 1,
        stage_label,
        result_label,
    })
}

pub fn plan_authz_decision_metric(
    boundary: AuthzBoundaryKind,
    result: AuthzDecisionResult,
) -> Result<AuthzDecisionMetricPlan, TelemetryAttributeError> {
    let boundary_label =
        TelemetryAttribute::metric_label("authz_boundary", authz_boundary_kind_label(boundary));
    let result_label =
        TelemetryAttribute::metric_label("authz_result", authz_decision_result_label(result));
    boundary_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuthzDecisionMetricPlan {
        metric_name: "prodex_authz_decisions_total",
        increment: 1,
        boundary_label,
        result_label,
    })
}

pub fn plan_credential_scope_mismatch_metric(
    direction: CredentialScopeMismatchDirection,
    result: CredentialScopeMismatchResult,
) -> Result<CredentialScopeMismatchMetricPlan, TelemetryAttributeError> {
    let direction_label = TelemetryAttribute::metric_label(
        "credential_scope_direction",
        credential_scope_mismatch_direction_label(direction),
    );
    let result_label = TelemetryAttribute::metric_label(
        "credential_scope_result",
        credential_scope_mismatch_result_label(result),
    );
    direction_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(CredentialScopeMismatchMetricPlan {
        metric_name: "prodex_credential_scope_mismatch_events_total",
        increment: 1,
        direction_label,
        result_label,
    })
}

pub fn plan_tenant_isolation_metric(
    surface: TenantIsolationSurface,
    result: TenantIsolationResult,
) -> Result<TenantIsolationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "tenant_isolation_surface",
        tenant_isolation_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "tenant_isolation_result",
        tenant_isolation_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TenantIsolationMetricPlan {
        metric_name: "prodex_tenant_isolation_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_postgres_tenant_context_metric(
    operation: PostgresTenantContextOperation,
    result: PostgresTenantContextResult,
) -> Result<PostgresTenantContextMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "postgres_tenant_context_operation",
        postgres_tenant_context_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "postgres_tenant_context_result",
        postgres_tenant_context_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PostgresTenantContextMetricPlan {
        metric_name: "prodex_postgres_tenant_context_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_identity_context_metric(
    surface: IdentityContextSurface,
    result: IdentityContextResult,
) -> Result<IdentityContextMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "identity_context_surface",
        identity_context_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "identity_context_result",
        identity_context_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(IdentityContextMetricPlan {
        metric_name: "prodex_identity_context_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_break_glass_lifecycle_metric(
    operation: BreakGlassLifecycleOperation,
    result: BreakGlassLifecycleResult,
) -> Result<BreakGlassLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "break_glass_operation",
        break_glass_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "break_glass_result",
        break_glass_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BreakGlassLifecycleMetricPlan {
        metric_name: "prodex_break_glass_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_user_lifecycle_metric(
    operation: UserLifecycleOperation,
    result: UserLifecycleResult,
) -> Result<UserLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "user_lifecycle_operation",
        user_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "user_lifecycle_result",
        user_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(UserLifecycleMetricPlan {
        metric_name: "prodex_user_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_service_identity_lifecycle_metric(
    operation: ServiceIdentityLifecycleOperation,
    result: ServiceIdentityLifecycleResult,
) -> Result<ServiceIdentityLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "service_identity_operation",
        service_identity_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "service_identity_result",
        service_identity_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ServiceIdentityLifecycleMetricPlan {
        metric_name: "prodex_service_identity_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_role_binding_lifecycle_metric(
    operation: RoleBindingLifecycleOperation,
    result: RoleBindingLifecycleResult,
) -> Result<RoleBindingLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "role_binding_operation",
        role_binding_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "role_binding_result",
        role_binding_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(RoleBindingLifecycleMetricPlan {
        metric_name: "prodex_role_binding_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_provider_credential_lifecycle_metric(
    operation: ProviderCredentialLifecycleOperation,
    result: ProviderCredentialLifecycleResult,
) -> Result<ProviderCredentialLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "provider_credential_operation",
        provider_credential_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "provider_credential_result",
        provider_credential_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ProviderCredentialLifecycleMetricPlan {
        metric_name: "prodex_provider_credential_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_virtual_key_lifecycle_metric(
    operation: VirtualKeyLifecycleOperation,
    result: VirtualKeyLifecycleResult,
) -> Result<VirtualKeyLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "credential_lifecycle_operation",
        virtual_key_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "credential_lifecycle_result",
        virtual_key_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(VirtualKeyLifecycleMetricPlan {
        metric_name: "prodex_virtual_key_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_budget_policy_lifecycle_metric(
    operation: BudgetPolicyLifecycleOperation,
    result: BudgetPolicyLifecycleResult,
) -> Result<BudgetPolicyLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "budget_policy_operation",
        budget_policy_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "budget_policy_result",
        budget_policy_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BudgetPolicyLifecycleMetricPlan {
        metric_name: "prodex_budget_policy_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_policy_lifecycle_metric(
    operation: PolicyLifecycleOperation,
    result: PolicyLifecycleResult,
) -> Result<PolicyLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "policy_lifecycle_operation",
        policy_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "policy_lifecycle_result",
        policy_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PolicyLifecycleMetricPlan {
        metric_name: "prodex_policy_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_tenant_lifecycle_metric(
    operation: TenantLifecycleOperation,
    result: TenantLifecycleResult,
) -> Result<TenantLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "account_lifecycle_operation",
        tenant_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "account_lifecycle_result",
        tenant_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TenantLifecycleMetricPlan {
        metric_name: "prodex_tenant_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_reservation_recovery_metric(
    operation: ReservationRecoveryOperation,
    result: ReservationRecoveryResult,
) -> Result<ReservationRecoveryMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "reservation_recovery_operation",
        reservation_recovery_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "reservation_recovery_result",
        reservation_recovery_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ReservationRecoveryMetricPlan {
        metric_name: "prodex_reservation_recovery_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_accounting_metric(
    operation: AccountingOperation,
    result: AccountingResult,
) -> Result<AccountingMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "accounting_operation",
        accounting_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("accounting_result", accounting_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AccountingMetricPlan {
        metric_name: "prodex_accounting_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_billing_ledger_metric(
    operation: BillingLedgerOperation,
    result: BillingLedgerResult,
) -> Result<BillingLedgerMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "billing_ledger_operation",
        billing_ledger_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "billing_ledger_result",
        billing_ledger_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BillingLedgerMetricPlan {
        metric_name: "prodex_billing_ledger_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_budget_rejection_metric(
    reason: BudgetRejectionReason,
) -> Result<BudgetRejectionMetricPlan, TelemetryAttributeError> {
    let reason_label = TelemetryAttribute::metric_label(
        "budget_rejection_reason",
        budget_rejection_reason_label(reason),
    );
    reason_label.as_metric_label()?;
    Ok(BudgetRejectionMetricPlan {
        metric_name: "prodex_budget_rejections_total",
        increment: 1,
        reason_label,
    })
}

pub fn plan_rate_limit_decision_metric(
    scope: RateLimitScope,
    decision: RateLimitDecision,
) -> Result<RateLimitDecisionMetricPlan, TelemetryAttributeError> {
    let scope_label =
        TelemetryAttribute::metric_label("rate_limit_scope", rate_limit_scope_label(scope));
    let decision_label = TelemetryAttribute::metric_label(
        "rate_limit_decision",
        rate_limit_decision_label(decision),
    );
    scope_label.as_metric_label()?;
    decision_label.as_metric_label()?;
    Ok(RateLimitDecisionMetricPlan {
        metric_name: "prodex_rate_limit_decisions_total",
        increment: 1,
        scope_label,
        decision_label,
    })
}

pub fn plan_redis_coordination_metric(
    operation: RedisCoordinationOperation,
    result: RedisCoordinationResult,
) -> Result<RedisCoordinationMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "redis_coordination_operation",
        redis_coordination_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "redis_coordination_result",
        redis_coordination_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(RedisCoordinationMetricPlan {
        metric_name: "prodex_redis_coordination_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_quota_correctness_metric(
    event: QuotaCorrectnessEvent,
) -> Result<QuotaCorrectnessMetricPlan, TelemetryAttributeError> {
    let event_label = TelemetryAttribute::metric_label(
        "quota_correctness_event",
        quota_correctness_event_label(event),
    );
    event_label.as_metric_label()?;
    Ok(QuotaCorrectnessMetricPlan {
        metric_name: "prodex_quota_correctness_events_total",
        increment: 1,
        event_label,
    })
}

pub fn plan_slo_alert_metric(
    sli: SloAlertSli,
    severity: SloAlertSeverity,
) -> Result<SloAlertMetricPlan, TelemetryAttributeError> {
    let sli_label = TelemetryAttribute::metric_label("slo_sli", slo_alert_sli_label(sli));
    let severity_label =
        TelemetryAttribute::metric_label("slo_severity", slo_alert_severity_label(severity));
    sli_label.as_metric_label()?;
    severity_label.as_metric_label()?;
    Ok(SloAlertMetricPlan {
        metric_name: "prodex_slo_alert_events_total",
        increment: 1,
        sli_label,
        severity_label,
    })
}

pub fn plan_shutdown_lifecycle_metric(
    event: ShutdownLifecycleEvent,
    result: ShutdownLifecycleResult,
) -> Result<ShutdownLifecycleMetricPlan, TelemetryAttributeError> {
    let event_label =
        TelemetryAttribute::metric_label("shutdown_event", shutdown_lifecycle_event_label(event));
    let result_label = TelemetryAttribute::metric_label(
        "shutdown_result",
        shutdown_lifecycle_result_label(result),
    );
    event_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ShutdownLifecycleMetricPlan {
        metric_name: "prodex_shutdown_lifecycle_total",
        increment: 1,
        event_label,
        result_label,
    })
}

pub fn plan_health_probe_metric(
    probe: HealthProbeKind,
    result: HealthProbeResult,
) -> Result<HealthProbeMetricPlan, TelemetryAttributeError> {
    let probe_label =
        TelemetryAttribute::metric_label("health_probe", health_probe_kind_label(probe));
    let result_label =
        TelemetryAttribute::metric_label("health_result", health_probe_result_label(result));
    probe_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(HealthProbeMetricPlan {
        metric_name: "prodex_health_probe_results_total",
        increment: 1,
        probe_label,
        result_label,
    })
}

pub fn plan_secret_provider_metric(
    backend: SecretProviderBackend,
    operation: SecretProviderOperation,
    result: SecretProviderResult,
) -> Result<SecretProviderMetricPlan, TelemetryAttributeError> {
    let backend_label =
        TelemetryAttribute::metric_label("secret_backend", secret_provider_backend_label(backend));
    let operation_label = TelemetryAttribute::metric_label(
        "secret_operation",
        secret_provider_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("secret_result", secret_provider_result_label(result));
    backend_label.as_metric_label()?;
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecretProviderMetricPlan {
        metric_name: "prodex_secret_provider_operations_total",
        increment: 1,
        backend_label,
        operation_label,
        result_label,
    })
}

pub fn plan_secret_rotation_metric(
    scope: SecretRotationScope,
    result: SecretRotationResult,
) -> Result<SecretRotationMetricPlan, TelemetryAttributeError> {
    let scope_label =
        TelemetryAttribute::metric_label("secret_scope", secret_rotation_scope_label(scope));
    let result_label = TelemetryAttribute::metric_label(
        "secret_rotation_result",
        secret_rotation_result_label(result),
    );
    scope_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecretRotationMetricPlan {
        metric_name: "prodex_secret_rotation_events_total",
        increment: 1,
        scope_label,
        result_label,
    })
}

pub fn plan_backup_restore_metric(
    operation: BackupRestoreOperation,
    result: BackupRestoreResult,
) -> Result<BackupRestoreMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "backup_restore_operation",
        backup_restore_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "backup_restore_result",
        backup_restore_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BackupRestoreMetricPlan {
        metric_name: "prodex_backup_restore_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_deployment_rollout_metric(
    operation: DeploymentRolloutOperation,
    result: DeploymentRolloutResult,
) -> Result<DeploymentRolloutMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "deployment_rollout_operation",
        deployment_rollout_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "deployment_rollout_result",
        deployment_rollout_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(DeploymentRolloutMetricPlan {
        metric_name: "prodex_deployment_rollout_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_load_soak_metric(
    scenario: LoadSoakScenarioKind,
    result: LoadSoakResult,
    duration_ms: u64,
) -> Result<LoadSoakMetricPlan, TelemetryAttributeError> {
    let scenario_label =
        TelemetryAttribute::metric_label("load_soak_scenario", load_soak_scenario_label(scenario));
    let result_label =
        TelemetryAttribute::metric_label("load_soak_result", load_soak_result_label(result));
    scenario_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(LoadSoakMetricPlan {
        event_count_metric_name: "prodex_load_soak_events_total",
        duration_metric_name: "prodex_load_soak_duration_ms",
        increment: 1,
        duration_ms,
        scenario_label,
        result_label,
    })
}

pub fn plan_fault_injection_metric(
    target: FaultInjectionTarget,
    result: FaultInjectionResult,
) -> Result<FaultInjectionMetricPlan, TelemetryAttributeError> {
    let target_label = TelemetryAttribute::metric_label(
        "fault_injection_target",
        fault_injection_target_label(target),
    );
    let result_label = TelemetryAttribute::metric_label(
        "fault_injection_result",
        fault_injection_result_label(result),
    );
    target_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(FaultInjectionMetricPlan {
        metric_name: "prodex_fault_injection_events_total",
        increment: 1,
        target_label,
        result_label,
    })
}

pub fn plan_migration_lifecycle_metric(
    operation: MigrationLifecycleOperation,
    result: MigrationLifecycleResult,
) -> Result<MigrationLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "migration_operation",
        migration_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "migration_result",
        migration_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(MigrationLifecycleMetricPlan {
        metric_name: "prodex_migration_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_persistence_metric(
    operation: PersistenceOperation,
    result: PersistenceResult,
) -> Result<PersistenceMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "persistence_operation",
        persistence_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("persistence_result", persistence_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PersistenceMetricPlan {
        metric_name: "prodex_persistence_operations_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

fn jwks_refresh_decision_label(decision: JwksRefreshDecision) -> &'static str {
    match decision {
        JwksRefreshDecision::UseFresh => "fresh",
        JwksRefreshDecision::RefreshNow => "refresh_now",
        JwksRefreshDecision::UseStaleWhileRevalidate => "stale_while_revalidate",
        JwksRefreshDecision::UseLastKnownGoodDuringBackoff => "last_known_good_backoff",
        JwksRefreshDecision::Unavailable => "unavailable",
    }
}

fn policy_refresh_decision_label(decision: PolicyRefreshDecision) -> &'static str {
    match decision {
        PolicyRefreshDecision::UseActive => "active",
        PolicyRefreshDecision::RefreshAsync => "refresh_async",
        PolicyRefreshDecision::UseLastKnownGoodAndRefresh => "last_known_good_refresh",
        PolicyRefreshDecision::Expired => "expired",
        PolicyRefreshDecision::Invalidated => "invalidated",
    }
}

fn jwks_refresh_outcome_label(outcome: JwksRefreshOutcome) -> &'static str {
    match outcome {
        JwksRefreshOutcome::Success => "success",
        JwksRefreshOutcome::Failure => "failure",
    }
}

fn oidc_refresh_operation_label(operation: OidcRefreshOperation) -> &'static str {
    match operation {
        OidcRefreshOperation::DiscoverIssuer => "discover_issuer",
        OidcRefreshOperation::FetchJwks => "fetch_jwks",
        OidcRefreshOperation::ValidateSnapshot => "validate_snapshot",
        OidcRefreshOperation::WriteCache => "write_cache",
    }
}

fn oidc_refresh_result_label(result: OidcRefreshResult) -> &'static str {
    match result {
        OidcRefreshResult::Success => "success",
        OidcRefreshResult::SkippedFresh => "skipped_fresh",
        OidcRefreshResult::Backoff => "backoff",
        OidcRefreshResult::InvalidSnapshot => "invalid_snapshot",
        OidcRefreshResult::Failed => "failed",
    }
}

fn enterprise_id_kind_label(kind: EnterpriseIdKind) -> &'static str {
    match kind {
        EnterpriseIdKind::Tenant => "tenant",
        EnterpriseIdKind::Principal => "principal",
        EnterpriseIdKind::Request => "request",
        EnterpriseIdKind::Call => "call",
        EnterpriseIdKind::Reservation => "reservation",
        EnterpriseIdKind::VirtualKey => "virtual_key",
        EnterpriseIdKind::PolicyRevision => "policy_revision",
        EnterpriseIdKind::AuditEvent => "audit_event",
    }
}

fn enterprise_id_result_label(result: EnterpriseIdResult) -> &'static str {
    match result {
        EnterpriseIdResult::Generated => "generated",
        EnterpriseIdResult::Parsed => "parsed",
        EnterpriseIdResult::Rejected => "rejected",
    }
}

fn policy_refresh_outcome_label(outcome: PolicyRefreshOutcome) -> &'static str {
    match outcome {
        PolicyRefreshOutcome::Success => "success",
        PolicyRefreshOutcome::Failure => "failure",
        PolicyRefreshOutcome::LastKnownGoodFallback => "last_known_good_fallback",
    }
}

fn policy_rollback_operation_label(operation: PolicyRollbackOperation) -> &'static str {
    match operation {
        PolicyRollbackOperation::ActivateLastKnownGood => "activate_last_known_good",
        PolicyRollbackOperation::RejectCandidate => "reject_candidate",
        PolicyRollbackOperation::Rollback => "rollback",
        PolicyRollbackOperation::Verify => "verify",
    }
}

fn policy_rollback_result_label(result: PolicyRollbackResult) -> &'static str {
    match result {
        PolicyRollbackResult::Success => "success",
        PolicyRollbackResult::Failed => "failed",
        PolicyRollbackResult::Blocked => "blocked",
        PolicyRollbackResult::Noop => "noop",
    }
}

fn config_activation_source_label(source: ConfigActivationSource) -> &'static str {
    match source {
        ConfigActivationSource::PublishedRevision => "published_revision",
        ConfigActivationSource::LastKnownGood => "last_known_good",
        ConfigActivationSource::Rollback => "rollback",
        ConfigActivationSource::InvalidationFallback => "invalidation_fallback",
    }
}

fn config_activation_result_label(result: ConfigActivationResult) -> &'static str {
    match result {
        ConfigActivationResult::Activated => "activated",
        ConfigActivationResult::Rejected => "rejected",
        ConfigActivationResult::MissingLastKnownGood => "missing_last_known_good",
        ConfigActivationResult::InvalidRevision => "invalid_revision",
    }
}

fn config_publication_delivery_target_label(
    target: ConfigPublicationDeliveryTarget,
) -> &'static str {
    match target {
        ConfigPublicationDeliveryTarget::GatewayCacheRefresh => "gateway_cache_refresh",
        ConfigPublicationDeliveryTarget::RuntimePolicyReload => "runtime_policy_reload",
        ConfigPublicationDeliveryTarget::AuditProjection => "audit_projection",
    }
}

fn config_publication_delivery_result_label(
    result: ConfigPublicationDeliveryResult,
) -> &'static str {
    match result {
        ConfigPublicationDeliveryResult::Delivered => "delivered",
        ConfigPublicationDeliveryResult::Failed => "failed",
        ConfigPublicationDeliveryResult::Skipped => "skipped",
        ConfigPublicationDeliveryResult::RetryScheduled => "retry_scheduled",
    }
}

fn config_cache_invalidation_target_label(target: ConfigCacheInvalidationTarget) -> &'static str {
    match target {
        ConfigCacheInvalidationTarget::GatewayPolicyCache => "gateway_policy_cache",
        ConfigCacheInvalidationTarget::RuntimePolicyCache => "runtime_policy_cache",
        ConfigCacheInvalidationTarget::RedisPolicyCache => "redis_policy_cache",
    }
}

fn config_cache_invalidation_result_label(result: ConfigCacheInvalidationResult) -> &'static str {
    match result {
        ConfigCacheInvalidationResult::Invalidated => "invalidated",
        ConfigCacheInvalidationResult::ReloadScheduled => "reload_scheduled",
        ConfigCacheInvalidationResult::NotFound => "not_found",
        ConfigCacheInvalidationResult::Failed => "failed",
    }
}

fn telemetry_drop_reason_label(reason: TelemetryDropReason) -> &'static str {
    match reason {
        TelemetryDropReason::QueueFull => "queue_full",
        TelemetryDropReason::ExporterUnavailable => "exporter_unavailable",
        TelemetryDropReason::Shutdown => "shutdown",
        TelemetryDropReason::InvalidPayload => "invalid_payload",
    }
}

fn queue_depth_kind_label(kind: QueueDepthKind) -> &'static str {
    match kind {
        QueueDepthKind::Responses => "responses",
        QueueDepthKind::Compact => "compact",
        QueueDepthKind::Websocket => "websocket",
        QueueDepthKind::Telemetry => "telemetry",
        QueueDepthKind::Persistence => "persistence",
    }
}

fn connection_pool_kind_label(kind: ConnectionPoolKind) -> &'static str {
    match kind {
        ConnectionPoolKind::Postgres => "postgres",
        ConnectionPoolKind::Redis => "redis",
        ConnectionPoolKind::ProviderHttp => "provider_http",
        ConnectionPoolKind::OidcHttp => "oidc_http",
    }
}

fn api_route_kind_label(route: ApiRouteKind) -> &'static str {
    match route {
        ApiRouteKind::Responses => "responses",
        ApiRouteKind::Compact => "compact",
        ApiRouteKind::Websocket => "websocket",
        ApiRouteKind::ControlPlane => "control_plane",
        ApiRouteKind::Health => "health",
    }
}

fn api_status_class_label(status_class: ApiStatusClass) -> &'static str {
    match status_class {
        ApiStatusClass::Informational => "1xx",
        ApiStatusClass::Success => "2xx",
        ApiStatusClass::Redirection => "3xx",
        ApiStatusClass::ClientError => "4xx",
        ApiStatusClass::ServerError => "5xx",
    }
}

fn api_admission_result_label(result: ApiAdmissionResult) -> &'static str {
    match result {
        ApiAdmissionResult::Accepted => "accepted",
        ApiAdmissionResult::GlobalLimitReached => "global_limit_reached",
        ApiAdmissionResult::RouteLimitReached => "route_limit_reached",
        ApiAdmissionResult::QueueFull => "queue_full",
        ApiAdmissionResult::Draining => "draining",
    }
}

fn api_schema_surface_label(surface: ApiSchemaSurface) -> &'static str {
    match surface {
        ApiSchemaSurface::Request => "request",
        ApiSchemaSurface::Response => "response",
        ApiSchemaSurface::OpenApi => "openapi",
        ApiSchemaSurface::ErrorEnvelope => "error_envelope",
    }
}

fn api_schema_validation_result_label(result: ApiSchemaValidationResult) -> &'static str {
    match result {
        ApiSchemaValidationResult::Valid => "valid",
        ApiSchemaValidationResult::Invalid => "invalid",
        ApiSchemaValidationResult::MissingSchema => "missing_schema",
        ApiSchemaValidationResult::Incompatible => "incompatible",
    }
}

fn api_deprecation_surface_label(surface: ApiDeprecationSurface) -> &'static str {
    match surface {
        ApiDeprecationSurface::DataPlane => "data_plane",
        ApiDeprecationSurface::ControlPlane => "control_plane",
        ApiDeprecationSurface::Scim => "scim",
        ApiDeprecationSurface::Health => "health",
    }
}

fn api_deprecation_signal_label(signal: ApiDeprecationSignal) -> &'static str {
    match signal {
        ApiDeprecationSignal::Notice => "notice",
        ApiDeprecationSignal::Sunset => "sunset",
        ApiDeprecationSignal::Rejected => "rejected",
    }
}

fn api_pagination_surface_label(surface: ApiPaginationSurface) -> &'static str {
    match surface {
        ApiPaginationSurface::ControlPlane => "control_plane",
        ApiPaginationSurface::Scim => "scim",
        ApiPaginationSurface::AuditExport => "audit_export",
        ApiPaginationSurface::Quota => "quota",
    }
}

fn api_pagination_result_label(result: ApiPaginationResult) -> &'static str {
    match result {
        ApiPaginationResult::PageReturned => "page_returned",
        ApiPaginationResult::EmptyPage => "empty_page",
        ApiPaginationResult::InvalidCursor => "invalid_cursor",
        ApiPaginationResult::ExpiredCursor => "expired_cursor",
    }
}

fn api_precondition_surface_label(surface: ApiPreconditionSurface) -> &'static str {
    match surface {
        ApiPreconditionSurface::Tenant => "tenant",
        ApiPreconditionSurface::Principal => "principal",
        ApiPreconditionSurface::VirtualKey => "virtual_key",
        ApiPreconditionSurface::Policy => "policy",
    }
}

fn api_precondition_result_label(result: ApiPreconditionResult) -> &'static str {
    match result {
        ApiPreconditionResult::Matched => "matched",
        ApiPreconditionResult::Missing => "missing",
        ApiPreconditionResult::Mismatched => "mismatched",
        ApiPreconditionResult::Invalid => "invalid",
    }
}

fn api_idempotency_surface_label(surface: ApiIdempotencySurface) -> &'static str {
    match surface {
        ApiIdempotencySurface::TenantMutation => "tenant_mutation",
        ApiIdempotencySurface::PrincipalMutation => "principal_mutation",
        ApiIdempotencySurface::VirtualKeyMutation => "virtual_key_mutation",
        ApiIdempotencySurface::PolicyMutation => "policy_mutation",
    }
}

fn api_idempotency_result_label(result: ApiIdempotencyResult) -> &'static str {
    match result {
        ApiIdempotencyResult::Accepted => "accepted",
        ApiIdempotencyResult::Replayed => "replayed",
        ApiIdempotencyResult::Conflict => "conflict",
        ApiIdempotencyResult::Missing => "missing",
        ApiIdempotencyResult::Invalid => "invalid",
    }
}

fn idempotency_record_backend_label(backend: IdempotencyRecordBackend) -> &'static str {
    match backend {
        IdempotencyRecordBackend::Postgres => "postgres",
        IdempotencyRecordBackend::Sqlite => "sqlite",
    }
}

fn idempotency_record_operation_label(operation: IdempotencyRecordOperation) -> &'static str {
    match operation {
        IdempotencyRecordOperation::PendingInsert => "pending_insert",
        IdempotencyRecordOperation::Complete => "complete",
        IdempotencyRecordOperation::Lookup => "lookup",
    }
}

fn idempotency_record_result_label(result: IdempotencyRecordResult) -> &'static str {
    match result {
        IdempotencyRecordResult::Recorded => "recorded",
        IdempotencyRecordResult::Replayed => "replayed",
        IdempotencyRecordResult::Conflict => "conflict",
        IdempotencyRecordResult::NotFound => "not_found",
        IdempotencyRecordResult::Failed => "failed",
    }
}

fn api_compatibility_surface_label(surface: ApiCompatibilitySurface) -> &'static str {
    match surface {
        ApiCompatibilitySurface::DataPlane => "data_plane",
        ApiCompatibilitySurface::ControlPlane => "control_plane",
        ApiCompatibilitySurface::Scim => "scim",
        ApiCompatibilitySurface::ErrorEnvelope => "error_envelope",
    }
}

fn api_compatibility_result_label(result: ApiCompatibilityResult) -> &'static str {
    match result {
        ApiCompatibilityResult::Compatible => "compatible",
        ApiCompatibilityResult::AdditiveChange => "additive_change",
        ApiCompatibilityResult::DeprecatedChange => "deprecated_change",
        ApiCompatibilityResult::BreakingChange => "breaking_change",
    }
}

fn api_mutation_audit_surface_label(surface: ApiMutationAuditSurface) -> &'static str {
    match surface {
        ApiMutationAuditSurface::Tenant => "tenant",
        ApiMutationAuditSurface::Principal => "principal",
        ApiMutationAuditSurface::VirtualKey => "virtual_key",
        ApiMutationAuditSurface::Policy => "policy",
    }
}

fn api_mutation_audit_result_label(result: ApiMutationAuditResult) -> &'static str {
    match result {
        ApiMutationAuditResult::Required => "required",
        ApiMutationAuditResult::Persisted => "persisted",
        ApiMutationAuditResult::Missing => "missing",
        ApiMutationAuditResult::Failed => "failed",
    }
}

fn api_version_surface_label(surface: ApiVersionSurface) -> &'static str {
    match surface {
        ApiVersionSurface::DataPlane => "data_plane",
        ApiVersionSurface::ControlPlane => "control_plane",
        ApiVersionSurface::Scim => "scim",
        ApiVersionSurface::Health => "health",
    }
}

fn api_version_result_label(result: ApiVersionResult) -> &'static str {
    match result {
        ApiVersionResult::Accepted => "accepted",
        ApiVersionResult::Defaulted => "defaulted",
        ApiVersionResult::Deprecated => "deprecated",
        ApiVersionResult::Unsupported => "unsupported",
    }
}

fn api_spec_surface_label(surface: ApiSpecSurface) -> &'static str {
    match surface {
        ApiSpecSurface::GatewayOpenApi => "gateway_openapi",
        ApiSpecSurface::ControlPlaneOpenApi => "control_plane_openapi",
        ApiSpecSurface::ScimSchema => "scim_schema",
        ApiSpecSurface::ErrorEnvelope => "error_envelope",
    }
}

fn api_spec_publication_result_label(result: ApiSpecPublicationResult) -> &'static str {
    match result {
        ApiSpecPublicationResult::Generated => "generated",
        ApiSpecPublicationResult::Validated => "validated",
        ApiSpecPublicationResult::Published => "published",
        ApiSpecPublicationResult::Rejected => "rejected",
    }
}

fn api_error_envelope_surface_label(surface: ApiErrorEnvelopeSurface) -> &'static str {
    match surface {
        ApiErrorEnvelopeSurface::DataPlane => "data_plane",
        ApiErrorEnvelopeSurface::ControlPlane => "control_plane",
        ApiErrorEnvelopeSurface::Scim => "scim",
        ApiErrorEnvelopeSurface::Health => "health",
    }
}

fn api_error_envelope_result_label(result: ApiErrorEnvelopeResult) -> &'static str {
    match result {
        ApiErrorEnvelopeResult::Emitted => "emitted",
        ApiErrorEnvelopeResult::Redacted => "redacted",
        ApiErrorEnvelopeResult::ValidationFailed => "validation_failed",
        ApiErrorEnvelopeResult::CompatibilityRejected => "compatibility_rejected",
    }
}

fn api_body_limit_surface_label(surface: ApiBodyLimitSurface) -> &'static str {
    match surface {
        ApiBodyLimitSurface::DataPlane => "data_plane",
        ApiBodyLimitSurface::ControlPlane => "control_plane",
        ApiBodyLimitSurface::Scim => "scim",
        ApiBodyLimitSurface::Upload => "upload",
    }
}

fn api_body_limit_result_label(result: ApiBodyLimitResult) -> &'static str {
    match result {
        ApiBodyLimitResult::Accepted => "accepted",
        ApiBodyLimitResult::RejectedTooLarge => "rejected_too_large",
        ApiBodyLimitResult::UnknownLength => "unknown_length",
        ApiBodyLimitResult::Truncated => "truncated",
    }
}

fn api_timeout_budget_surface_label(surface: ApiTimeoutBudgetSurface) -> &'static str {
    match surface {
        ApiTimeoutBudgetSurface::DataPlane => "data_plane",
        ApiTimeoutBudgetSurface::ControlPlane => "control_plane",
        ApiTimeoutBudgetSurface::Provider => "provider",
        ApiTimeoutBudgetSurface::Persistence => "persistence",
    }
}

fn api_timeout_budget_result_label(result: ApiTimeoutBudgetResult) -> &'static str {
    match result {
        ApiTimeoutBudgetResult::Accepted => "accepted",
        ApiTimeoutBudgetResult::Expired => "expired",
        ApiTimeoutBudgetResult::Exhausted => "exhausted",
        ApiTimeoutBudgetResult::Cancelled => "cancelled",
    }
}

fn api_cancellation_surface_label(surface: ApiCancellationSurface) -> &'static str {
    match surface {
        ApiCancellationSurface::DataPlane => "data_plane",
        ApiCancellationSurface::ControlPlane => "control_plane",
        ApiCancellationSurface::ProviderStream => "provider_stream",
        ApiCancellationSurface::Persistence => "persistence",
    }
}

fn api_cancellation_source_label(source: ApiCancellationSource) -> &'static str {
    match source {
        ApiCancellationSource::ClientDisconnect => "client_disconnect",
        ApiCancellationSource::TimeoutBudget => "timeout_budget",
        ApiCancellationSource::ShutdownDrain => "shutdown_drain",
        ApiCancellationSource::UpstreamAbort => "upstream_abort",
    }
}

fn api_stream_backpressure_surface_label(surface: ApiStreamBackpressureSurface) -> &'static str {
    match surface {
        ApiStreamBackpressureSurface::DataPlaneStream => "data_plane_stream",
        ApiStreamBackpressureSurface::ProviderStream => "provider_stream",
        ApiStreamBackpressureSurface::Websocket => "websocket",
        ApiStreamBackpressureSurface::AuditExport => "audit_export",
    }
}

fn api_stream_backpressure_state_label(state: ApiStreamBackpressureState) -> &'static str {
    match state {
        ApiStreamBackpressureState::Ready => "ready",
        ApiStreamBackpressureState::Paused => "paused",
        ApiStreamBackpressureState::Dropped => "dropped",
        ApiStreamBackpressureState::Closed => "closed",
    }
}

fn provider_kind_label(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::OpenAi => "openai",
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::Gemini => "gemini",
        ProviderKind::Local => "local",
        ProviderKind::Other => "other",
    }
}

fn provider_result_class_label(result: ProviderResultClass) -> &'static str {
    match result {
        ProviderResultClass::Success => "success",
        ProviderResultClass::RateLimited => "rate_limited",
        ProviderResultClass::Overloaded => "overloaded",
        ProviderResultClass::ProviderError => "provider_error",
        ProviderResultClass::TransportError => "transport_error",
    }
}

fn provider_capability_kind_label(capability: ProviderCapabilityKind) -> &'static str {
    match capability {
        ProviderCapabilityKind::ResponsesApi => "responses_api",
        ProviderCapabilityKind::Streaming => "streaming",
        ProviderCapabilityKind::Tools => "tools",
        ProviderCapabilityKind::Vision => "vision",
        ProviderCapabilityKind::JsonMode => "json_mode",
        ProviderCapabilityKind::RemoteCompact => "remote_compact",
        ProviderCapabilityKind::WebSocket => "websocket",
    }
}

fn provider_capability_result_label(result: ProviderCapabilityResult) -> &'static str {
    match result {
        ProviderCapabilityResult::Compatible => "compatible",
        ProviderCapabilityResult::Incompatible => "incompatible",
        ProviderCapabilityResult::NoCandidate => "no_candidate",
    }
}

fn provider_retry_attempt_stage_label(stage: ProviderRetryAttemptStage) -> &'static str {
    match stage {
        ProviderRetryAttemptStage::BeforeDispatch => "before_dispatch",
        ProviderRetryAttemptStage::BeforeFirstByte => "before_first_byte",
        ProviderRetryAttemptStage::AfterFirstByte => "after_first_byte",
        ProviderRetryAttemptStage::AfterCancellation => "after_cancellation",
    }
}

fn provider_retry_outcome_label(outcome: ProviderRetryOutcome) -> &'static str {
    match outcome {
        ProviderRetryOutcome::Allowed => "allowed",
        ProviderRetryOutcome::DeniedCommitted => "denied_committed",
        ProviderRetryOutcome::DeniedBudgetExhausted => "denied_budget_exhausted",
    }
}

fn provider_circuit_breaker_decision_label(
    decision: ProviderCircuitBreakerDecision,
) -> &'static str {
    match decision {
        ProviderCircuitBreakerDecision::Closed => "closed",
        ProviderCircuitBreakerDecision::Open => "open",
        ProviderCircuitBreakerDecision::HalfOpenProbe => "half_open_probe",
    }
}

fn provider_circuit_breaker_event_label(event: ProviderCircuitBreakerEvent) -> &'static str {
    match event {
        ProviderCircuitBreakerEvent::Success => "success",
        ProviderCircuitBreakerEvent::Failure => "failure",
        ProviderCircuitBreakerEvent::Probe => "probe",
    }
}

fn provider_degradation_signal_label(signal: ProviderDegradationSignal) -> &'static str {
    match signal {
        ProviderDegradationSignal::ErrorRate => "error_rate",
        ProviderDegradationSignal::Latency => "latency",
        ProviderDegradationSignal::Overload => "overload",
        ProviderDegradationSignal::Transport => "transport",
        ProviderDegradationSignal::CircuitOpen => "circuit_open",
    }
}

fn provider_degradation_severity_label(severity: ProviderDegradationSeverity) -> &'static str {
    match severity {
        ProviderDegradationSeverity::Warning => "warning",
        ProviderDegradationSeverity::Critical => "critical",
        ProviderDegradationSeverity::Recovered => "recovered",
    }
}

fn stream_transport_kind_label(transport: StreamTransportKind) -> &'static str {
    match transport {
        StreamTransportKind::Responses => "responses",
        StreamTransportKind::Websocket => "websocket",
    }
}

fn stream_outcome_label(outcome: StreamOutcome) -> &'static str {
    match outcome {
        StreamOutcome::Completed => "completed",
        StreamOutcome::Cancelled => "cancelled",
        StreamOutcome::Interrupted => "interrupted",
        StreamOutcome::GuardrailBlocked => "guardrail_blocked",
    }
}

fn routing_lane_kind_label(lane: RoutingLaneKind) -> &'static str {
    match lane {
        RoutingLaneKind::Responses => "responses",
        RoutingLaneKind::Compact => "compact",
        RoutingLaneKind::Websocket => "websocket",
        RoutingLaneKind::ControlPlane => "control_plane",
    }
}

fn routing_decision_outcome_label(outcome: RoutingDecisionOutcome) -> &'static str {
    match outcome {
        RoutingDecisionOutcome::Selected => "selected",
        RoutingDecisionOutcome::Fallback => "fallback",
        RoutingDecisionOutcome::Rejected => "rejected",
        RoutingDecisionOutcome::NoCandidate => "no_candidate",
    }
}

fn audit_operation_label(operation: AuditOperation) -> &'static str {
    match operation {
        AuditOperation::Emit => "emit",
        AuditOperation::Persist => "persist",
        AuditOperation::Export => "export",
    }
}

fn audit_result_label(result: AuditResult) -> &'static str {
    match result {
        AuditResult::Success => "success",
        AuditResult::Failure => "failure",
        AuditResult::Dropped => "dropped",
    }
}

fn audit_query_lifecycle_operation_label(operation: AuditQueryLifecycleOperation) -> &'static str {
    match operation {
        AuditQueryLifecycleOperation::PlanQuery => "plan_query",
        AuditQueryLifecycleOperation::PageQuery => "page_query",
        AuditQueryLifecycleOperation::PlanExport => "plan_export",
        AuditQueryLifecycleOperation::SerializeExport => "serialize_export",
    }
}

fn audit_query_lifecycle_result_label(result: AuditQueryLifecycleResult) -> &'static str {
    match result {
        AuditQueryLifecycleResult::Planned => "planned",
        AuditQueryLifecycleResult::PageReturned => "page_returned",
        AuditQueryLifecycleResult::Empty => "empty",
        AuditQueryLifecycleResult::Denied => "denied",
        AuditQueryLifecycleResult::Failed => "failed",
    }
}

fn audit_chain_operation_label(operation: AuditChainOperation) -> &'static str {
    match operation {
        AuditChainOperation::Append => "append",
        AuditChainOperation::VerifyLink => "verify_link",
        AuditChainOperation::VerifyRange => "verify_range",
        AuditChainOperation::ExportProof => "export_proof",
    }
}

fn audit_chain_result_label(result: AuditChainResult) -> &'static str {
    match result {
        AuditChainResult::Success => "success",
        AuditChainResult::Conflict => "conflict",
        AuditChainResult::DigestInvalid => "digest_invalid",
        AuditChainResult::GapDetected => "gap_detected",
        AuditChainResult::Failed => "failed",
    }
}

fn audit_retention_purge_operation_label(operation: AuditRetentionPurgeOperation) -> &'static str {
    match operation {
        AuditRetentionPurgeOperation::SelectCandidates => "select_candidates",
        AuditRetentionPurgeOperation::ApplyLegalHold => "apply_legal_hold",
        AuditRetentionPurgeOperation::DeleteBatch => "delete_batch",
        AuditRetentionPurgeOperation::VerifyChain => "verify_chain",
    }
}

fn audit_retention_purge_result_label(result: AuditRetentionPurgeResult) -> &'static str {
    match result {
        AuditRetentionPurgeResult::Success => "success",
        AuditRetentionPurgeResult::Protected => "protected",
        AuditRetentionPurgeResult::Empty => "empty",
        AuditRetentionPurgeResult::Failed => "failed",
    }
}

fn security_decision_kind_label(decision: SecurityDecisionKind) -> &'static str {
    match decision {
        SecurityDecisionKind::Authentication => "authentication",
        SecurityDecisionKind::TenantResolution => "tenant_resolution",
        SecurityDecisionKind::Authorization => "authorization",
        SecurityDecisionKind::CredentialScope => "credential_scope",
    }
}

fn security_decision_result_label(result: SecurityDecisionResult) -> &'static str {
    match result {
        SecurityDecisionResult::Allowed => "allowed",
        SecurityDecisionResult::Denied => "denied",
        SecurityDecisionResult::Error => "error",
    }
}

fn authn_token_validation_stage_label(stage: AuthnTokenValidationStage) -> &'static str {
    match stage {
        AuthnTokenValidationStage::Decode => "decode",
        AuthnTokenValidationStage::Signature => "signature",
        AuthnTokenValidationStage::Claims => "claims",
        AuthnTokenValidationStage::TenantClaim => "tenant_claim",
        AuthnTokenValidationStage::RoleClaim => "role_claim",
        AuthnTokenValidationStage::JwksCache => "jwks_cache",
    }
}

fn authn_token_validation_result_label(result: AuthnTokenValidationResult) -> &'static str {
    match result {
        AuthnTokenValidationResult::Accepted => "accepted",
        AuthnTokenValidationResult::Malformed => "malformed",
        AuthnTokenValidationResult::InvalidSignature => "invalid_signature",
        AuthnTokenValidationResult::Expired => "expired",
        AuthnTokenValidationResult::UnknownKey => "unknown_key",
        AuthnTokenValidationResult::MissingTenant => "missing_tenant",
        AuthnTokenValidationResult::RoleDenied => "role_denied",
        AuthnTokenValidationResult::CacheUnavailable => "cache_unavailable",
    }
}

fn authz_boundary_kind_label(boundary: AuthzBoundaryKind) -> &'static str {
    match boundary {
        AuthzBoundaryKind::DataPlaneInference => "data_plane_inference",
        AuthzBoundaryKind::DataPlaneQuota => "data_plane_quota",
        AuthzBoundaryKind::ControlPlaneRead => "control_plane_read",
        AuthzBoundaryKind::ControlPlaneMutation => "control_plane_mutation",
        AuthzBoundaryKind::ControlPlaneBilling => "control_plane_billing",
        AuthzBoundaryKind::BreakGlass => "break_glass",
    }
}

fn authz_decision_result_label(result: AuthzDecisionResult) -> &'static str {
    match result {
        AuthzDecisionResult::Allowed => "allowed",
        AuthzDecisionResult::CredentialScopeDenied => "credential_scope_denied",
        AuthzDecisionResult::RoleDenied => "role_denied",
        AuthzDecisionResult::TenantDenied => "tenant_denied",
        AuthzDecisionResult::ResourceDenied => "resource_denied",
        AuthzDecisionResult::Failed => "failed",
    }
}

fn credential_scope_mismatch_direction_label(
    direction: CredentialScopeMismatchDirection,
) -> &'static str {
    match direction {
        CredentialScopeMismatchDirection::DataPlaneToControlPlane => "data_plane_to_control_plane",
        CredentialScopeMismatchDirection::ControlPlaneToDataPlane => "control_plane_to_data_plane",
        CredentialScopeMismatchDirection::BreakGlassToDataPlane => "break_glass_to_data_plane",
        CredentialScopeMismatchDirection::BreakGlassToControlPlane => {
            "break_glass_to_control_plane"
        }
        CredentialScopeMismatchDirection::MissingCredential => "missing_credential",
    }
}

fn credential_scope_mismatch_result_label(result: CredentialScopeMismatchResult) -> &'static str {
    match result {
        CredentialScopeMismatchResult::Rejected => "rejected",
        CredentialScopeMismatchResult::Audited => "audited",
        CredentialScopeMismatchResult::Failed => "failed",
    }
}

fn tenant_isolation_surface_label(surface: TenantIsolationSurface) -> &'static str {
    match surface {
        TenantIsolationSurface::Authentication => "authentication",
        TenantIsolationSurface::Authorization => "authorization",
        TenantIsolationSurface::StoragePredicate => "storage_predicate",
        TenantIsolationSurface::CacheKey => "cache_key",
        TenantIsolationSurface::AuditQuery => "audit_query",
    }
}

fn tenant_isolation_result_label(result: TenantIsolationResult) -> &'static str {
    match result {
        TenantIsolationResult::Enforced => "enforced",
        TenantIsolationResult::CrossTenantDenied => "cross_tenant_denied",
        TenantIsolationResult::MissingTenantDenied => "missing_tenant_denied",
        TenantIsolationResult::MismatchRejected => "mismatch_rejected",
        TenantIsolationResult::Failed => "failed",
    }
}

fn postgres_tenant_context_operation_label(
    operation: PostgresTenantContextOperation,
) -> &'static str {
    match operation {
        PostgresTenantContextOperation::SetContext => "set_context",
        PostgresTenantContextOperation::VerifyContext => "verify_context",
        PostgresTenantContextOperation::ApplyRlsPolicy => "apply_rls_policy",
        PostgresTenantContextOperation::ExecuteTenantDml => "execute_tenant_dml",
    }
}

fn postgres_tenant_context_result_label(result: PostgresTenantContextResult) -> &'static str {
    match result {
        PostgresTenantContextResult::Applied => "applied",
        PostgresTenantContextResult::Missing => "missing",
        PostgresTenantContextResult::MismatchRejected => "mismatch_rejected",
        PostgresTenantContextResult::RlsDenied => "rls_denied",
        PostgresTenantContextResult::Failed => "failed",
    }
}

fn identity_context_surface_label(surface: IdentityContextSurface) -> &'static str {
    match surface {
        IdentityContextSurface::Authentication => "authentication",
        IdentityContextSurface::Authorization => "authorization",
        IdentityContextSurface::Audit => "audit",
        IdentityContextSurface::ControlPlane => "control_plane",
        IdentityContextSurface::DataPlane => "data_plane",
    }
}

fn identity_context_result_label(result: IdentityContextResult) -> &'static str {
    match result {
        IdentityContextResult::Consistent => "consistent",
        IdentityContextResult::MissingPrincipal => "missing_principal",
        IdentityContextResult::MissingTenant => "missing_tenant",
        IdentityContextResult::TenantMismatch => "tenant_mismatch",
        IdentityContextResult::CorrelationMissing => "correlation_missing",
        IdentityContextResult::Failed => "failed",
    }
}

fn break_glass_lifecycle_operation_label(operation: BreakGlassLifecycleOperation) -> &'static str {
    match operation {
        BreakGlassLifecycleOperation::Request => "request",
        BreakGlassLifecycleOperation::Approve => "approve",
        BreakGlassLifecycleOperation::Activate => "activate",
        BreakGlassLifecycleOperation::Revoke => "revoke",
        BreakGlassLifecycleOperation::Expire => "expire",
    }
}

fn break_glass_lifecycle_result_label(result: BreakGlassLifecycleResult) -> &'static str {
    match result {
        BreakGlassLifecycleResult::Authorized => "authorized",
        BreakGlassLifecycleResult::Denied => "denied",
        BreakGlassLifecycleResult::Persisted => "persisted",
        BreakGlassLifecycleResult::Expired => "expired",
        BreakGlassLifecycleResult::Failed => "failed",
    }
}

fn user_lifecycle_operation_label(operation: UserLifecycleOperation) -> &'static str {
    match operation {
        UserLifecycleOperation::Invite => "invite",
        UserLifecycleOperation::ScimCreate => "scim_create",
        UserLifecycleOperation::ScimUpdate => "scim_update",
        UserLifecycleOperation::ScimDelete => "scim_delete",
    }
}

fn user_lifecycle_result_label(result: UserLifecycleResult) -> &'static str {
    match result {
        UserLifecycleResult::Authorized => "authorized",
        UserLifecycleResult::Denied => "denied",
        UserLifecycleResult::Persisted => "persisted",
        UserLifecycleResult::Failed => "failed",
    }
}

fn service_identity_lifecycle_operation_label(
    operation: ServiceIdentityLifecycleOperation,
) -> &'static str {
    match operation {
        ServiceIdentityLifecycleOperation::Create => "create",
        ServiceIdentityLifecycleOperation::RotateSecret => "rotate_secret",
        ServiceIdentityLifecycleOperation::Disable => "disable",
    }
}

fn service_identity_lifecycle_result_label(result: ServiceIdentityLifecycleResult) -> &'static str {
    match result {
        ServiceIdentityLifecycleResult::Authorized => "authorized",
        ServiceIdentityLifecycleResult::Denied => "denied",
        ServiceIdentityLifecycleResult::Persisted => "persisted",
        ServiceIdentityLifecycleResult::Failed => "failed",
    }
}

fn role_binding_lifecycle_operation_label(
    operation: RoleBindingLifecycleOperation,
) -> &'static str {
    match operation {
        RoleBindingLifecycleOperation::Grant => "grant",
        RoleBindingLifecycleOperation::Revoke => "revoke",
    }
}

fn role_binding_lifecycle_result_label(result: RoleBindingLifecycleResult) -> &'static str {
    match result {
        RoleBindingLifecycleResult::Authorized => "authorized",
        RoleBindingLifecycleResult::Denied => "denied",
        RoleBindingLifecycleResult::Persisted => "persisted",
        RoleBindingLifecycleResult::Failed => "failed",
    }
}

fn provider_credential_lifecycle_operation_label(
    operation: ProviderCredentialLifecycleOperation,
) -> &'static str {
    match operation {
        ProviderCredentialLifecycleOperation::Rotate => "rotate",
        ProviderCredentialLifecycleOperation::ValidateReference => "validate_reference",
        ProviderCredentialLifecycleOperation::PersistReference => "persist_reference",
    }
}

fn provider_credential_lifecycle_result_label(
    result: ProviderCredentialLifecycleResult,
) -> &'static str {
    match result {
        ProviderCredentialLifecycleResult::Authorized => "authorized",
        ProviderCredentialLifecycleResult::Denied => "denied",
        ProviderCredentialLifecycleResult::Persisted => "persisted",
        ProviderCredentialLifecycleResult::Failed => "failed",
    }
}

fn virtual_key_lifecycle_operation_label(operation: VirtualKeyLifecycleOperation) -> &'static str {
    match operation {
        VirtualKeyLifecycleOperation::Create => "create",
        VirtualKeyLifecycleOperation::RotateSecret => "rotate_secret",
        VirtualKeyLifecycleOperation::PersistReference => "persist_reference",
    }
}

fn virtual_key_lifecycle_result_label(result: VirtualKeyLifecycleResult) -> &'static str {
    match result {
        VirtualKeyLifecycleResult::Authorized => "authorized",
        VirtualKeyLifecycleResult::Denied => "denied",
        VirtualKeyLifecycleResult::Persisted => "persisted",
        VirtualKeyLifecycleResult::Failed => "failed",
    }
}

fn budget_policy_lifecycle_operation_label(
    operation: BudgetPolicyLifecycleOperation,
) -> &'static str {
    match operation {
        BudgetPolicyLifecycleOperation::Update => "update",
        BudgetPolicyLifecycleOperation::ValidateScope => "validate_scope",
        BudgetPolicyLifecycleOperation::PersistPolicy => "persist_policy",
    }
}

fn budget_policy_lifecycle_result_label(result: BudgetPolicyLifecycleResult) -> &'static str {
    match result {
        BudgetPolicyLifecycleResult::Authorized => "authorized",
        BudgetPolicyLifecycleResult::Denied => "denied",
        BudgetPolicyLifecycleResult::Persisted => "persisted",
        BudgetPolicyLifecycleResult::Failed => "failed",
    }
}

fn policy_lifecycle_operation_label(operation: PolicyLifecycleOperation) -> &'static str {
    match operation {
        PolicyLifecycleOperation::Create => "create",
        PolicyLifecycleOperation::Update => "update",
        PolicyLifecycleOperation::Publish => "publish",
        PolicyLifecycleOperation::Invalidate => "invalidate",
    }
}

fn policy_lifecycle_result_label(result: PolicyLifecycleResult) -> &'static str {
    match result {
        PolicyLifecycleResult::Authorized => "authorized",
        PolicyLifecycleResult::Denied => "denied",
        PolicyLifecycleResult::Persisted => "persisted",
        PolicyLifecycleResult::Published => "published",
        PolicyLifecycleResult::Failed => "failed",
    }
}

fn tenant_lifecycle_operation_label(operation: TenantLifecycleOperation) -> &'static str {
    match operation {
        TenantLifecycleOperation::Create => "create",
        TenantLifecycleOperation::Update => "update",
    }
}

fn tenant_lifecycle_result_label(result: TenantLifecycleResult) -> &'static str {
    match result {
        TenantLifecycleResult::Authorized => "authorized",
        TenantLifecycleResult::Denied => "denied",
        TenantLifecycleResult::Persisted => "persisted",
        TenantLifecycleResult::Failed => "failed",
    }
}

fn reservation_recovery_operation_label(operation: ReservationRecoveryOperation) -> &'static str {
    match operation {
        ReservationRecoveryOperation::ScanExpired => "scan_expired",
        ReservationRecoveryOperation::AcquireLease => "acquire_lease",
        ReservationRecoveryOperation::ReleaseBudget => "release_budget",
        ReservationRecoveryOperation::WriteLedger => "write_ledger",
    }
}

fn reservation_recovery_result_label(result: ReservationRecoveryResult) -> &'static str {
    match result {
        ReservationRecoveryResult::Recovered => "recovered",
        ReservationRecoveryResult::Skipped => "skipped",
        ReservationRecoveryResult::LeaseUnavailable => "lease_unavailable",
        ReservationRecoveryResult::Failed => "failed",
    }
}

fn accounting_operation_label(operation: AccountingOperation) -> &'static str {
    match operation {
        AccountingOperation::Reservation => "reservation",
        AccountingOperation::Commit => "commit",
        AccountingOperation::Release => "release",
        AccountingOperation::Expire => "expire",
        AccountingOperation::Reconciliation => "reconciliation",
        AccountingOperation::BudgetRejection => "budget_rejection",
    }
}

fn accounting_result_label(result: AccountingResult) -> &'static str {
    match result {
        AccountingResult::Accepted => "accepted",
        AccountingResult::Rejected => "rejected",
        AccountingResult::Committed => "committed",
        AccountingResult::Released => "released",
        AccountingResult::Expired => "expired",
        AccountingResult::Reconciled => "reconciled",
        AccountingResult::Failed => "failed",
    }
}

fn billing_ledger_operation_label(operation: BillingLedgerOperation) -> &'static str {
    match operation {
        BillingLedgerOperation::ReserveAppend => "reserve_append",
        BillingLedgerOperation::CommitAppend => "commit_append",
        BillingLedgerOperation::ReleaseAppend => "release_append",
        BillingLedgerOperation::ReconciliationAppend => "reconciliation_append",
        BillingLedgerOperation::Query => "query",
    }
}

fn billing_ledger_result_label(result: BillingLedgerResult) -> &'static str {
    match result {
        BillingLedgerResult::Written => "written",
        BillingLedgerResult::Read => "read",
        BillingLedgerResult::Skipped => "skipped",
        BillingLedgerResult::Failed => "failed",
    }
}

fn budget_rejection_reason_label(reason: BudgetRejectionReason) -> &'static str {
    match reason {
        BudgetRejectionReason::TenantBudgetExceeded => "tenant_budget_exceeded",
        BudgetRejectionReason::VirtualKeyBudgetExceeded => "virtual_key_budget_exceeded",
        BudgetRejectionReason::RateLimited => "rate_limited",
        BudgetRejectionReason::ReservationUnavailable => "reservation_unavailable",
        BudgetRejectionReason::PolicyDenied => "policy_denied",
    }
}

fn rate_limit_scope_label(scope: RateLimitScope) -> &'static str {
    match scope {
        RateLimitScope::Tenant => "tenant",
        RateLimitScope::VirtualKey => "virtual_key",
        RateLimitScope::Principal => "principal",
        RateLimitScope::Provider => "provider",
    }
}

fn rate_limit_decision_label(decision: RateLimitDecision) -> &'static str {
    match decision {
        RateLimitDecision::Allowed => "allowed",
        RateLimitDecision::Delayed => "delayed",
        RateLimitDecision::Rejected => "rejected",
        RateLimitDecision::Unavailable => "unavailable",
    }
}

fn redis_coordination_operation_label(operation: RedisCoordinationOperation) -> &'static str {
    match operation {
        RedisCoordinationOperation::RateLimitCheck => "rate_limit_check",
        RedisCoordinationOperation::RateLimitCommit => "rate_limit_commit",
        RedisCoordinationOperation::RecoveryLeaseAcquire => "recovery_lease_acquire",
        RedisCoordinationOperation::RecoveryLeaseRelease => "recovery_lease_release",
        RedisCoordinationOperation::CacheRead => "cache_read",
        RedisCoordinationOperation::CacheWrite => "cache_write",
    }
}

fn redis_coordination_result_label(result: RedisCoordinationResult) -> &'static str {
    match result {
        RedisCoordinationResult::Success => "success",
        RedisCoordinationResult::Limited => "limited",
        RedisCoordinationResult::LeaseUnavailable => "lease_unavailable",
        RedisCoordinationResult::CacheMiss => "cache_miss",
        RedisCoordinationResult::Unavailable => "unavailable",
        RedisCoordinationResult::Failed => "failed",
    }
}

fn quota_correctness_event_label(event: QuotaCorrectnessEvent) -> &'static str {
    match event {
        QuotaCorrectnessEvent::ReservationOvershoot => "reservation_overshoot",
        QuotaCorrectnessEvent::DuplicateChargePrevented => "duplicate_charge_prevented",
        QuotaCorrectnessEvent::MissingCommitRecovered => "missing_commit_recovered",
        QuotaCorrectnessEvent::MissingReleaseRecovered => "missing_release_recovered",
        QuotaCorrectnessEvent::LedgerMismatchDetected => "ledger_mismatch_detected",
    }
}

fn slo_alert_sli_label(sli: SloAlertSli) -> &'static str {
    match sli {
        SloAlertSli::Availability => "availability",
        SloAlertSli::LatencyP95 => "latency_p95",
        SloAlertSli::ErrorRate => "error_rate",
        SloAlertSli::QuotaCorrectness => "quota_correctness",
        SloAlertSli::ProviderDegradation => "provider_degradation",
        SloAlertSli::PersistenceFailure => "persistence_failure",
    }
}

fn slo_alert_severity_label(severity: SloAlertSeverity) -> &'static str {
    match severity {
        SloAlertSeverity::Warning => "warning",
        SloAlertSeverity::Critical => "critical",
    }
}

fn shutdown_lifecycle_event_label(event: ShutdownLifecycleEvent) -> &'static str {
    match event {
        ShutdownLifecycleEvent::SignalReceived => "signal_received",
        ShutdownLifecycleEvent::DrainingStarted => "draining_started",
        ShutdownLifecycleEvent::ReadinessDisabled => "readiness_disabled",
        ShutdownLifecycleEvent::InflightDrained => "inflight_drained",
        ShutdownLifecycleEvent::TimeoutElapsed => "timeout_elapsed",
        ShutdownLifecycleEvent::Completed => "completed",
    }
}

fn shutdown_lifecycle_result_label(result: ShutdownLifecycleResult) -> &'static str {
    match result {
        ShutdownLifecycleResult::Success => "success",
        ShutdownLifecycleResult::Timeout => "timeout",
        ShutdownLifecycleResult::Forced => "forced",
        ShutdownLifecycleResult::Failed => "failed",
    }
}

fn health_probe_kind_label(probe: HealthProbeKind) -> &'static str {
    match probe {
        HealthProbeKind::Live => "live",
        HealthProbeKind::Ready => "ready",
        HealthProbeKind::Startup => "startup",
    }
}

fn health_probe_result_label(result: HealthProbeResult) -> &'static str {
    match result {
        HealthProbeResult::Passing => "passing",
        HealthProbeResult::Degraded => "degraded",
        HealthProbeResult::Failing => "failing",
        HealthProbeResult::Draining => "draining",
    }
}

fn secret_provider_backend_label(backend: SecretProviderBackend) -> &'static str {
    match backend {
        SecretProviderBackend::File => "file",
        SecretProviderBackend::Keyring => "keyring",
        SecretProviderBackend::ExternalManager => "external_manager",
    }
}

fn secret_provider_operation_label(operation: SecretProviderOperation) -> &'static str {
    match operation {
        SecretProviderOperation::Read => "read",
        SecretProviderOperation::Write => "write",
        SecretProviderOperation::Delete => "delete",
        SecretProviderOperation::RevisionLookup => "revision_lookup",
    }
}

fn secret_provider_result_label(result: SecretProviderResult) -> &'static str {
    match result {
        SecretProviderResult::Success => "success",
        SecretProviderResult::NotFound => "not_found",
        SecretProviderResult::Unsupported => "unsupported",
        SecretProviderResult::Failed => "failed",
    }
}

fn secret_rotation_scope_label(scope: SecretRotationScope) -> &'static str {
    match scope {
        SecretRotationScope::ProviderCredential => "provider_credential",
        SecretRotationScope::OidcClient => "oidc_client",
        SecretRotationScope::SigningKey => "signing_key",
        SecretRotationScope::StorageCredential => "storage_credential",
        SecretRotationScope::WebhookSecret => "webhook_secret",
    }
}

fn secret_rotation_result_label(result: SecretRotationResult) -> &'static str {
    match result {
        SecretRotationResult::Success => "success",
        SecretRotationResult::Failed => "failed",
        SecretRotationResult::Skipped => "skipped",
        SecretRotationResult::Rollback => "rollback",
    }
}

fn backup_restore_operation_label(operation: BackupRestoreOperation) -> &'static str {
    match operation {
        BackupRestoreOperation::Backup => "backup",
        BackupRestoreOperation::Restore => "restore",
        BackupRestoreOperation::Verify => "verify",
        BackupRestoreOperation::Drill => "drill",
    }
}

fn backup_restore_result_label(result: BackupRestoreResult) -> &'static str {
    match result {
        BackupRestoreResult::Success => "success",
        BackupRestoreResult::Failed => "failed",
        BackupRestoreResult::Partial => "partial",
        BackupRestoreResult::Skipped => "skipped",
    }
}

fn deployment_rollout_operation_label(operation: DeploymentRolloutOperation) -> &'static str {
    match operation {
        DeploymentRolloutOperation::Apply => "apply",
        DeploymentRolloutOperation::Verify => "verify",
        DeploymentRolloutOperation::Promote => "promote",
        DeploymentRolloutOperation::Rollback => "rollback",
    }
}

fn deployment_rollout_result_label(result: DeploymentRolloutResult) -> &'static str {
    match result {
        DeploymentRolloutResult::Success => "success",
        DeploymentRolloutResult::Failed => "failed",
        DeploymentRolloutResult::Degraded => "degraded",
        DeploymentRolloutResult::Skipped => "skipped",
    }
}

fn load_soak_scenario_label(scenario: LoadSoakScenarioKind) -> &'static str {
    match scenario {
        LoadSoakScenarioKind::Load => "load",
        LoadSoakScenarioKind::Soak => "soak",
        LoadSoakScenarioKind::Spike => "spike",
        LoadSoakScenarioKind::Recovery => "recovery",
    }
}

fn load_soak_result_label(result: LoadSoakResult) -> &'static str {
    match result {
        LoadSoakResult::Passed => "passed",
        LoadSoakResult::Failed => "failed",
        LoadSoakResult::Aborted => "aborted",
        LoadSoakResult::ThresholdBreached => "threshold_breached",
    }
}

fn fault_injection_target_label(target: FaultInjectionTarget) -> &'static str {
    match target {
        FaultInjectionTarget::Postgres => "postgres",
        FaultInjectionTarget::Redis => "redis",
        FaultInjectionTarget::Idp => "idp",
        FaultInjectionTarget::Provider => "provider",
    }
}

fn fault_injection_result_label(result: FaultInjectionResult) -> &'static str {
    match result {
        FaultInjectionResult::Injected => "injected",
        FaultInjectionResult::Recovered => "recovered",
        FaultInjectionResult::Failed => "failed",
        FaultInjectionResult::Skipped => "skipped",
    }
}

fn migration_lifecycle_operation_label(operation: MigrationLifecycleOperation) -> &'static str {
    match operation {
        MigrationLifecycleOperation::StatusCheck => "status_check",
        MigrationLifecycleOperation::CompatibilityCheck => "compatibility_check",
        MigrationLifecycleOperation::Apply => "apply",
        MigrationLifecycleOperation::Rollback => "rollback",
    }
}

fn migration_lifecycle_result_label(result: MigrationLifecycleResult) -> &'static str {
    match result {
        MigrationLifecycleResult::Compatible => "compatible",
        MigrationLifecycleResult::Applied => "applied",
        MigrationLifecycleResult::Blocked => "blocked",
        MigrationLifecycleResult::Failed => "failed",
        MigrationLifecycleResult::RolledBack => "rolled_back",
    }
}

fn persistence_operation_label(operation: PersistenceOperation) -> &'static str {
    match operation {
        PersistenceOperation::Read => "read",
        PersistenceOperation::Write => "write",
        PersistenceOperation::Commit => "commit",
        PersistenceOperation::Rollback => "rollback",
        PersistenceOperation::HealthCheck => "health_check",
    }
}

fn persistence_result_label(result: PersistenceResult) -> &'static str {
    match result {
        PersistenceResult::Success => "success",
        PersistenceResult::Conflict => "conflict",
        PersistenceResult::Timeout => "timeout",
        PersistenceResult::Unavailable => "unavailable",
        PersistenceResult::Failed => "failed",
    }
}
