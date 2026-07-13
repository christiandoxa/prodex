use prodex_domain::TelemetryAttribute;

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
