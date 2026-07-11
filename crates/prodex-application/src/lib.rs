#![forbid(unsafe_code)]
//! Application use-case orchestration boundary.
//!
//! This crate composes domain/gateway/control-plane/storage contracts into
//! side-effect-free use-case plans. It intentionally has no CLI, filesystem,
//! HTTP framework, async runtime, database driver, network client, or provider
//! SDK dependency. `prodex-app` and future binaries should become composition
//! roots that adapt real transports and storage clients into these plans.

pub mod distributed_rate_limit;

mod accounting;
mod auth;
mod config;
mod control_plane;
mod data_plane;
mod provider;
mod request_context;

pub use accounting::*;
pub use auth::*;
pub use config::*;
pub use control_plane::*;
pub use data_plane::*;
pub use provider::*;
pub use request_context::{
    ApplicationAuthenticatedRequestContext, ApplicationAuthorizedRequestContext,
    ApplicationRequestAuthorizationError, ApplicationRequestContext,
    ApplicationRequestContextError, plan_application_control_plane_authorization,
    plan_application_control_plane_authorization_from_compatibility,
    plan_application_data_plane_authorization,
    plan_application_data_plane_authorization_from_compatibility,
    plan_application_request_authentication_from_compatibility,
    plan_application_request_authentication_from_evidence, plan_application_request_context,
};

use std::error::Error;
use std::fmt;

use prodex_authn::{
    AuthenticationErrorResponsePlan, TokenClaims, authenticate_oidc_claims,
    plan_authentication_error_response,
};
use prodex_config::{
    ConfigActivationPlan, ConfigCacheState, ConfigPublicationError,
    ConfigPublicationErrorResponsePlan, ConfigPublicationErrorStatus,
    ConfigPublicationEventDelivery, ConfigPublicationEventError, ConfigPublicationEventPlan,
    ConfigRefreshDecision, evaluate_config_refresh, plan_config_activation,
    plan_config_publication_error_response as plan_config_activation_error_response,
    plan_config_publication_event, plan_config_publication_event_error_response,
};
use prodex_control_plane::{
    BreakGlassAuthorization, ConfigurationPublicationDecision,
    ConfigurationPublicationErrorResponsePlan, ConfigurationPublicationErrorStatus,
    ConfigurationPublicationRequest, ControlPlaneActionRequest, ControlPlaneAuditWritePlan,
    ControlPlaneDecision, ControlPlaneOperation, decide_break_glass_action,
    decide_configuration_publication, decide_control_plane_action,
    plan_configuration_publication_error_response,
};
use prodex_domain::{
    AuditDigest, CallId, CapabilityErrorResponsePlan, CapabilityRequest, CorrelationContext,
    CredentialScope, EntityTag, ExplicitRoleMapper, GatewaySpanKind, HealthCheck, HealthSnapshot,
    HealthState, IdempotencyConflict, IdempotencyConflictStatus, IdempotencyEntry, IdempotencyKey,
    IdempotencyReplayDecision, IdempotentOperation, IdempotentOperationError, JwksCacheSnapshot,
    OidcValidationPolicy, PageRequest, Principal, RequestId, ResourceKind, TelemetryAttribute,
    TenantId, decide_idempotency_replay, plan_idempotency_conflict_response,
    tenant_trace_attribute,
};
use prodex_gateway_core::{
    GatewayAdmissionErrorResponsePlan, GatewayAdmissionPlan, GatewayAdmissionRequest,
    GatewayExpiredReservationRecoveryErrorResponsePlan,
    GatewayExpiredReservationRecoveryErrorStatus, GatewayExpiredReservationRecoveryPlan,
    GatewayExpiredReservationRecoveryRequest, GatewayQuotaReadAuthorizationErrorResponsePlan,
    GatewayQuotaReadAuthorizationPlan, GatewayQuotaReadAuthorizationRequest,
    GatewayUsageReconciliationErrorResponsePlan, GatewayUsageReconciliationErrorStatus,
    GatewayUsageReconciliationPlan, GatewayUsageReconciliationRequest, plan_data_plane_admission,
    plan_gateway_admission_error_response, plan_gateway_expired_reservation_recovery,
    plan_gateway_expired_reservation_recovery_error_response,
    plan_gateway_quota_read_authorization, plan_gateway_quota_read_authorization_error_response,
    plan_gateway_usage_reconciliation, plan_gateway_usage_reconciliation_error_response,
};
use prodex_gateway_http::{
    GatewayControlPlaneOperation, GatewayControlPlaneRouteError,
    GatewayControlPlaneRouteErrorResponsePlan, GatewayControlPlaneRouteErrorStatus,
    GatewayControlPlaneRoutePlan, GatewayHttpErrorResponsePlan, GatewayHttpErrorStatus,
    GatewayHttpExecutionPlan, GatewayHttpPlan, GatewayHttpPlanError, GatewayHttpPolicy,
    GatewayHttpRequestMeta, GatewayHttpRouteKind, control_plane_request_fingerprint,
    entity_tag_from_if_match_headers, idempotency_key_from_headers, page_request_from_query,
    plan_control_plane_route, plan_gateway_control_plane_route_error_response,
    plan_gateway_http_entity_tag_error_response, plan_gateway_http_error_response,
    plan_gateway_http_execution, plan_gateway_http_pagination_query_error_response,
    plan_gateway_http_request,
};
use prodex_observability::{
    ObservabilityErrorStatus, SpanPlan, SpanPlanError, plan_gateway_span, plan_span_error_response,
};
use prodex_provider_spi::{
    ProviderCapabilityNegotiationDecision, ProviderCapabilityNegotiationPlan,
    ProviderCircuitBreakerDecision, ProviderCircuitBreakerEvent, ProviderCircuitBreakerPlan,
    ProviderCircuitBreakerPolicy, ProviderCircuitBreakerState, ProviderRetryPlan,
    ProviderRetryPolicy, ProviderRetryStage, ProviderRouteCapabilityCandidate,
    negotiate_provider_route_capability, plan_provider_capability_negotiation_error_response,
    plan_provider_circuit_breaker, plan_provider_circuit_breaker_event, plan_provider_retry,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AtomicReservationCommand, AuditExportQueryCommand,
    AuditRetentionPurgeCommand, BillingLedgerQueryCommand, BudgetPolicyUpdateCommand,
    DurableStoreKind, IdempotencyCompletedRecordCommand, IdempotencyPendingRecordCommand,
    IdempotencyRecordLookupCommand, IdempotencyRecordLookupRow, IdempotencyRecordLookupRowError,
    MultiReplicaAccountingConcurrencySpec, MultiReplicaAccountingEvidence,
    MultiReplicaAccountingVerificationPlan, ProviderCredentialReferenceCommand,
    RoleBindingMutationCommand, RoleBindingMutationKind, ServiceIdentityCreateCommand,
    StoragePlanErrorResponsePlan, StoragePlanErrorStatus, StorageTopology, TenantLifecycleCommand,
    TenantLifecycleKind, TenantStorageKey, UserLifecycleCommand, UserLifecycleKind,
    VirtualKeySecretReferenceCommand, VirtualKeySecretReferenceKind,
    materialize_idempotency_record_lookup_row,
    materialize_idempotency_record_lookup_row_error_response,
    plan_multi_replica_accounting_concurrency_spec, plan_multi_replica_accounting_error_response,
    plan_multi_replica_accounting_verification, plan_storage_topology_error_response,
};
use prodex_storage_postgres::{
    PostgresAppendOnlyAuditSqlPlan, PostgresAtomicReservationSqlPlan,
    PostgresAuditExportQuerySqlPlan, PostgresAuditRetentionPurgeSqlPlan,
    PostgresBillingLedgerQuerySqlPlan, PostgresBudgetPolicyUpdateSqlPlan,
    PostgresExpiredReservationRecoverySqlPlan, PostgresIdempotencyCompletedRecordSqlPlan,
    PostgresIdempotencyPendingRecordSqlPlan, PostgresIdempotencyRecordLookupSqlPlan,
    PostgresProviderCredentialReferenceSqlPlan, PostgresRoleBindingMutationSqlPlan,
    PostgresRuntimeMode, PostgresServiceIdentityCreateSqlPlan, PostgresStorageErrorResponsePlan,
    PostgresStorageErrorStatus, PostgresTenantLifecycleSqlPlan, PostgresUsageReconciliationSqlPlan,
    PostgresUserLifecycleSqlPlan, PostgresVirtualKeySecretReferenceSqlPlan,
    plan_postgres_append_only_audit, plan_postgres_atomic_reservation,
    plan_postgres_audit_export_query, plan_postgres_audit_retention_purge,
    plan_postgres_billing_ledger_query, plan_postgres_budget_policy_update,
    plan_postgres_expired_reservation_recovery, plan_postgres_idempotency_completed_record,
    plan_postgres_idempotency_pending_record, plan_postgres_idempotency_record_lookup,
    plan_postgres_migrations, plan_postgres_provider_credential_reference,
    plan_postgres_role_binding_mutation, plan_postgres_service_identity_create,
    plan_postgres_storage_error_response, plan_postgres_tenant_lifecycle,
    plan_postgres_usage_reconciliation, plan_postgres_user_lifecycle,
    plan_postgres_virtual_key_secret_reference,
};
use prodex_storage_redis::{
    RedisCachePlan, RedisCoordinationPlan, RedisPlanErrorResponsePlan, RedisPlanErrorStatus,
    RedisRateLimitPlan, RedisRecoveryLeasePlan, RedisRecoveryLeaseReleasePlan,
    plan_policy_revision_cache, plan_recovery_lease_acquire, plan_recovery_lease_release,
    plan_redis_error_response,
};
use prodex_storage_sqlite::{
    SqliteAppendOnlyAuditSqlPlan, SqliteAtomicReservationSqlPlan, SqliteAuditExportQuerySqlPlan,
    SqliteAuditRetentionPurgeSqlPlan, SqliteBillingLedgerQuerySqlPlan,
    SqliteBudgetPolicyUpdateSqlPlan, SqliteExpiredReservationRecoverySqlPlan,
    SqliteIdempotencyCompletedRecordSqlPlan, SqliteIdempotencyPendingRecordSqlPlan,
    SqliteIdempotencyRecordLookupSqlPlan, SqliteProviderCredentialReferenceSqlPlan,
    SqliteRoleBindingMutationSqlPlan, SqliteRuntimeMode, SqliteServiceIdentityCreateSqlPlan,
    SqliteStorageErrorResponsePlan, SqliteStorageErrorStatus, SqliteTenantLifecycleSqlPlan,
    SqliteUsageReconciliationSqlPlan, SqliteUserLifecycleSqlPlan,
    SqliteVirtualKeySecretReferenceSqlPlan, plan_sqlite_append_only_audit,
    plan_sqlite_atomic_reservation, plan_sqlite_audit_export_query,
    plan_sqlite_audit_retention_purge, plan_sqlite_billing_ledger_query,
    plan_sqlite_budget_policy_update, plan_sqlite_expired_reservation_recovery,
    plan_sqlite_idempotency_completed_record, plan_sqlite_idempotency_pending_record,
    plan_sqlite_idempotency_record_lookup, plan_sqlite_migrations,
    plan_sqlite_provider_credential_reference, plan_sqlite_role_binding_mutation,
    plan_sqlite_service_identity_create, plan_sqlite_storage_error_response,
    plan_sqlite_tenant_lifecycle, plan_sqlite_usage_reconciliation, plan_sqlite_user_lifecycle,
    plan_sqlite_virtual_key_secret_reference,
};

pub fn assert_app_is_composition_root_only_marker() -> bool {
    true
}
