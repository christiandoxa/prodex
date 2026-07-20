#![forbid(unsafe_code)]
//! Pure Prodex domain types.
//!
//! This crate intentionally stays independent from HTTP frameworks, CLI,
//! database drivers, filesystem access, provider SDKs, and network clients.

mod accounting;
mod accounting_budget;
mod api;
mod audit;
mod backup;
mod capabilities;
mod correlation;
mod deployment;
mod errors;
mod governance;
mod health;
mod idempotency;
mod identity;
mod ids;
mod migration;
mod observability;
mod policy;
mod rate_limit;
mod secrets;
mod security;
mod slo;

pub use ids::{
    AuditEventId, CallId, IdParseError, IdParseErrorResponsePlan, IdParseErrorStatus,
    PolicyRevisionId, PrincipalId, ProviderCredentialId, RequestId, ReservationId, RoleBindingId,
    TenantId, VirtualKeyId, plan_id_parse_error_response,
};

pub use security::{
    AuthorizationError, AuthorizationRequirement, CredentialScope, ExplicitRoleMapper, Principal,
    PrincipalKind, ResourceAction, ResourceAuthorizationError, ResourceKind, Role, RoleClaimError,
    SecurityErrorResponsePlan, SecurityErrorStatus, TenantAccessError, TenantContext, TenantMode,
    TenantResolutionError, TenantScopedResource, authorize_min_role, authorize_resource_action,
    authorize_scope, authorize_tenant_access, plan_domain_authorization_error_response,
    plan_resource_authorization_error_response, plan_role_claim_error_response,
    plan_tenant_access_error_response, plan_tenant_resolution_error_response,
};

pub use accounting::{
    AccountingErrorResponsePlan, AccountingErrorStatus, BudgetRejection, BudgetRejectionReason,
    BudgetSnapshot, LedgerEvent, LedgerEventKind, ReservationCommit, ReservationCommitError,
    ReservationCommitMismatch, ReservationReconciliation, ReservationReconciliationError,
    ReservationReconciliationReason, ReservationRecord, ReservationRecoveryError,
    ReservationRequest, UsageAmount, commit_reservation, commit_reservation_checked,
    plan_budget_rejection_response, plan_reservation_commit_error_response,
    plan_reservation_commit_mismatch_response, plan_reservation_reconciliation_error_response,
    plan_reservation_recovery_error_response, reconcile_reserved_usage,
    release_expired_reservation, reserve_budget, validate_reservation_commit,
};
pub use accounting_budget::BudgetLimit;
pub use secrets::{
    SecretErrorResponsePlan, SecretErrorStatus, SecretMaterial, SecretProvider,
    SecretProviderDescriptor, SecretProviderKind, SecretPurpose, SecretRef, SecretResolutionError,
    SecretResolutionRequest, SecretRotationPolicy, SecretRotationPolicyError, SecretRotationStatus,
    plan_secret_resolution_error_response, plan_secret_rotation_policy_error_response,
};

pub use policy::{
    PolicyActivationError, PolicyActivationState, PolicyAuditAction, PolicyAuditRecord,
    PolicyCacheStatus, PolicyDigest, PolicyErrorResponsePlan, PolicyErrorStatus,
    PolicyRefreshDecision, PolicyRefreshWindow, PolicyRefreshWindowError, PolicySignature,
    PolicySnapshot, PolicyValidation, ValidatedPolicySnapshot, evaluate_policy_refresh,
    plan_policy_activation_error_response, plan_policy_refresh_decision_error_response,
    plan_policy_refresh_window_error_response, validate_policy_snapshot,
};

pub use audit::{
    AuditAction, AuditActionError, AuditChainError, AuditDigest, AuditDigestError, AuditEnvelope,
    AuditErrorResponsePlan, AuditErrorStatus, AuditEvent, AuditExportFormat,
    AuditExportFormatError, AuditExportPlan, AuditOutcome, AuditOutcomeError, AuditPageLimit,
    AuditPageLimitError, AuditQueryCursor, AuditQueryCursorError, AuditQueryPageError,
    AuditQueryPlan, AuditQueryPlanError, AuditQueryScope, AuditQueryScopeError, AuditReasonCode,
    AuditReasonCodeError, AuditResource, AuditResourceId, AuditResourceIdError,
    AuditResourceKindError, AuditRetentionBatchLimit, AuditRetentionBatchLimitError,
    AuditRetentionDecision, AuditRetentionDecisionError, AuditRetentionHold,
    AuditRetentionHoldError, AuditRetentionPageError, AuditRetentionPlan, AuditRetentionPlanError,
    AuditRetentionPolicy, AuditRetentionPolicyError, AuditRetentionPurgeBatch,
    AuditRetentionPurgeBatchError, AuditRetentionPurgeKey, AuditSortOrder, AuditSortOrderError,
    AuditTimeRange, AuditTimeRangeError, AuditTimestamp, AuditTimestampError,
    compute_audit_chain_digest, plan_audit_action_error_response, plan_audit_chain_error_response,
    plan_audit_digest_error_response, plan_audit_export_format_error_response,
    plan_audit_outcome_error_response, plan_audit_page_limit_error_response,
    plan_audit_query_cursor_error_response, plan_audit_query_page_error_response,
    plan_audit_query_plan_error_response, plan_audit_query_scope_error_response,
    plan_audit_reason_code_error_response, plan_audit_resource_id_error_response,
    plan_audit_resource_kind_error_response, plan_audit_retention_batch_limit_error_response,
    plan_audit_retention_decision_error_response, plan_audit_retention_hold_error_response,
    plan_audit_retention_page_error_response, plan_audit_retention_plan_error_response,
    plan_audit_retention_policy_error_response, plan_audit_retention_purge_batch_error_response,
    plan_audit_sort_order_error_response, plan_audit_time_range_error_response,
    plan_audit_timestamp_error_response, sha256_checksum,
};

pub use idempotency::{
    IdempotencyConflict, IdempotencyConflictResponsePlan, IdempotencyConflictStatus,
    IdempotencyDecision, IdempotencyEntry, IdempotencyKey, IdempotencyKeyError,
    IdempotencyKeyErrorResponsePlan, IdempotencyKeyErrorStatus, IdempotencyRecord,
    IdempotencyReplayDecision, IdempotentOperation, IdempotentOperationError, decide_idempotency,
    decide_idempotency_replay, plan_idempotency_conflict_response,
    plan_idempotency_key_error_response,
};

pub use errors::{
    ErrorCategory, ErrorCode, ErrorCodeError, ErrorCodeErrorResponsePlan, ErrorCodeErrorStatus,
    ErrorEnvelope, ErrorMetadata, plan_error_code_error_response,
};

pub use api::{
    ApiVersion, ApiVersionDecision, ApiVersionError, ApiVersionErrorResponsePlan,
    ApiVersionErrorStatus, ApiVersionPolicy, ApiVersionStatus, ConcurrencyError,
    ConcurrencyErrorResponsePlan, ConcurrencyErrorStatus, Cursor, CursorError,
    CursorErrorResponsePlan, CursorErrorStatus, EntityTag, EntityTagError,
    EntityTagErrorResponsePlan, EntityTagErrorStatus, Page, PageRequest, ResourceVersion,
    ResourceVersionError, ResourceVersionErrorResponsePlan, ResourceVersionErrorStatus,
    evaluate_api_version, plan_api_version_error_response, plan_concurrency_error_response,
    plan_cursor_error_response, plan_entity_tag_error_response,
    plan_resource_version_error_response, require_matching_etag, require_matching_version,
};

pub use health::{
    HealthCheck, HealthProbeKind, HealthProbeResponsePlan, HealthSnapshot, HealthState,
    plan_health_probe_response,
};

pub use governance::{
    ApprovalAction, ApprovalError, ApprovalFingerprint, ApprovalId, ApprovalKind,
    ApprovalReasonCode, ApprovalRecord, ApprovalScope, ApprovalState, ApprovalTransition,
    ApprovalTransitionRequest, ApprovalVote, AuditDetailLevel, BreakGlassPolicyContext,
    CanonicalRoute, Channel, ClassificationDecision, ClassificationError, ClassificationReasonCode,
    ClassificationRequest, ClassificationRule, ClassificationRuleSet,
    ClassificationRuleSetChecksum, ClassificationRuleSetRevisionId, CompiledClassificationRuleSet,
    CompiledGovernancePolicy, ContentLocation, DataClassification, DataModality, DataPolicyContext,
    DetectorId, DetectorRevisionId, EnvironmentContext, ExecutionApprovalBinding, FindingKind,
    GovernanceObligation, GovernancePolicyArtifact, GovernancePolicyError, GovernancePolicyRule,
    GovernancePolicyRuleId, GovernedAction, HIGH_RISK_MINIMUM_APPROVAL_QUORUM, InspectionCoverage,
    InspectionFinding, InspectionLimits, InspectionModelError, InspectionReasonCode,
    InspectionResult, InspectionTag, MAX_APPROVAL_QUORUM, MAX_CLASSIFICATION_REASON_CODES,
    MAX_CLASSIFICATION_RULES, MAX_CONTENT_LOCATION_PATH_BYTES, MAX_GOVERNANCE_POLICY_RULES,
    MAX_INSPECTION_DETECTORS, MAX_INSPECTION_FINDINGS, MAX_INSPECTION_REASON_CODES,
    MAX_INSPECTION_TAGS, MAX_INSPECTION_TOKEN_BYTES, MAX_POLICY_OBLIGATIONS,
    MAX_POLICY_PRINCIPAL_GROUPS, MAX_POLICY_REASON_CODES, NetworkZone, PolicyDecision,
    PolicyEffect, PolicyInput, PolicyReasonCode, PolicyRuleCondition, PolicySelector,
    PrincipalPolicyAttributes, ProviderTrustTier, QuotaContext, RequestPolicyAttributes,
    RequestRisk, SessionPolicyContext, classify_inspection, compile_classification_rule_set,
    compile_governance_policy, evaluate_governance_policy, execution_approval_id,
    transition_approval,
};

pub use rate_limit::{
    RateLimitAllowance, RateLimitAtomicUpdate, RateLimitAtomicUpdateError, RateLimitBucketKey,
    RateLimitDecision, RateLimitErrorResponsePlan, RateLimitErrorStatus, RateLimitRejection,
    RateLimitRequest, RateLimitRule, RateLimitSnapshot, evaluate_rate_limit,
    plan_rate_limit_atomic_update_error_response, plan_rate_limit_decision_error_response,
};

pub use capabilities::{
    CapabilityDecision, CapabilityErrorResponsePlan, CapabilityErrorStatus, CapabilityRequest,
    CapabilitySet, ModelCapability, ModelRouteCandidate, negotiate_capability,
    plan_capability_decision_error_response,
};

pub use correlation::{
    CorrelationContext, TraceId, TraceIdError, TraceIdErrorResponsePlan, TraceIdErrorStatus,
    plan_trace_id_error_response,
};

pub use migration::{
    MigrationCompatibilityError, MigrationCompatibilityErrorResponsePlan,
    MigrationCompatibilityErrorStatus, MigrationCompatibilityWindow, MigrationExecutionMode,
    MigrationPlan, MigrationPlanError, MigrationPlanErrorResponsePlan, MigrationPlanErrorStatus,
    MigrationStep, MigrationStepKind, MigrationStepState, MigrationVersion, MigrationVersionError,
    MigrationVersionErrorResponsePlan, MigrationVersionErrorStatus,
    plan_migration_compatibility_error_response, plan_migration_plan_error_response,
    plan_migration_version_error_response, validate_expand_contract_order,
    validate_migration_compatibility,
};

pub use observability::{
    GatewaySpanDescriptor, GatewaySpanKind, TelemetryAttribute, TelemetryAttributeError,
    TelemetryAttributeErrorResponsePlan, TelemetryAttributeErrorStatus, TelemetryAttributeScope,
    plan_telemetry_attribute_error_response, tenant_trace_attribute,
};

pub use backup::{
    BackupId, BackupIdError, BackupIdErrorResponsePlan, BackupIdErrorStatus, BackupSnapshot,
    BackupStatus, RestorePlan, RestorePlanError, RestorePlanErrorResponsePlan,
    RestorePlanErrorStatus, plan_backup_id_error_response, plan_restore_error_response,
};

pub use slo::{
    AlertDecision, AlertSeverity, SliKind, SliMeasurement, SloAlertResponsePlan,
    SloAlertResponseStatus, SloObjective, ThresholdDirection, evaluate_slo,
    minimum_enterprise_slo_objectives, plan_slo_alert_response,
};

pub use identity::{
    Audience, IdentityConfigError, IdentityErrorResponsePlan, IdentityErrorStatus, Issuer,
    JwksCacheSnapshot, JwksRefreshDecision, JwtAlgorithm, OidcValidationPolicy,
    TokenValidationError, evaluate_jwks_refresh, plan_identity_config_error_response,
    plan_jwks_refresh_decision_error_response, plan_token_validation_error_response,
};

pub use deployment::{
    ContainerSecurityPolicy, DeploymentReadinessPlan, DeploymentSecurityErrorResponsePlan,
    DeploymentSecurityErrorStatus, DeploymentSecurityReport, DeploymentViolation,
    KubernetesArtifactSet, ProductionReadinessTopology, evaluate_deployment_security,
    plan_deployment_security_error_response, plan_production_deployment_readiness,
};
