use prodex_domain::TelemetryAttribute;

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
