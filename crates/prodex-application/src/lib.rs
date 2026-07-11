#![forbid(unsafe_code)]
//! Application use-case orchestration boundary.
//!
//! This crate composes domain/gateway/control-plane/storage contracts into
//! side-effect-free use-case plans. It intentionally has no CLI, filesystem,
//! HTTP framework, async runtime, database driver, network client, or provider
//! SDK dependency. `prodex-app` and future binaries should become composition
//! roots that adapt real transports and storage clients into these plans.

pub mod distributed_rate_limit;

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
    GatewayHttpPlan, GatewayHttpPlanError, GatewayHttpPolicy, GatewayHttpRequestMeta,
    GatewayHttpRouteKind, control_plane_request_fingerprint, entity_tag_from_if_match_headers,
    idempotency_key_from_headers, page_request_from_query, plan_control_plane_route,
    plan_gateway_control_plane_route_error_response, plan_gateway_http_entity_tag_error_response,
    plan_gateway_http_error_response, plan_gateway_http_pagination_query_error_response,
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

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRequestAuthenticationRequest<'a> {
    pub http: GatewayHttpRequestMeta,
    pub oidc_policy: &'a OidcValidationPolicy,
    pub jwks_snapshot: Option<&'a JwksCacheSnapshot>,
    pub role_mapper: &'a ExplicitRoleMapper,
    pub claims: TokenClaims,
    pub now_unix_ms: u64,
}

impl fmt::Debug for ApplicationRequestAuthenticationRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestAuthenticationRequest")
            .field("http", &"<redacted>")
            .field("oidc_policy", &"<redacted>")
            .field(
                "jwks_snapshot",
                &self.jwks_snapshot.as_ref().map(|_| "<redacted>"),
            )
            .field("role_mapper", &"<redacted>")
            .field("claims", &"<redacted>")
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRequestAuthenticationPlan {
    pub http: GatewayHttpPlan,
    pub principal: Principal,
    pub required_scope: CredentialScope,
}

impl fmt::Debug for ApplicationRequestAuthenticationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestAuthenticationPlan")
            .field("http", &"<redacted>")
            .field("principal", &"<redacted>")
            .field("required_scope", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRequestAuthenticationError {
    Http(GatewayHttpPlanError),
    WrongRoute(GatewayHttpRouteKind),
    Authentication(prodex_authn::AuthenticationError),
    CredentialScopeMismatch {
        route: GatewayHttpRouteKind,
        actual: CredentialScope,
        required: CredentialScope,
    },
}

impl fmt::Debug for ApplicationRequestAuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(_) => f.debug_tuple("Http").field(&"<redacted>").finish(),
            Self::WrongRoute(_) => f.debug_tuple("WrongRoute").field(&"<redacted>").finish(),
            Self::Authentication(_) => f
                .debug_tuple("Authentication")
                .field(&"<redacted>")
                .finish(),
            Self::CredentialScopeMismatch { .. } => f
                .debug_struct("CredentialScopeMismatch")
                .field("route", &"<redacted>")
                .field("actual", &"<redacted>")
                .field("required", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationRequestAuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(err) => err.fmt(f),
            Self::WrongRoute(_) => write!(f, "request authentication is denied"),
            Self::Authentication(err) => err.fmt(f),
            Self::CredentialScopeMismatch { .. } => {
                write!(f, "request authentication is denied")
            }
        }
    }
}

impl Error for ApplicationRequestAuthenticationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRequestAuthenticationErrorResponsePlan {
    pub http: Option<GatewayHttpErrorResponsePlan>,
    pub authentication: Option<AuthenticationErrorResponsePlan>,
}

pub fn plan_application_request_authentication_error_response(
    error: &ApplicationRequestAuthenticationError,
) -> ApplicationRequestAuthenticationErrorResponsePlan {
    match error {
        ApplicationRequestAuthenticationError::Http(error) => {
            ApplicationRequestAuthenticationErrorResponsePlan {
                http: Some(plan_gateway_http_error_response(error)),
                authentication: None,
            }
        }
        ApplicationRequestAuthenticationError::Authentication(error) => {
            ApplicationRequestAuthenticationErrorResponsePlan {
                http: None,
                authentication: Some(plan_authentication_error_response(error)),
            }
        }
        ApplicationRequestAuthenticationError::WrongRoute(_)
        | ApplicationRequestAuthenticationError::CredentialScopeMismatch { .. } => {
            ApplicationRequestAuthenticationErrorResponsePlan {
                http: None,
                authentication: Some(AuthenticationErrorResponsePlan {
                    status: prodex_authn::AuthenticationErrorStatus::Unauthorized,
                    code: "credential_scope_not_allowed",
                    message: "credential scope is not allowed for this endpoint",
                }),
            }
        }
    }
}

pub fn plan_application_request_authentication(
    policy: GatewayHttpPolicy,
    request: ApplicationRequestAuthenticationRequest<'_>,
) -> Result<ApplicationRequestAuthenticationPlan, ApplicationRequestAuthenticationError> {
    let http = plan_gateway_http_request(policy, request.http)
        .map_err(ApplicationRequestAuthenticationError::Http)?;
    let required_scope = required_credential_scope_for_route(http.route).ok_or(
        ApplicationRequestAuthenticationError::WrongRoute(http.route),
    )?;
    let principal = authenticate_oidc_claims(
        request.oidc_policy,
        request.jwks_snapshot,
        request.role_mapper,
        request.claims,
        request.now_unix_ms,
    )
    .map_err(ApplicationRequestAuthenticationError::Authentication)?;
    if principal.credential_scope != required_scope {
        return Err(
            ApplicationRequestAuthenticationError::CredentialScopeMismatch {
                route: http.route,
                actual: principal.credential_scope,
                required: required_scope,
            },
        );
    }
    Ok(ApplicationRequestAuthenticationPlan {
        http,
        principal,
        required_scope,
    })
}

fn required_credential_scope_for_route(route: GatewayHttpRouteKind) -> Option<CredentialScope> {
    match route {
        GatewayHttpRouteKind::DataPlaneResponses
        | GatewayHttpRouteKind::DataPlaneCompact
        | GatewayHttpRouteKind::DataPlaneWebSocket
        | GatewayHttpRouteKind::DataPlaneQuota => Some(CredentialScope::DataPlane),
        GatewayHttpRouteKind::ControlPlane => Some(CredentialScope::ControlPlane),
        GatewayHttpRouteKind::HealthLive
        | GatewayHttpRouteKind::HealthReady
        | GatewayHttpRouteKind::HealthStartup
        | GatewayHttpRouteKind::Unknown => None,
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCapabilityRequest {
    pub request: CapabilityRequest,
    pub candidates: Vec<ProviderRouteCapabilityCandidate>,
}

impl fmt::Debug for ApplicationProviderCapabilityRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCapabilityRequest")
            .field("request", &"<redacted>")
            .field("candidates", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCapabilityPlan {
    pub provider: ProviderCapabilityNegotiationPlan,
}

impl fmt::Debug for ApplicationProviderCapabilityPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCapabilityPlan")
            .field("provider", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationProviderCapabilityError {
    Incompatible(ProviderCapabilityNegotiationDecision),
}

impl fmt::Debug for ApplicationProviderCapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incompatible(_) => f.debug_tuple("Incompatible").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationProviderCapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incompatible(_) => write!(f, "provider capability negotiation failed"),
        }
    }
}

impl Error for ApplicationProviderCapabilityError {}

pub fn plan_application_provider_capability_error_response(
    error: &ApplicationProviderCapabilityError,
) -> CapabilityErrorResponsePlan {
    match error {
        ApplicationProviderCapabilityError::Incompatible(decision) => {
            plan_provider_capability_negotiation_error_response(decision).unwrap_or(
                CapabilityErrorResponsePlan {
                    status: prodex_domain::CapabilityErrorStatus::ServiceUnavailable,
                    code: "model_route_unavailable",
                    message: "model route is temporarily unavailable",
                },
            )
        }
    }
}

pub fn plan_application_provider_capability(
    request: ApplicationProviderCapabilityRequest,
) -> Result<ApplicationProviderCapabilityPlan, ApplicationProviderCapabilityError> {
    match negotiate_provider_route_capability(&request.request, &request.candidates) {
        ProviderCapabilityNegotiationDecision::Compatible(provider) => {
            Ok(ApplicationProviderCapabilityPlan { provider })
        }
        decision => Err(ApplicationProviderCapabilityError::Incompatible(decision)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderRetryRequest {
    pub policy: ProviderRetryPolicy,
    pub stage: ProviderRetryStage,
    pub attempted_precommit_retries: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderRetryPlan {
    pub retry: ProviderRetryPlan,
}

pub fn plan_application_provider_retry(
    request: ApplicationProviderRetryRequest,
) -> ApplicationProviderRetryPlan {
    ApplicationProviderRetryPlan {
        retry: plan_provider_retry(
            request.policy,
            request.stage,
            request.attempted_precommit_retries,
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerRequest {
    pub policy: ProviderCircuitBreakerPolicy,
    pub state: ProviderCircuitBreakerState,
    pub now_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerPlan {
    pub decision: ProviderCircuitBreakerDecision,
}

pub fn plan_application_provider_circuit_breaker(
    request: ApplicationProviderCircuitBreakerRequest,
) -> ApplicationProviderCircuitBreakerPlan {
    ApplicationProviderCircuitBreakerPlan {
        decision: plan_provider_circuit_breaker(request.policy, request.state, request.now_unix_ms),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerEventRequest {
    pub policy: ProviderCircuitBreakerPolicy,
    pub state: ProviderCircuitBreakerState,
    pub event: ProviderCircuitBreakerEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerEventPlan {
    pub event: ProviderCircuitBreakerPlan,
}

pub fn plan_application_provider_circuit_breaker_event(
    request: ApplicationProviderCircuitBreakerEventRequest,
) -> ApplicationProviderCircuitBreakerEventPlan {
    ApplicationProviderCircuitBreakerEventPlan {
        event: plan_provider_circuit_breaker_event(request.policy, request.state, request.event),
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationDataPlaneRequest<R> {
    pub http: GatewayHttpRequestMeta,
    pub admission: GatewayAdmissionRequest<R>,
}

impl<R> fmt::Debug for ApplicationDataPlaneRequest<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationDataPlaneRequest")
            .field("http", &"<redacted>")
            .field("admission", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationDataPlanePlan {
    pub http: GatewayHttpPlan,
    pub admission: GatewayAdmissionPlan,
}

impl fmt::Debug for ApplicationDataPlanePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationDataPlanePlan")
            .field("http", &"<redacted>")
            .field("admission", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationDataPlaneError {
    Http(GatewayHttpPlanError),
    WrongRoute(GatewayHttpRouteKind),
    Admission(prodex_gateway_core::GatewayAdmissionError),
}

impl fmt::Debug for ApplicationDataPlaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(_) => f.debug_tuple("Http").field(&"<redacted>").finish(),
            Self::WrongRoute(_) => f.debug_tuple("WrongRoute").field(&"<redacted>").finish(),
            Self::Admission(_) => f.debug_tuple("Admission").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationDataPlaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(err) => err.fmt(f),
            Self::WrongRoute(_) => write!(f, "application route is not available"),
            Self::Admission(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationDataPlaneError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationDataPlaneErrorResponsePlan {
    pub http: Option<GatewayHttpErrorResponsePlan>,
    pub admission: Option<GatewayAdmissionErrorResponsePlan>,
}

pub fn plan_application_data_plane_error_response(
    error: &ApplicationDataPlaneError,
) -> ApplicationDataPlaneErrorResponsePlan {
    match error {
        ApplicationDataPlaneError::Http(error) => ApplicationDataPlaneErrorResponsePlan {
            http: Some(plan_gateway_http_error_response(error)),
            admission: None,
        },
        ApplicationDataPlaneError::Admission(error) => ApplicationDataPlaneErrorResponsePlan {
            http: None,
            admission: Some(plan_gateway_admission_error_response(error)),
        },
        ApplicationDataPlaneError::WrongRoute(_) => ApplicationDataPlaneErrorResponsePlan {
            http: Some(GatewayHttpErrorResponsePlan {
                status: prodex_gateway_http::GatewayHttpErrorStatus::BadRequest,
                code: "route_not_available",
                message: "route is not available for this endpoint",
            }),
            admission: None,
        },
    }
}

pub fn plan_application_data_plane<R>(
    policy: GatewayHttpPolicy,
    request: ApplicationDataPlaneRequest<R>,
) -> Result<ApplicationDataPlanePlan, ApplicationDataPlaneError>
where
    R: prodex_domain::TenantScopedResource,
{
    let http =
        plan_gateway_http_request(policy, request.http).map_err(ApplicationDataPlaneError::Http)?;
    if !matches!(
        http.route,
        GatewayHttpRouteKind::DataPlaneResponses
            | GatewayHttpRouteKind::DataPlaneCompact
            | GatewayHttpRouteKind::DataPlaneWebSocket
    ) {
        return Err(ApplicationDataPlaneError::WrongRoute(http.route));
    }
    let admission = plan_data_plane_admission(request.admission)
        .map_err(ApplicationDataPlaneError::Admission)?;
    Ok(ApplicationDataPlanePlan { http, admission })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAtomicReservationRequest {
    pub durable_store: DurableStoreKind,
    pub reservation: AtomicReservationCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationAtomicReservationStoragePlan {
    Postgres(PostgresAtomicReservationSqlPlan),
    Sqlite(SqliteAtomicReservationSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAtomicReservationPlan {
    pub storage: ApplicationAtomicReservationStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationAtomicReservationError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationAtomicReservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationAtomicReservationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationAtomicReservationErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAtomicReservationErrorResponsePlan {
    pub status: ApplicationAtomicReservationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_atomic_reservation_error_response(
    error: &ApplicationAtomicReservationError,
) -> ApplicationAtomicReservationErrorResponsePlan {
    match error {
        ApplicationAtomicReservationError::Postgres(error) => {
            application_atomic_reservation_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationAtomicReservationError::Sqlite(error) => {
            application_atomic_reservation_response_from_sqlite(plan_sqlite_storage_error_response(
                error,
            ))
        }
    }
}

fn application_atomic_reservation_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationAtomicReservationErrorResponsePlan {
    ApplicationAtomicReservationErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationAtomicReservationErrorStatus::ServiceUnavailable
            }
        },
        code: "atomic_reservation_storage_unavailable",
        message: "atomic reservation storage is temporarily unavailable",
    }
}

fn application_atomic_reservation_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationAtomicReservationErrorResponsePlan {
    ApplicationAtomicReservationErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationAtomicReservationErrorStatus::ServiceUnavailable
            }
        },
        code: "atomic_reservation_storage_unavailable",
        message: "atomic reservation storage is temporarily unavailable",
    }
}

pub fn plan_application_atomic_reservation(
    request: ApplicationAtomicReservationRequest,
) -> Result<ApplicationAtomicReservationPlan, ApplicationAtomicReservationError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationAtomicReservationStoragePlan::Postgres(
            plan_postgres_atomic_reservation(request.reservation)
                .map_err(ApplicationAtomicReservationError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationAtomicReservationStoragePlan::Sqlite(
            plan_sqlite_atomic_reservation(request.reservation)
                .map_err(ApplicationAtomicReservationError::Sqlite)?,
        ),
    };
    Ok(ApplicationAtomicReservationPlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationQuotaReadRequest<R> {
    pub http: GatewayHttpRequestMeta,
    pub authorization: GatewayQuotaReadAuthorizationRequest<R>,
}

impl<R> fmt::Debug for ApplicationQuotaReadRequest<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationQuotaReadRequest")
            .field("http", &"<redacted>")
            .field("authorization", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationQuotaReadPlan {
    pub http: GatewayHttpPlan,
    pub authorization: GatewayQuotaReadAuthorizationPlan,
}

impl fmt::Debug for ApplicationQuotaReadPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationQuotaReadPlan")
            .field("http", &"<redacted>")
            .field("authorization", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationQuotaReadError {
    Http(GatewayHttpPlanError),
    WrongRoute(GatewayHttpRouteKind),
    Authorization(prodex_gateway_core::GatewayQuotaReadAuthorizationError),
}

impl fmt::Debug for ApplicationQuotaReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(_) => f.debug_tuple("Http").field(&"<redacted>").finish(),
            Self::WrongRoute(_) => f.debug_tuple("WrongRoute").field(&"<redacted>").finish(),
            Self::Authorization(_) => f.debug_tuple("Authorization").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationQuotaReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(err) => err.fmt(f),
            Self::WrongRoute(_) => write!(f, "application route is not available"),
            Self::Authorization(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationQuotaReadError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationQuotaReadErrorResponsePlan {
    pub http: Option<GatewayHttpErrorResponsePlan>,
    pub authorization: Option<GatewayQuotaReadAuthorizationErrorResponsePlan>,
}

pub fn plan_application_quota_read_error_response(
    error: &ApplicationQuotaReadError,
) -> ApplicationQuotaReadErrorResponsePlan {
    match error {
        ApplicationQuotaReadError::Http(error) => ApplicationQuotaReadErrorResponsePlan {
            http: Some(plan_gateway_http_error_response(error)),
            authorization: None,
        },
        ApplicationQuotaReadError::Authorization(error) => ApplicationQuotaReadErrorResponsePlan {
            http: None,
            authorization: Some(plan_gateway_quota_read_authorization_error_response(error)),
        },
        ApplicationQuotaReadError::WrongRoute(_) => ApplicationQuotaReadErrorResponsePlan {
            http: Some(GatewayHttpErrorResponsePlan {
                status: prodex_gateway_http::GatewayHttpErrorStatus::BadRequest,
                code: "route_not_available",
                message: "route is not available for this endpoint",
            }),
            authorization: None,
        },
    }
}

pub fn plan_application_quota_read<R>(
    policy: GatewayHttpPolicy,
    request: ApplicationQuotaReadRequest<R>,
) -> Result<ApplicationQuotaReadPlan, ApplicationQuotaReadError>
where
    R: prodex_domain::TenantScopedResource,
{
    let http =
        plan_gateway_http_request(policy, request.http).map_err(ApplicationQuotaReadError::Http)?;
    if http.route != GatewayHttpRouteKind::DataPlaneQuota {
        return Err(ApplicationQuotaReadError::WrongRoute(http.route));
    }
    let authorization = plan_gateway_quota_read_authorization(request.authorization)
        .map_err(ApplicationQuotaReadError::Authorization)?;
    Ok(ApplicationQuotaReadPlan {
        http,
        authorization,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationRequest {
    pub durable_store: DurableStoreKind,
    pub gateway: GatewayUsageReconciliationRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationUsageReconciliationStoragePlan {
    Postgres(PostgresUsageReconciliationSqlPlan),
    Sqlite(SqliteUsageReconciliationSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationPlan {
    pub gateway: GatewayUsageReconciliationPlan,
    pub storage: ApplicationUsageReconciliationStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationUsageReconciliationError {
    Gateway(prodex_gateway_core::GatewayUsageReconciliationError),
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationUsageReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gateway(err) => err.fmt(f),
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationUsageReconciliationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationUsageReconciliationErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationErrorResponsePlan {
    pub status: ApplicationUsageReconciliationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_usage_reconciliation_error_response(
    error: &ApplicationUsageReconciliationError,
) -> ApplicationUsageReconciliationErrorResponsePlan {
    match error {
        ApplicationUsageReconciliationError::Gateway(error) => {
            application_usage_reconciliation_response_from_gateway(
                plan_gateway_usage_reconciliation_error_response(error),
            )
        }
        ApplicationUsageReconciliationError::Postgres(error) => {
            application_usage_reconciliation_response_from_postgres(
                plan_postgres_storage_error_response(error),
                "usage_reconciliation_storage_unavailable",
                "usage reconciliation storage is temporarily unavailable",
            )
        }
        ApplicationUsageReconciliationError::Sqlite(error) => {
            application_usage_reconciliation_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
                "usage_reconciliation_storage_unavailable",
                "usage reconciliation storage is temporarily unavailable",
            )
        }
    }
}

fn application_usage_reconciliation_response_from_gateway(
    response: GatewayUsageReconciliationErrorResponsePlan,
) -> ApplicationUsageReconciliationErrorResponsePlan {
    ApplicationUsageReconciliationErrorResponsePlan {
        status: match response.status {
            GatewayUsageReconciliationErrorStatus::BadRequest => {
                ApplicationUsageReconciliationErrorStatus::BadRequest
            }
            GatewayUsageReconciliationErrorStatus::ServiceUnavailable => {
                ApplicationUsageReconciliationErrorStatus::ServiceUnavailable
            }
        },
        code: response.code,
        message: response.message,
    }
}

fn application_usage_reconciliation_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationUsageReconciliationErrorResponsePlan {
    ApplicationUsageReconciliationErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationUsageReconciliationErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_usage_reconciliation_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationUsageReconciliationErrorResponsePlan {
    ApplicationUsageReconciliationErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationUsageReconciliationErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_usage_reconciliation(
    request: ApplicationUsageReconciliationRequest,
) -> Result<ApplicationUsageReconciliationPlan, ApplicationUsageReconciliationError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationUsageReconciliationStoragePlan::Postgres(
            plan_postgres_usage_reconciliation(request.gateway.reconciliation.clone())
                .map_err(ApplicationUsageReconciliationError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationUsageReconciliationStoragePlan::Sqlite(
            plan_sqlite_usage_reconciliation(request.gateway.reconciliation.clone())
                .map_err(ApplicationUsageReconciliationError::Sqlite)?,
        ),
    };
    let gateway = plan_gateway_usage_reconciliation(request.gateway)
        .map_err(ApplicationUsageReconciliationError::Gateway)?;
    Ok(ApplicationUsageReconciliationPlan { gateway, storage })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationExpiredReservationRecoveryRequest {
    pub durable_store: DurableStoreKind,
    pub gateway: GatewayExpiredReservationRecoveryRequest,
    pub coordination: Option<ApplicationExpiredReservationRecoveryCoordinationRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationExpiredReservationRecoveryCoordinationRequest {
    pub shard: String,
    pub owner_token: String,
    pub ttl_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationExpiredReservationRecoveryStoragePlan {
    Postgres(PostgresExpiredReservationRecoverySqlPlan),
    Sqlite(SqliteExpiredReservationRecoverySqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationExpiredReservationRecoveryPlan {
    pub gateway: GatewayExpiredReservationRecoveryPlan,
    pub storage: ApplicationExpiredReservationRecoveryStoragePlan,
    pub coordination: Option<ApplicationExpiredReservationRecoveryCoordinationPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationExpiredReservationRecoveryCoordinationPlan {
    RedisLease(RedisRecoveryLeasePlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationExpiredReservationRecoveryError {
    Gateway(prodex_gateway_core::GatewayExpiredReservationRecoveryError),
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Redis(prodex_storage_redis::RedisPlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationExpiredReservationRecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gateway(err) => err.fmt(f),
            Self::Postgres(err) => err.fmt(f),
            Self::Redis(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationExpiredReservationRecoveryError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationExpiredReservationRecoveryErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationExpiredReservationRecoveryErrorResponsePlan {
    pub status: ApplicationExpiredReservationRecoveryErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_expired_reservation_recovery_error_response(
    error: &ApplicationExpiredReservationRecoveryError,
) -> ApplicationExpiredReservationRecoveryErrorResponsePlan {
    match error {
        ApplicationExpiredReservationRecoveryError::Gateway(error) => {
            application_expired_reservation_recovery_response_from_gateway(
                plan_gateway_expired_reservation_recovery_error_response(error),
            )
        }
        ApplicationExpiredReservationRecoveryError::Redis(_) => {
            ApplicationExpiredReservationRecoveryErrorResponsePlan {
                status: ApplicationExpiredReservationRecoveryErrorStatus::ServiceUnavailable,
                code: "recovery_coordination_unavailable",
                message: "expired reservation recovery coordination is temporarily unavailable",
            }
        }
        ApplicationExpiredReservationRecoveryError::Postgres(error) => {
            application_expired_reservation_recovery_response_from_postgres(
                plan_postgres_storage_error_response(error),
                "expired_reservation_recovery_storage_unavailable",
                "expired reservation recovery storage is temporarily unavailable",
            )
        }
        ApplicationExpiredReservationRecoveryError::Sqlite(error) => {
            application_expired_reservation_recovery_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
                "expired_reservation_recovery_storage_unavailable",
                "expired reservation recovery storage is temporarily unavailable",
            )
        }
    }
}

fn application_expired_reservation_recovery_response_from_gateway(
    response: GatewayExpiredReservationRecoveryErrorResponsePlan,
) -> ApplicationExpiredReservationRecoveryErrorResponsePlan {
    ApplicationExpiredReservationRecoveryErrorResponsePlan {
        status: match response.status {
            GatewayExpiredReservationRecoveryErrorStatus::BadRequest => {
                ApplicationExpiredReservationRecoveryErrorStatus::BadRequest
            }
            GatewayExpiredReservationRecoveryErrorStatus::ServiceUnavailable => {
                ApplicationExpiredReservationRecoveryErrorStatus::ServiceUnavailable
            }
        },
        code: response.code,
        message: response.message,
    }
}

fn application_expired_reservation_recovery_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationExpiredReservationRecoveryErrorResponsePlan {
    ApplicationExpiredReservationRecoveryErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationExpiredReservationRecoveryErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_expired_reservation_recovery_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationExpiredReservationRecoveryErrorResponsePlan {
    ApplicationExpiredReservationRecoveryErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationExpiredReservationRecoveryErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_expired_reservation_recovery(
    request: ApplicationExpiredReservationRecoveryRequest,
) -> Result<ApplicationExpiredReservationRecoveryPlan, ApplicationExpiredReservationRecoveryError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationExpiredReservationRecoveryStoragePlan::Postgres(
            plan_postgres_expired_reservation_recovery(request.gateway.recovery.clone())
                .map_err(ApplicationExpiredReservationRecoveryError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationExpiredReservationRecoveryStoragePlan::Sqlite(
            plan_sqlite_expired_reservation_recovery(request.gateway.recovery.clone())
                .map_err(ApplicationExpiredReservationRecoveryError::Sqlite)?,
        ),
    };
    let coordination = request
        .coordination
        .map(|coordination| {
            plan_recovery_lease_acquire(
                request.gateway.tenant.tenant_id,
                request.gateway.recovery.storage_key,
                coordination.shard,
                coordination.owner_token,
                coordination.ttl_seconds,
            )
            .map(ApplicationExpiredReservationRecoveryCoordinationPlan::RedisLease)
            .map_err(ApplicationExpiredReservationRecoveryError::Redis)
        })
        .transpose()?;
    let gateway = plan_gateway_expired_reservation_recovery(request.gateway)
        .map_err(ApplicationExpiredReservationRecoveryError::Gateway)?;
    Ok(ApplicationExpiredReservationRecoveryPlan {
        gateway,
        storage,
        coordination,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRecoveryLeaseReleaseRequest {
    pub tenant_id: prodex_domain::TenantId,
    pub storage_key: TenantStorageKey,
    pub shard: String,
    pub owner_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRecoveryLeaseReleasePlan {
    pub release: RedisRecoveryLeaseReleasePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRecoveryLeaseReleaseError {
    Redis(prodex_storage_redis::RedisPlanError),
}

impl fmt::Display for ApplicationRecoveryLeaseReleaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Redis(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationRecoveryLeaseReleaseError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRecoveryLeaseReleaseErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRecoveryLeaseReleaseErrorResponsePlan {
    pub status: ApplicationRecoveryLeaseReleaseErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_recovery_lease_release_error_response(
    error: &ApplicationRecoveryLeaseReleaseError,
) -> ApplicationRecoveryLeaseReleaseErrorResponsePlan {
    match error {
        ApplicationRecoveryLeaseReleaseError::Redis(error) => {
            application_recovery_lease_release_response_from_redis(
                plan_redis_error_response(error),
                "recovery_lease_release_unavailable",
                "recovery lease release is temporarily unavailable",
            )
        }
    }
}

fn application_recovery_lease_release_response_from_redis(
    response: RedisPlanErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationRecoveryLeaseReleaseErrorResponsePlan {
    ApplicationRecoveryLeaseReleaseErrorResponsePlan {
        status: match response.status {
            RedisPlanErrorStatus::ServiceUnavailable => {
                ApplicationRecoveryLeaseReleaseErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_recovery_lease_release(
    request: ApplicationRecoveryLeaseReleaseRequest,
) -> Result<ApplicationRecoveryLeaseReleasePlan, ApplicationRecoveryLeaseReleaseError> {
    let release = plan_recovery_lease_release(
        request.tenant_id,
        request.storage_key,
        request.shard,
        request.owner_token,
    )
    .map_err(ApplicationRecoveryLeaseReleaseError::Redis)?;
    Ok(ApplicationRecoveryLeaseReleasePlan { release })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePlan {
    pub decision: ControlPlaneDecision,
}

pub fn plan_application_control_plane(
    request: ControlPlaneActionRequest,
) -> ApplicationControlPlanePlan {
    ApplicationControlPlanePlan {
        decision: decide_control_plane_action(request),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBreakGlassAuditRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub authorization: BreakGlassAuthorization,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditStoragePlan {
    Postgres(PostgresAppendOnlyAuditSqlPlan),
    Sqlite(SqliteAppendOnlyAuditSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditHttpPlan {
    pub action: ControlPlaneActionRequest,
    pub route: GatewayControlPlaneRoutePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditCorrelationRequest {
    pub request_id: RequestId,
    pub call_id: Option<CallId>,
    pub http_policy: GatewayHttpPolicy,
    pub http: GatewayHttpRequestMeta,
    pub audit: ApplicationControlPlaneAuditPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditCorrelationPlan {
    pub correlation: CorrelationContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditEmissionSpanRequest {
    pub correlation: CorrelationContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditEmissionSpanPlan {
    pub span: SpanPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPersistenceSpanRequest {
    pub correlation: CorrelationContext,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPersistenceSpanPlan {
    pub span: SpanPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBreakGlassAuditPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditError {
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    AuditNotRequired {
        route_operation: GatewayControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditCorrelationError {
    Http(GatewayHttpPlanError),
    MissingTraceContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditEmissionSpanError {
    Span(SpanPlanError),
    MissingTenant,
    MissingAuditEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditPersistenceSpanError {
    Span(SpanPlanError),
    MissingTenant,
    MissingAuditEvent,
}

impl fmt::Display for ApplicationControlPlaneAuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpRoute(err) => err.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::AuditNotRequired { .. } => {
                write!(f, "control-plane route does not require audit")
            }
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlaneAuditError {}

impl fmt::Display for ApplicationControlPlaneAuditCorrelationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(error) => error.fmt(f),
            Self::MissingTraceContext => write!(f, "trace context is required for audit"),
        }
    }
}

impl Error for ApplicationControlPlaneAuditCorrelationError {}

impl fmt::Display for ApplicationControlPlaneAuditEmissionSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Span(error) => error.fmt(f),
            Self::MissingTenant => write!(f, "audit emission span requires tenant correlation"),
            Self::MissingAuditEvent => {
                write!(f, "audit emission span requires audit event correlation")
            }
        }
    }
}

impl Error for ApplicationControlPlaneAuditEmissionSpanError {}

impl fmt::Display for ApplicationControlPlaneAuditPersistenceSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Span(error) => error.fmt(f),
            Self::MissingTenant => write!(f, "audit persistence span requires tenant correlation"),
            Self::MissingAuditEvent => {
                write!(f, "audit persistence span requires audit event correlation")
            }
        }
    }
}

impl Error for ApplicationControlPlaneAuditPersistenceSpanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditErrorStatus {
    BadRequest,
    MethodNotAllowed,
    ServiceUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditCorrelationErrorStatus {
    BadRequest,
    MethodNotAllowed,
    PayloadTooLarge,
    ServiceUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditEmissionSpanErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditPersistenceSpanErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditCorrelationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditEmissionSpanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditPersistenceSpanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_control_plane_audit_error_response(
    error: &ApplicationControlPlaneAuditError,
) -> ApplicationControlPlaneAuditErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditError::HttpRoute(error) => {
            let response = match error {
                ApplicationControlPlaneHttpRouteError::Route(error) => {
                    plan_gateway_control_plane_route_error_response(error)
                }
            };
            ApplicationControlPlaneAuditErrorResponsePlan {
                status: match response.status {
                    GatewayControlPlaneRouteErrorStatus::BadRequest => {
                        ApplicationControlPlaneAuditErrorStatus::BadRequest
                    }
                    GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditError::OperationMismatch { .. } => {
            ApplicationControlPlaneAuditErrorResponsePlan {
                status: ApplicationControlPlaneAuditErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlaneAuditError::AuditNotRequired { .. } => {
            ApplicationControlPlaneAuditErrorResponsePlan {
                status: ApplicationControlPlaneAuditErrorStatus::BadRequest,
                code: "control_plane_audit_required",
                message: "control-plane audit is required",
            }
        }
        ApplicationControlPlaneAuditError::Postgres(error) => {
            application_control_plane_audit_response_from_postgres(
                plan_postgres_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
        ApplicationControlPlaneAuditError::Sqlite(error) => {
            application_control_plane_audit_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
    }
}

pub fn plan_application_control_plane_audit_correlation_error_response(
    error: &ApplicationControlPlaneAuditCorrelationError,
) -> ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditCorrelationError::Http(error) => {
            let response = plan_gateway_http_error_response(error);
            ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
                status: match response.status {
                    GatewayHttpErrorStatus::BadRequest => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::BadRequest
                    }
                    GatewayHttpErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::MethodNotAllowed
                    }
                    GatewayHttpErrorStatus::PayloadTooLarge => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::PayloadTooLarge
                    }
                    GatewayHttpErrorStatus::InternalServerError => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditCorrelationError::MissingTraceContext => {
            ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
                status: ApplicationControlPlaneAuditCorrelationErrorStatus::BadRequest,
                code: "invalid_trace_context",
                message: "trace context is required and must be valid",
            }
        }
    }
}

pub fn plan_application_control_plane_audit_emission_span_error_response(
    error: &ApplicationControlPlaneAuditEmissionSpanError,
) -> ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditEmissionSpanError::Span(error) => {
            let response = plan_span_error_response(error);
            ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
                status: match response.status {
                    ObservabilityErrorStatus::ServiceUnavailable => {
                        ApplicationControlPlaneAuditEmissionSpanErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditEmissionSpanError::MissingTenant
        | ApplicationControlPlaneAuditEmissionSpanError::MissingAuditEvent => {
            ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
                status: ApplicationControlPlaneAuditEmissionSpanErrorStatus::ServiceUnavailable,
                code: "telemetry_unavailable",
                message: "telemetry planning is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_control_plane_audit_persistence_span_error_response(
    error: &ApplicationControlPlaneAuditPersistenceSpanError,
) -> ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditPersistenceSpanError::Span(error) => {
            let response = plan_span_error_response(error);
            ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
                status: match response.status {
                    ObservabilityErrorStatus::ServiceUnavailable => {
                        ApplicationControlPlaneAuditPersistenceSpanErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditPersistenceSpanError::MissingTenant
        | ApplicationControlPlaneAuditPersistenceSpanError::MissingAuditEvent => {
            ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
                status: ApplicationControlPlaneAuditPersistenceSpanErrorStatus::ServiceUnavailable,
                code: "telemetry_unavailable",
                message: "telemetry planning is temporarily unavailable",
            }
        }
    }
}

fn application_control_plane_audit_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationControlPlaneAuditErrorResponsePlan {
    ApplicationControlPlaneAuditErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_control_plane_audit_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationControlPlaneAuditErrorResponsePlan {
    ApplicationControlPlaneAuditErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_control_plane_with_audit_storage(
    request: ApplicationControlPlaneAuditRequest,
) -> Result<ApplicationControlPlaneAuditPlan, ApplicationControlPlaneAuditError> {
    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)?,
        ),
    };
    Ok(ApplicationControlPlaneAuditPlan {
        decision,
        audit_storage,
    })
}

pub fn plan_application_control_plane_audit_from_http(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneAuditHttpPlan, ApplicationControlPlaneAuditError> {
    let route = validate_control_plane_http_action_for_audit(&action, http)?;
    Ok(ApplicationControlPlaneAuditHttpPlan {
        action,
        route: route.http,
    })
}

pub fn plan_application_control_plane_with_audit_storage_from_http(
    request: ApplicationControlPlaneAuditRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneAuditPlan, ApplicationControlPlaneAuditError> {
    validate_control_plane_http_action_for_audit(&request.action, http)?;
    plan_application_control_plane_with_audit_storage(request)
}

pub fn plan_application_control_plane_audit_correlation_from_http(
    request: ApplicationControlPlaneAuditCorrelationRequest,
) -> Result<ApplicationControlPlaneAuditCorrelationPlan, ApplicationControlPlaneAuditCorrelationError>
{
    let http = plan_gateway_http_request(request.http_policy, request.http)
        .map_err(ApplicationControlPlaneAuditCorrelationError::Http)?;
    let trace_context = http
        .trace_context
        .ok_or(ApplicationControlPlaneAuditCorrelationError::MissingTraceContext)?;
    let audit_write = control_plane_audit_write(&request.audit.decision);
    let mut correlation = CorrelationContext::new(request.request_id)
        .with_trace_id(trace_context.trace_id)
        .with_tenant_id(audit_write.tenant_partition_key)
        .with_audit_event_id(audit_write.event.id);
    if let Some(call_id) = request.call_id {
        correlation = correlation.with_call_id(call_id);
    }
    Ok(ApplicationControlPlaneAuditCorrelationPlan { correlation })
}

pub fn plan_application_control_plane_audit_emission_span(
    request: ApplicationControlPlaneAuditEmissionSpanRequest,
) -> Result<
    ApplicationControlPlaneAuditEmissionSpanPlan,
    ApplicationControlPlaneAuditEmissionSpanError,
> {
    let tenant_id = request
        .correlation
        .tenant_id
        .ok_or(ApplicationControlPlaneAuditEmissionSpanError::MissingTenant)?;
    let audit_event_id = request
        .correlation
        .audit_event_id
        .ok_or(ApplicationControlPlaneAuditEmissionSpanError::MissingAuditEvent)?;
    let span = plan_gateway_span(
        GatewaySpanKind::AuditEmission,
        "prodex.control_plane.audit.emit",
        request.correlation,
        None,
        vec![
            tenant_trace_attribute(tenant_id),
            TelemetryAttribute::trace_only("audit_event_id", audit_event_id.to_string()),
        ],
    )
    .map_err(ApplicationControlPlaneAuditEmissionSpanError::Span)?;
    Ok(ApplicationControlPlaneAuditEmissionSpanPlan { span })
}

pub fn plan_application_control_plane_audit_persistence_span(
    request: ApplicationControlPlaneAuditPersistenceSpanRequest,
) -> Result<
    ApplicationControlPlaneAuditPersistenceSpanPlan,
    ApplicationControlPlaneAuditPersistenceSpanError,
> {
    let tenant_id = request
        .correlation
        .tenant_id
        .ok_or(ApplicationControlPlaneAuditPersistenceSpanError::MissingTenant)?;
    let audit_event_id = request
        .correlation
        .audit_event_id
        .ok_or(ApplicationControlPlaneAuditPersistenceSpanError::MissingAuditEvent)?;
    let storage_backend = application_control_plane_audit_storage_backend(&request.audit_storage);
    let span = plan_gateway_span(
        GatewaySpanKind::Persistence,
        "prodex.control_plane.audit.persist",
        request.correlation,
        None,
        vec![
            TelemetryAttribute::metric_label("storage_backend", storage_backend),
            tenant_trace_attribute(tenant_id),
            TelemetryAttribute::trace_only("audit_event_id", audit_event_id.to_string()),
        ],
    )
    .map_err(ApplicationControlPlaneAuditPersistenceSpanError::Span)?;
    Ok(ApplicationControlPlaneAuditPersistenceSpanPlan { span })
}

fn application_control_plane_audit_storage_backend(
    audit_storage: &ApplicationControlPlaneAuditStoragePlan,
) -> &'static str {
    match audit_storage {
        ApplicationControlPlaneAuditStoragePlan::Postgres(_) => "postgres",
        ApplicationControlPlaneAuditStoragePlan::Sqlite(_) => "sqlite",
    }
}

pub fn plan_application_break_glass_with_audit_storage(
    request: ApplicationBreakGlassAuditRequest,
) -> Result<ApplicationBreakGlassAuditPlan, ApplicationControlPlaneAuditError> {
    let decision = decide_break_glass_action(request.action, request.authorization);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)?,
        ),
    };
    Ok(ApplicationBreakGlassAuditPlan {
        decision,
        audit_storage,
    })
}

fn control_plane_audit_write(decision: &ControlPlaneDecision) -> &ControlPlaneAuditWritePlan {
    match decision {
        ControlPlaneDecision::Authorized(plan) => &plan.audit_write,
        ControlPlaneDecision::Denied { audit_write, .. } => audit_write,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyRequest {
    pub action: ControlPlaneActionRequest,
    pub idempotency_key: Option<IdempotencyKey>,
    pub request_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyPlan {
    pub action: ControlPlaneActionRequest,
    pub operation: Option<IdempotentOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneHttpRoutePlan {
    pub http: GatewayControlPlaneRoutePlan,
    pub operation: ControlPlaneOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneHttpRouteError {
    Route(GatewayControlPlaneRouteError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneHttpRouteErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneHttpRouteErrorResponsePlan {
    pub status: ApplicationControlPlaneHttpRouteErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Display for ApplicationControlPlaneHttpRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlaneHttpRouteError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyError {
    IdempotencyKeyRequired,
    IdempotencyKeyInvalid(prodex_gateway_http::GatewayHttpIdempotencyKeyError),
    RequestFingerprintInvalid(prodex_gateway_http::GatewayHttpRequestFingerprintError),
    RequestFingerprintDomainInvalid(IdempotentOperationError),
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    ReplayConflict(IdempotencyConflict),
    LookupRowInvalid(IdempotencyRecordLookupRowError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePageRequestPlan {
    pub action: ControlPlaneActionRequest,
    pub page_request: PageRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePreconditionPlan {
    pub action: ControlPlaneActionRequest,
    pub entity_tag: Option<EntityTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePageRequestError {
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    Pagination(prodex_gateway_http::GatewayHttpPaginationQueryError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePreconditionError {
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    EntityTag(prodex_gateway_http::GatewayHttpEntityTagError),
}

impl fmt::Display for ApplicationControlPlanePageRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpRoute(error) => error.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::Pagination(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlanePageRequestError {}

impl fmt::Display for ApplicationControlPlanePreconditionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpRoute(error) => error.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::EntityTag(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlanePreconditionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePageRequestErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePageRequestErrorResponsePlan {
    pub status: ApplicationControlPlanePageRequestErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePreconditionErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePreconditionErrorResponsePlan {
    pub status: ApplicationControlPlanePreconditionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Display for ApplicationControlPlaneIdempotencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdempotencyKeyRequired => {
                write!(
                    f,
                    "idempotency key is required for mutating control-plane operation"
                )
            }
            Self::IdempotencyKeyInvalid(error) => error.fmt(f),
            Self::RequestFingerprintInvalid(error) => error.fmt(f),
            Self::RequestFingerprintDomainInvalid(error) => error.fmt(f),
            Self::HttpRoute(error) => error.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::ReplayConflict(error) => error.fmt(f),
            Self::LookupRowInvalid(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlaneIdempotencyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyErrorStatus {
    BadRequest,
    Conflict,
    MethodNotAllowed,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyErrorResponsePlan {
    pub status: ApplicationControlPlaneIdempotencyErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_control_plane_idempotency_error_response(
    error: &ApplicationControlPlaneIdempotencyError,
) -> ApplicationControlPlaneIdempotencyErrorResponsePlan {
    match error {
        ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired => {
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: ApplicationControlPlaneIdempotencyErrorStatus::BadRequest,
                code: "control_plane_idempotency_key_required",
                message: "control-plane idempotency key is required",
            }
        }
        ApplicationControlPlaneIdempotencyError::IdempotencyKeyInvalid(error) => {
            let response =
                prodex_gateway_http::plan_gateway_http_idempotency_key_error_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpIdempotencyKeyErrorStatus::BadRequest => {
                        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneIdempotencyError::RequestFingerprintInvalid(error) => {
            let response =
                prodex_gateway_http::plan_gateway_http_request_fingerprint_error_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpRequestFingerprintErrorStatus::BadRequest => {
                        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneIdempotencyError::RequestFingerprintDomainInvalid(_) => {
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: ApplicationControlPlaneIdempotencyErrorStatus::BadRequest,
                code: "request_fingerprint_invalid",
                message: "request fingerprint is invalid",
            }
        }
        ApplicationControlPlaneIdempotencyError::HttpRoute(error) => {
            application_control_plane_route_response_from_gateway(error)
        }
        ApplicationControlPlaneIdempotencyError::OperationMismatch { .. } => {
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: ApplicationControlPlaneIdempotencyErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlaneIdempotencyError::ReplayConflict(error) => {
            let response = plan_idempotency_conflict_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    IdempotencyConflictStatus::Conflict => {
                        ApplicationControlPlaneIdempotencyErrorStatus::Conflict
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneIdempotencyError::LookupRowInvalid(error) => {
            let response = materialize_idempotency_record_lookup_row_error_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    StoragePlanErrorStatus::BadRequest
                    | StoragePlanErrorStatus::InvalidConfiguration => {
                        ApplicationControlPlaneIdempotencyErrorStatus::ServiceUnavailable
                    }
                },
                code: "control_plane_idempotency_lookup_invalid",
                message: "control-plane idempotency lookup is invalid",
            }
        }
    }
}

fn application_control_plane_route_response_from_gateway(
    error: &ApplicationControlPlaneHttpRouteError,
) -> ApplicationControlPlaneIdempotencyErrorResponsePlan {
    let response = match error {
        ApplicationControlPlaneHttpRouteError::Route(error) => {
            plan_gateway_control_plane_route_error_response(error)
        }
    };
    application_control_plane_route_response_from_gateway_plan(response)
}

fn application_control_plane_route_response_from_gateway_plan(
    response: GatewayControlPlaneRouteErrorResponsePlan,
) -> ApplicationControlPlaneIdempotencyErrorResponsePlan {
    ApplicationControlPlaneIdempotencyErrorResponsePlan {
        status: match response.status {
            GatewayControlPlaneRouteErrorStatus::BadRequest => {
                ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
            }
            GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_application_control_plane_page_request_error_response(
    error: &ApplicationControlPlanePageRequestError,
) -> ApplicationControlPlanePageRequestErrorResponsePlan {
    match error {
        ApplicationControlPlanePageRequestError::HttpRoute(error) => {
            let response = match error {
                ApplicationControlPlaneHttpRouteError::Route(error) => {
                    plan_gateway_control_plane_route_error_response(error)
                }
            };
            ApplicationControlPlanePageRequestErrorResponsePlan {
                status: match response.status {
                    GatewayControlPlaneRouteErrorStatus::BadRequest => {
                        ApplicationControlPlanePageRequestErrorStatus::BadRequest
                    }
                    GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlanePageRequestErrorStatus::MethodNotAllowed
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlanePageRequestError::OperationMismatch { .. } => {
            ApplicationControlPlanePageRequestErrorResponsePlan {
                status: ApplicationControlPlanePageRequestErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlanePageRequestError::Pagination(error) => {
            let response = plan_gateway_http_pagination_query_error_response(error);
            ApplicationControlPlanePageRequestErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpPaginationQueryErrorStatus::BadRequest => {
                        ApplicationControlPlanePageRequestErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
    }
}

pub fn plan_application_control_plane_precondition_error_response(
    error: &ApplicationControlPlanePreconditionError,
) -> ApplicationControlPlanePreconditionErrorResponsePlan {
    match error {
        ApplicationControlPlanePreconditionError::HttpRoute(error) => {
            let response = match error {
                ApplicationControlPlaneHttpRouteError::Route(error) => {
                    plan_gateway_control_plane_route_error_response(error)
                }
            };
            ApplicationControlPlanePreconditionErrorResponsePlan {
                status: match response.status {
                    GatewayControlPlaneRouteErrorStatus::BadRequest => {
                        ApplicationControlPlanePreconditionErrorStatus::BadRequest
                    }
                    GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlanePreconditionErrorStatus::MethodNotAllowed
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlanePreconditionError::OperationMismatch { .. } => {
            ApplicationControlPlanePreconditionErrorResponsePlan {
                status: ApplicationControlPlanePreconditionErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlanePreconditionError::EntityTag(error) => {
            let response = plan_gateway_http_entity_tag_error_response(error);
            ApplicationControlPlanePreconditionErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpEntityTagErrorStatus::BadRequest => {
                        ApplicationControlPlanePreconditionErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
    }
}

pub fn plan_application_control_plane_idempotency(
    request: ApplicationControlPlaneIdempotencyRequest,
) -> Result<ApplicationControlPlaneIdempotencyPlan, ApplicationControlPlaneIdempotencyError> {
    let operation = if request.action.operation.requires_idempotency() {
        let key = request
            .idempotency_key
            .ok_or(ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired)?;
        request
            .action
            .idempotent_operation(key, request.request_fingerprint)
            .map_err(ApplicationControlPlaneIdempotencyError::RequestFingerprintDomainInvalid)?
    } else {
        None
    };
    Ok(ApplicationControlPlaneIdempotencyPlan {
        action: request.action,
        operation,
    })
}

pub fn plan_application_control_plane_http_route(
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneHttpRoutePlan, ApplicationControlPlaneHttpRouteError> {
    let route =
        plan_control_plane_route(http).map_err(ApplicationControlPlaneHttpRouteError::Route)?;
    Ok(ApplicationControlPlaneHttpRoutePlan {
        operation: control_plane_operation_from_gateway_route(route.operation),
        http: route,
    })
}

pub fn plan_application_control_plane_http_route_error_response(
    error: &ApplicationControlPlaneHttpRouteError,
) -> ApplicationControlPlaneHttpRouteErrorResponsePlan {
    let response = match error {
        ApplicationControlPlaneHttpRouteError::Route(error) => {
            plan_gateway_control_plane_route_error_response(error)
        }
    };
    ApplicationControlPlaneHttpRouteErrorResponsePlan {
        status: match response.status {
            GatewayControlPlaneRouteErrorStatus::BadRequest => {
                ApplicationControlPlaneHttpRouteErrorStatus::BadRequest
            }
            GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                ApplicationControlPlaneHttpRouteErrorStatus::MethodNotAllowed
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_application_control_plane_idempotency_from_http(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
    request_fingerprint: impl Into<String>,
) -> Result<ApplicationControlPlaneIdempotencyPlan, ApplicationControlPlaneIdempotencyError> {
    validate_control_plane_http_action(&action, http)?;
    let idempotency_key = idempotency_key_from_headers(&http.headers)
        .map_err(ApplicationControlPlaneIdempotencyError::IdempotencyKeyInvalid)?;
    plan_application_control_plane_idempotency(ApplicationControlPlaneIdempotencyRequest {
        action,
        idempotency_key,
        request_fingerprint: request_fingerprint.into(),
    })
}

pub fn plan_application_control_plane_idempotency_from_http_digest(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
    body_digest: impl AsRef<str>,
) -> Result<ApplicationControlPlaneIdempotencyPlan, ApplicationControlPlaneIdempotencyError> {
    validate_control_plane_http_action(&action, http)?;
    let request_fingerprint = control_plane_request_fingerprint(http, body_digest)
        .map_err(ApplicationControlPlaneIdempotencyError::RequestFingerprintInvalid)?;
    plan_application_control_plane_idempotency_from_http(action, http, request_fingerprint)
}

pub fn plan_application_control_plane_page_request_from_http_query(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
    query: impl AsRef<str>,
) -> Result<ApplicationControlPlanePageRequestPlan, ApplicationControlPlanePageRequestError> {
    validate_control_plane_http_action_for_page_request(&action, http)?;
    let page_request = page_request_from_query(query.as_ref())
        .map_err(ApplicationControlPlanePageRequestError::Pagination)?;
    Ok(ApplicationControlPlanePageRequestPlan {
        action,
        page_request,
    })
}

pub fn plan_application_control_plane_precondition_from_http(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlanePreconditionPlan, ApplicationControlPlanePreconditionError> {
    validate_control_plane_http_action_for_precondition(&action, http)?;
    let entity_tag = entity_tag_from_if_match_headers(&http.headers)
        .map_err(ApplicationControlPlanePreconditionError::EntityTag)?;
    Ok(ApplicationControlPlanePreconditionPlan { action, entity_tag })
}

fn validate_control_plane_http_action(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<(), ApplicationControlPlaneIdempotencyError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlaneIdempotencyError::HttpRoute)?;
    if route.operation != action.operation {
        if control_plane_operations_share_route_family(route.operation, action.operation)
            && !control_plane_operation_allows_http_method(action.operation, http.method)
        {
            return Err(ApplicationControlPlaneIdempotencyError::HttpRoute(
                ApplicationControlPlaneHttpRouteError::Route(
                    GatewayControlPlaneRouteError::MethodNotAllowed {
                        operation: gateway_operation_from_control_plane_action(action.operation),
                        method: http.method,
                    },
                ),
            ));
        }
        return Err(ApplicationControlPlaneIdempotencyError::OperationMismatch {
            route_operation: route.operation,
            action_operation: action.operation,
        });
    }
    Ok(())
}

fn validate_control_plane_http_action_for_page_request(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<(), ApplicationControlPlanePageRequestError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlanePageRequestError::HttpRoute)?;
    if route.operation != action.operation {
        return Err(ApplicationControlPlanePageRequestError::OperationMismatch {
            route_operation: route.operation,
            action_operation: action.operation,
        });
    }
    Ok(())
}

fn validate_control_plane_http_action_for_precondition(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<(), ApplicationControlPlanePreconditionError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlanePreconditionError::HttpRoute)?;
    if route.operation != action.operation {
        return Err(
            ApplicationControlPlanePreconditionError::OperationMismatch {
                route_operation: route.operation,
                action_operation: action.operation,
            },
        );
    }
    Ok(())
}

fn validate_control_plane_http_action_for_audit(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneHttpRoutePlan, ApplicationControlPlaneAuditError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlaneAuditError::HttpRoute)?;
    if route.operation != action.operation {
        return Err(ApplicationControlPlaneAuditError::OperationMismatch {
            route_operation: route.operation,
            action_operation: action.operation,
        });
    }
    if !route.http.requires_audit || !action.operation.requires_immutable_audit() {
        return Err(ApplicationControlPlaneAuditError::AuditNotRequired {
            route_operation: route.http.operation,
            action_operation: action.operation,
        });
    }
    Ok(route)
}

fn control_plane_operation_from_gateway_route(
    operation: GatewayControlPlaneOperation,
) -> ControlPlaneOperation {
    match operation {
        GatewayControlPlaneOperation::GatewayAdminRead => ControlPlaneOperation::GatewayAdminRead,
        GatewayControlPlaneOperation::TenantCreate => ControlPlaneOperation::TenantCreate,
        GatewayControlPlaneOperation::TenantUpdate => ControlPlaneOperation::TenantUpdate,
        GatewayControlPlaneOperation::UserInvite => ControlPlaneOperation::UserInvite,
        GatewayControlPlaneOperation::ScimUserRead => ControlPlaneOperation::ScimUserRead,
        GatewayControlPlaneOperation::ScimUserCreate => ControlPlaneOperation::ScimUserCreate,
        GatewayControlPlaneOperation::ScimUserUpdate => ControlPlaneOperation::ScimUserUpdate,
        GatewayControlPlaneOperation::ScimUserDelete => ControlPlaneOperation::ScimUserDelete,
        GatewayControlPlaneOperation::RoleBindingGrant => ControlPlaneOperation::RoleBindingGrant,
        GatewayControlPlaneOperation::RoleBindingRevoke => ControlPlaneOperation::RoleBindingRevoke,
        GatewayControlPlaneOperation::ServiceIdentityCreate => {
            ControlPlaneOperation::ServiceIdentityCreate
        }
        GatewayControlPlaneOperation::VirtualKeyRead => ControlPlaneOperation::VirtualKeyRead,
        GatewayControlPlaneOperation::VirtualKeyCreate => ControlPlaneOperation::VirtualKeyCreate,
        GatewayControlPlaneOperation::VirtualKeyUpdate => ControlPlaneOperation::VirtualKeyUpdate,
        GatewayControlPlaneOperation::VirtualKeyDelete => ControlPlaneOperation::VirtualKeyDelete,
        GatewayControlPlaneOperation::VirtualKeyRotateSecret => {
            ControlPlaneOperation::VirtualKeyRotateSecret
        }
        GatewayControlPlaneOperation::PolicyPublish => ControlPlaneOperation::PolicyPublish,
        GatewayControlPlaneOperation::ProviderCredentialRotate => {
            ControlPlaneOperation::ProviderCredentialRotate
        }
        GatewayControlPlaneOperation::BudgetUpdate => ControlPlaneOperation::BudgetUpdate,
        GatewayControlPlaneOperation::BillingRead => ControlPlaneOperation::BillingRead,
        GatewayControlPlaneOperation::AuditExport => ControlPlaneOperation::AuditExport,
        GatewayControlPlaneOperation::AuditRetentionPurge => {
            ControlPlaneOperation::AuditRetentionPurge
        }
        GatewayControlPlaneOperation::ConfigurationPublish => {
            ControlPlaneOperation::ConfigurationPublish
        }
    }
}

fn control_plane_operations_share_route_family(
    route_operation: ControlPlaneOperation,
    action_operation: ControlPlaneOperation,
) -> bool {
    use ControlPlaneOperation::*;

    matches!(
        (route_operation, action_operation),
        (GatewayAdminRead, GatewayAdminRead)
            | (TenantCreate | TenantUpdate, TenantCreate | TenantUpdate)
            | (UserInvite, UserInvite)
            | (
                ScimUserRead | ScimUserCreate | ScimUserUpdate | ScimUserDelete,
                ScimUserRead | ScimUserCreate | ScimUserUpdate | ScimUserDelete
            )
            | (
                RoleBindingGrant | RoleBindingRevoke,
                RoleBindingGrant | RoleBindingRevoke
            )
            | (ServiceIdentityCreate, ServiceIdentityCreate)
            | (
                VirtualKeyRead
                    | VirtualKeyCreate
                    | VirtualKeyUpdate
                    | VirtualKeyDelete
                    | VirtualKeyRotateSecret,
                VirtualKeyRead
                    | VirtualKeyCreate
                    | VirtualKeyUpdate
                    | VirtualKeyDelete
                    | VirtualKeyRotateSecret
            )
            | (ProviderCredentialRotate, ProviderCredentialRotate)
            | (BudgetUpdate, BudgetUpdate)
            | (PolicyPublish, PolicyPublish)
            | (ConfigurationPublish, ConfigurationPublish)
            | (BillingRead, BillingRead)
            | (
                AuditExport | AuditRetentionPurge,
                AuditExport | AuditRetentionPurge
            )
    )
}

fn gateway_operation_from_control_plane_action(
    operation: ControlPlaneOperation,
) -> GatewayControlPlaneOperation {
    match operation {
        ControlPlaneOperation::GatewayAdminRead => GatewayControlPlaneOperation::GatewayAdminRead,
        ControlPlaneOperation::TenantCreate => GatewayControlPlaneOperation::TenantCreate,
        ControlPlaneOperation::TenantUpdate => GatewayControlPlaneOperation::TenantUpdate,
        ControlPlaneOperation::UserInvite => GatewayControlPlaneOperation::UserInvite,
        ControlPlaneOperation::ScimUserRead => GatewayControlPlaneOperation::ScimUserRead,
        ControlPlaneOperation::ScimUserCreate => GatewayControlPlaneOperation::ScimUserCreate,
        ControlPlaneOperation::ScimUserUpdate => GatewayControlPlaneOperation::ScimUserUpdate,
        ControlPlaneOperation::ScimUserDelete => GatewayControlPlaneOperation::ScimUserDelete,
        ControlPlaneOperation::RoleBindingGrant => GatewayControlPlaneOperation::RoleBindingGrant,
        ControlPlaneOperation::RoleBindingRevoke => GatewayControlPlaneOperation::RoleBindingRevoke,
        ControlPlaneOperation::ServiceIdentityCreate => {
            GatewayControlPlaneOperation::ServiceIdentityCreate
        }
        ControlPlaneOperation::VirtualKeyRead => GatewayControlPlaneOperation::VirtualKeyRead,
        ControlPlaneOperation::VirtualKeyCreate => GatewayControlPlaneOperation::VirtualKeyCreate,
        ControlPlaneOperation::VirtualKeyUpdate => GatewayControlPlaneOperation::VirtualKeyUpdate,
        ControlPlaneOperation::VirtualKeyDelete => GatewayControlPlaneOperation::VirtualKeyDelete,
        ControlPlaneOperation::VirtualKeyRotateSecret => {
            GatewayControlPlaneOperation::VirtualKeyRotateSecret
        }
        ControlPlaneOperation::ProviderCredentialRotate => {
            GatewayControlPlaneOperation::ProviderCredentialRotate
        }
        ControlPlaneOperation::BudgetUpdate => GatewayControlPlaneOperation::BudgetUpdate,
        ControlPlaneOperation::PolicyPublish => GatewayControlPlaneOperation::PolicyPublish,
        ControlPlaneOperation::ConfigurationPublish => {
            GatewayControlPlaneOperation::ConfigurationPublish
        }
        ControlPlaneOperation::BillingRead => GatewayControlPlaneOperation::BillingRead,
        ControlPlaneOperation::AuditExport => GatewayControlPlaneOperation::AuditExport,
        ControlPlaneOperation::AuditRetentionPurge => {
            GatewayControlPlaneOperation::AuditRetentionPurge
        }
    }
}

fn control_plane_operation_allows_http_method(
    operation: ControlPlaneOperation,
    method: prodex_gateway_http::GatewayHttpMethod,
) -> bool {
    use prodex_gateway_http::GatewayHttpMethod::{Delete, Get, Patch, Post};

    match operation {
        ControlPlaneOperation::GatewayAdminRead
        | ControlPlaneOperation::ScimUserRead
        | ControlPlaneOperation::VirtualKeyRead
        | ControlPlaneOperation::BillingRead => method == Get,
        ControlPlaneOperation::TenantCreate
        | ControlPlaneOperation::UserInvite
        | ControlPlaneOperation::ScimUserCreate
        | ControlPlaneOperation::RoleBindingGrant
        | ControlPlaneOperation::ServiceIdentityCreate
        | ControlPlaneOperation::VirtualKeyCreate
        | ControlPlaneOperation::VirtualKeyRotateSecret
        | ControlPlaneOperation::ProviderCredentialRotate
        | ControlPlaneOperation::PolicyPublish
        | ControlPlaneOperation::ConfigurationPublish
        | ControlPlaneOperation::AuditExport => method == Post,
        ControlPlaneOperation::TenantUpdate
        | ControlPlaneOperation::ScimUserUpdate
        | ControlPlaneOperation::VirtualKeyUpdate
        | ControlPlaneOperation::BudgetUpdate => method == Patch,
        ControlPlaneOperation::ScimUserDelete
        | ControlPlaneOperation::RoleBindingRevoke
        | ControlPlaneOperation::VirtualKeyDelete
        | ControlPlaneOperation::AuditRetentionPurge => method == Delete,
    }
}

pub fn plan_application_control_plane_idempotency_replay<R: Clone>(
    operation: &IdempotentOperation,
    existing: Option<&IdempotencyEntry<R>>,
) -> Result<IdempotencyReplayDecision<R>, ApplicationControlPlaneIdempotencyError> {
    decide_idempotency_replay(operation, existing)
        .map_err(ApplicationControlPlaneIdempotencyError::ReplayConflict)
}

pub fn plan_application_control_plane_idempotency_replay_from_lookup_row(
    operation: &IdempotentOperation,
    row: Option<IdempotencyRecordLookupRow>,
) -> Result<IdempotencyReplayDecision<Vec<u8>>, ApplicationControlPlaneIdempotencyError> {
    let entry = row
        .map(|row| materialize_idempotency_record_lookup_row(operation, row))
        .transpose()
        .map_err(ApplicationControlPlaneIdempotencyError::LookupRowInvalid)?;
    plan_application_control_plane_idempotency_replay(operation, entry.as_ref())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyStoragePrepareRequest {
    pub durable_store: DurableStoreKind,
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
    pub started_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyLookupStoragePlan {
    Postgres(PostgresIdempotencyRecordLookupSqlPlan),
    Sqlite(SqliteIdempotencyRecordLookupSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyPendingStoragePlan {
    Postgres(PostgresIdempotencyPendingRecordSqlPlan),
    Sqlite(SqliteIdempotencyPendingRecordSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyStoragePreparePlan {
    pub lookup: ApplicationControlPlaneIdempotencyLookupStoragePlan,
    pub pending: ApplicationControlPlaneIdempotencyPendingStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyStorageCompleteRequest {
    pub durable_store: DurableStoreKind,
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
    pub completed_at_unix_ms: u64,
    pub response_body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyCompletedStoragePlan {
    Postgres(PostgresIdempotencyCompletedRecordSqlPlan),
    Sqlite(SqliteIdempotencyCompletedRecordSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyStorageCompletePlan {
    pub completed: ApplicationControlPlaneIdempotencyCompletedStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyStorageError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationControlPlaneIdempotencyStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlaneIdempotencyStorageError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyStorageErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyStorageErrorResponsePlan {
    pub status: ApplicationControlPlaneIdempotencyStorageErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_control_plane_idempotency_storage_error_response(
    error: &ApplicationControlPlaneIdempotencyStorageError,
) -> ApplicationControlPlaneIdempotencyStorageErrorResponsePlan {
    match error {
        ApplicationControlPlaneIdempotencyStorageError::Postgres(error) => {
            application_control_plane_idempotency_storage_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationControlPlaneIdempotencyStorageError::Sqlite(error) => {
            application_control_plane_idempotency_storage_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_control_plane_idempotency_storage_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationControlPlaneIdempotencyStorageErrorResponsePlan {
    ApplicationControlPlaneIdempotencyStorageErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationControlPlaneIdempotencyStorageErrorStatus::ServiceUnavailable
            }
        },
        code: "control_plane_idempotency_storage_unavailable",
        message: "control-plane idempotency storage is temporarily unavailable",
    }
}

fn application_control_plane_idempotency_storage_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationControlPlaneIdempotencyStorageErrorResponsePlan {
    ApplicationControlPlaneIdempotencyStorageErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationControlPlaneIdempotencyStorageErrorStatus::ServiceUnavailable
            }
        },
        code: "control_plane_idempotency_storage_unavailable",
        message: "control-plane idempotency storage is temporarily unavailable",
    }
}

pub fn plan_application_control_plane_idempotency_storage_prepare(
    request: ApplicationControlPlaneIdempotencyStoragePrepareRequest,
) -> Result<
    ApplicationControlPlaneIdempotencyStoragePreparePlan,
    ApplicationControlPlaneIdempotencyStorageError,
> {
    let lookup_command = IdempotencyRecordLookupCommand {
        storage_key: request.storage_key,
        operation: request.operation.clone(),
    };
    let pending_command = IdempotencyPendingRecordCommand {
        storage_key: request.storage_key,
        operation: request.operation,
        started_at_unix_ms: request.started_at_unix_ms,
    };
    match request.durable_store {
        DurableStoreKind::Postgres => {
            let lookup = plan_postgres_idempotency_record_lookup(lookup_command)
                .map_err(ApplicationControlPlaneIdempotencyStorageError::Postgres)?;
            let pending = plan_postgres_idempotency_pending_record(pending_command)
                .map_err(ApplicationControlPlaneIdempotencyStorageError::Postgres)?;
            Ok(ApplicationControlPlaneIdempotencyStoragePreparePlan {
                lookup: ApplicationControlPlaneIdempotencyLookupStoragePlan::Postgres(lookup),
                pending: ApplicationControlPlaneIdempotencyPendingStoragePlan::Postgres(pending),
            })
        }
        DurableStoreKind::Sqlite => {
            let lookup = plan_sqlite_idempotency_record_lookup(lookup_command)
                .map_err(ApplicationControlPlaneIdempotencyStorageError::Sqlite)?;
            let pending = plan_sqlite_idempotency_pending_record(pending_command)
                .map_err(ApplicationControlPlaneIdempotencyStorageError::Sqlite)?;
            Ok(ApplicationControlPlaneIdempotencyStoragePreparePlan {
                lookup: ApplicationControlPlaneIdempotencyLookupStoragePlan::Sqlite(lookup),
                pending: ApplicationControlPlaneIdempotencyPendingStoragePlan::Sqlite(pending),
            })
        }
    }
}

pub fn plan_application_control_plane_idempotency_storage_complete(
    request: ApplicationControlPlaneIdempotencyStorageCompleteRequest,
) -> Result<
    ApplicationControlPlaneIdempotencyStorageCompletePlan,
    ApplicationControlPlaneIdempotencyStorageError,
> {
    let command = IdempotencyCompletedRecordCommand {
        storage_key: request.storage_key,
        operation: request.operation,
        completed_at_unix_ms: request.completed_at_unix_ms,
        response_body: request.response_body,
    };
    let completed = match request.durable_store {
        DurableStoreKind::Postgres => {
            ApplicationControlPlaneIdempotencyCompletedStoragePlan::Postgres(
                plan_postgres_idempotency_completed_record(command)
                    .map_err(ApplicationControlPlaneIdempotencyStorageError::Postgres)?,
            )
        }
        DurableStoreKind::Sqlite => ApplicationControlPlaneIdempotencyCompletedStoragePlan::Sqlite(
            plan_sqlite_idempotency_completed_record(command)
                .map_err(ApplicationControlPlaneIdempotencyStorageError::Sqlite)?,
        ),
    };
    Ok(ApplicationControlPlaneIdempotencyStorageCompletePlan { completed })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAuditRetentionPurgeRequest {
    pub durable_store: DurableStoreKind,
    pub purge: AuditRetentionPurgeCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationAuditRetentionPurgeStoragePlan {
    Postgres(PostgresAuditRetentionPurgeSqlPlan),
    Sqlite(SqliteAuditRetentionPurgeSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAuditRetentionPurgePlan {
    pub storage: ApplicationAuditRetentionPurgeStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationAuditRetentionPurgeError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationAuditRetentionPurgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationAuditRetentionPurgeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationAuditRetentionPurgeErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAuditRetentionPurgeErrorResponsePlan {
    pub status: ApplicationAuditRetentionPurgeErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_audit_retention_purge_error_response(
    error: &ApplicationAuditRetentionPurgeError,
) -> ApplicationAuditRetentionPurgeErrorResponsePlan {
    match error {
        ApplicationAuditRetentionPurgeError::Postgres(error) => {
            application_audit_retention_purge_response_from_postgres(
                plan_postgres_storage_error_response(error),
                "audit_retention_purge_storage_unavailable",
                "audit retention purge storage is temporarily unavailable",
            )
        }
        ApplicationAuditRetentionPurgeError::Sqlite(error) => {
            application_audit_retention_purge_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
                "audit_retention_purge_storage_unavailable",
                "audit retention purge storage is temporarily unavailable",
            )
        }
    }
}

fn application_audit_retention_purge_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationAuditRetentionPurgeErrorResponsePlan {
    ApplicationAuditRetentionPurgeErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationAuditRetentionPurgeErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_audit_retention_purge_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationAuditRetentionPurgeErrorResponsePlan {
    ApplicationAuditRetentionPurgeErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationAuditRetentionPurgeErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_audit_retention_purge(
    request: ApplicationAuditRetentionPurgeRequest,
) -> Result<ApplicationAuditRetentionPurgePlan, ApplicationAuditRetentionPurgeError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationAuditRetentionPurgeStoragePlan::Postgres(
            plan_postgres_audit_retention_purge(request.purge)
                .map_err(ApplicationAuditRetentionPurgeError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationAuditRetentionPurgeStoragePlan::Sqlite(
            plan_sqlite_audit_retention_purge(request.purge)
                .map_err(ApplicationAuditRetentionPurgeError::Sqlite)?,
        ),
    };
    Ok(ApplicationAuditRetentionPurgePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationAuditExportRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub export: AuditExportQueryCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationAuditExportRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationAuditExportRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("export", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationAuditExportQueryStoragePlan {
    Postgres(PostgresAuditExportQuerySqlPlan),
    Sqlite(SqliteAuditExportQuerySqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationAuditExportPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub query_storage: Option<ApplicationAuditExportQueryStoragePlan>,
}

impl fmt::Debug for ApplicationAuditExportPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationAuditExportPlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "query_storage",
                &self.query_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationAuditExportError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        query_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    Storage(prodex_storage_postgres::PostgresStoragePlanError),
    SqliteStorage(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationAuditExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("query_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::Storage(_) => f.debug_tuple("Storage").field(&"<redacted>").finish(),
            Self::SqliteStorage(_) => f.debug_tuple("SqliteStorage").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationAuditExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => write!(f, "control-plane operation is not audit export"),
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not audit log")
            }
            Self::TenantMismatch { .. } => write!(f, "audit export request is invalid"),
            Self::Audit(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::SqliteStorage(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationAuditExportError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationAuditExportErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAuditExportErrorResponsePlan {
    pub status: ApplicationAuditExportErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_audit_export_error_response(
    error: &ApplicationAuditExportError,
) -> ApplicationAuditExportErrorResponsePlan {
    match error {
        ApplicationAuditExportError::WrongOperation(_)
        | ApplicationAuditExportError::WrongResourceKind(_)
        | ApplicationAuditExportError::TenantMismatch { .. } => {
            ApplicationAuditExportErrorResponsePlan {
                status: ApplicationAuditExportErrorStatus::BadRequest,
                code: "audit_export_invalid",
                message: "audit export request is invalid",
            }
        }
        ApplicationAuditExportError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationAuditExportErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationAuditExportErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationAuditExportErrorStatus::ServiceUnavailable
                    }
                },
                code: "audit_export_audit_unavailable",
                message: "audit export audit is temporarily unavailable",
            }
        }
        ApplicationAuditExportError::Storage(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationAuditExportErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationAuditExportErrorStatus::ServiceUnavailable
                    }
                },
                code: "audit_export_storage_unavailable",
                message: "audit export storage is temporarily unavailable",
            }
        }
        ApplicationAuditExportError::SqliteStorage(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationAuditExportErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationAuditExportErrorStatus::ServiceUnavailable
                    }
                },
                code: "audit_export_storage_unavailable",
                message: "audit export storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_audit_export(
    request: ApplicationAuditExportRequest,
) -> Result<ApplicationAuditExportPlan, ApplicationAuditExportError> {
    if request.action.operation != ControlPlaneOperation::AuditExport {
        return Err(ApplicationAuditExportError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::AuditLog {
        return Err(ApplicationAuditExportError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    let query_tenant = request.export.export.query.scope.tenant_id;
    if request.action.resource.tenant_id != query_tenant {
        return Err(ApplicationAuditExportError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            query_tenant,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationAuditExportError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationAuditExportError::Audit)?,
        ),
    };
    let query_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        Some(match request.durable_store {
            DurableStoreKind::Postgres => ApplicationAuditExportQueryStoragePlan::Postgres(
                plan_postgres_audit_export_query(request.export)
                    .map_err(ApplicationAuditExportError::Storage)?,
            ),
            DurableStoreKind::Sqlite => ApplicationAuditExportQueryStoragePlan::Sqlite(
                plan_sqlite_audit_export_query(request.export)
                    .map_err(ApplicationAuditExportError::SqliteStorage)?,
            ),
        })
    } else {
        None
    };

    Ok(ApplicationAuditExportPlan {
        decision,
        audit_storage,
        query_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBillingReadRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub query: BillingLedgerQueryCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationBillingReadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBillingReadRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("query", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationBillingLedgerStoragePlan {
    Postgres(PostgresBillingLedgerQuerySqlPlan),
    Sqlite(SqliteBillingLedgerQuerySqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBillingReadPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub query_storage: Option<ApplicationBillingLedgerStoragePlan>,
}

impl fmt::Debug for ApplicationBillingReadPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBillingReadPlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "query_storage",
                &self.query_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationBillingReadError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        query_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    Storage(prodex_storage_postgres::PostgresStoragePlanError),
    SqliteStorage(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationBillingReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("query_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::Storage(_) => f.debug_tuple("Storage").field(&"<redacted>").finish(),
            Self::SqliteStorage(_) => f.debug_tuple("SqliteStorage").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationBillingReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => write!(f, "control-plane operation is not billing read"),
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not billing")
            }
            Self::TenantMismatch { .. } => write!(f, "billing read request is invalid"),
            Self::Audit(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::SqliteStorage(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationBillingReadError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationBillingReadErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBillingReadErrorResponsePlan {
    pub status: ApplicationBillingReadErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_billing_read_error_response(
    error: &ApplicationBillingReadError,
) -> ApplicationBillingReadErrorResponsePlan {
    match error {
        ApplicationBillingReadError::WrongOperation(_)
        | ApplicationBillingReadError::WrongResourceKind(_)
        | ApplicationBillingReadError::TenantMismatch { .. } => {
            ApplicationBillingReadErrorResponsePlan {
                status: ApplicationBillingReadErrorStatus::BadRequest,
                code: "billing_read_invalid",
                message: "billing read request is invalid",
            }
        }
        ApplicationBillingReadError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationBillingReadErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationBillingReadErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationBillingReadErrorStatus::ServiceUnavailable
                    }
                },
                code: "billing_read_audit_unavailable",
                message: "billing read audit is temporarily unavailable",
            }
        }
        ApplicationBillingReadError::Storage(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationBillingReadErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationBillingReadErrorStatus::ServiceUnavailable
                    }
                },
                code: "billing_read_storage_unavailable",
                message: "billing read storage is temporarily unavailable",
            }
        }
        ApplicationBillingReadError::SqliteStorage(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationBillingReadErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationBillingReadErrorStatus::ServiceUnavailable
                    }
                },
                code: "billing_read_storage_unavailable",
                message: "billing read storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_billing_read(
    request: ApplicationBillingReadRequest,
) -> Result<ApplicationBillingReadPlan, ApplicationBillingReadError> {
    if request.action.operation != ControlPlaneOperation::BillingRead {
        return Err(ApplicationBillingReadError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::Billing {
        return Err(ApplicationBillingReadError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.query.tenant_id {
        return Err(ApplicationBillingReadError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            query_tenant: request.query.tenant_id,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationBillingReadError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationBillingReadError::Audit)?,
        ),
    };
    let query_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        Some(match request.durable_store {
            DurableStoreKind::Postgres => ApplicationBillingLedgerStoragePlan::Postgres(
                plan_postgres_billing_ledger_query(request.query)
                    .map_err(ApplicationBillingReadError::Storage)?,
            ),
            DurableStoreKind::Sqlite => ApplicationBillingLedgerStoragePlan::Sqlite(
                plan_sqlite_billing_ledger_query(request.query)
                    .map_err(ApplicationBillingReadError::SqliteStorage)?,
            ),
        })
    } else {
        None
    };

    Ok(ApplicationBillingReadPlan {
        decision,
        audit_storage,
        query_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationTenantLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub tenant: TenantLifecycleCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationTenantLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationTenantLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("tenant", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationTenantLifecycleStoragePlan {
    Postgres(PostgresTenantLifecycleSqlPlan),
    Sqlite(SqliteTenantLifecycleSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationTenantLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub tenant_storage: Option<ApplicationTenantLifecycleStoragePlan>,
}

impl fmt::Debug for ApplicationTenantLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationTenantLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "tenant_storage",
                &self.tenant_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationTenantLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        command_tenant: TenantId,
    },
    KindMismatch {
        operation: ControlPlaneOperation,
        kind: TenantLifecycleKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    Storage(prodex_storage_postgres::PostgresStoragePlanError),
    SqliteStorage(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationTenantLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("command_tenant", &"<redacted>")
                .finish(),
            Self::KindMismatch { .. } => f
                .debug_struct("KindMismatch")
                .field("operation", &"<redacted>")
                .field("kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::Storage(_) => f.debug_tuple("Storage").field(&"<redacted>").finish(),
            Self::SqliteStorage(_) => f.debug_tuple("SqliteStorage").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationTenantLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(f, "control-plane operation is not tenant lifecycle")
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not tenant")
            }
            Self::TenantMismatch { .. } => write!(f, "tenant lifecycle request is invalid"),
            Self::KindMismatch { .. } => {
                write!(f, "tenant lifecycle operation does not match command kind")
            }
            Self::Audit(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::SqliteStorage(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationTenantLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationTenantLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationTenantLifecycleErrorResponsePlan {
    pub status: ApplicationTenantLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_tenant_lifecycle_error_response(
    error: &ApplicationTenantLifecycleError,
) -> ApplicationTenantLifecycleErrorResponsePlan {
    match error {
        ApplicationTenantLifecycleError::WrongOperation(_)
        | ApplicationTenantLifecycleError::WrongResourceKind(_)
        | ApplicationTenantLifecycleError::TenantMismatch { .. }
        | ApplicationTenantLifecycleError::KindMismatch { .. } => {
            ApplicationTenantLifecycleErrorResponsePlan {
                status: ApplicationTenantLifecycleErrorStatus::BadRequest,
                code: "tenant_lifecycle_invalid",
                message: "tenant lifecycle request is invalid",
            }
        }
        ApplicationTenantLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationTenantLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationTenantLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationTenantLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "tenant_lifecycle_audit_unavailable",
                message: "tenant lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationTenantLifecycleError::Storage(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationTenantLifecycleErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationTenantLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "tenant_lifecycle_storage_unavailable",
                message: "tenant lifecycle storage is temporarily unavailable",
            }
        }
        ApplicationTenantLifecycleError::SqliteStorage(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationTenantLifecycleErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationTenantLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "tenant_lifecycle_storage_unavailable",
                message: "tenant lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_tenant_lifecycle(
    request: ApplicationTenantLifecycleRequest,
) -> Result<ApplicationTenantLifecyclePlan, ApplicationTenantLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::TenantCreate | ControlPlaneOperation::TenantUpdate
    ) {
        return Err(ApplicationTenantLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::Tenant {
        return Err(ApplicationTenantLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.tenant.tenant_id {
        return Err(ApplicationTenantLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            command_tenant: request.tenant.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::TenantCreate => TenantLifecycleKind::Create,
        ControlPlaneOperation::TenantUpdate => TenantLifecycleKind::Update,
        _ => unreachable!("tenant lifecycle operation was already validated"),
    };
    if request.tenant.kind != expected_kind {
        return Err(ApplicationTenantLifecycleError::KindMismatch {
            operation: request.action.operation,
            kind: request.tenant.kind,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationTenantLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationTenantLifecycleError::Audit)?,
        ),
    };
    let tenant_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        Some(match request.durable_store {
            DurableStoreKind::Postgres => ApplicationTenantLifecycleStoragePlan::Postgres(
                plan_postgres_tenant_lifecycle(request.tenant)
                    .map_err(ApplicationTenantLifecycleError::Storage)?,
            ),
            DurableStoreKind::Sqlite => ApplicationTenantLifecycleStoragePlan::Sqlite(
                plan_sqlite_tenant_lifecycle(request.tenant)
                    .map_err(ApplicationTenantLifecycleError::SqliteStorage)?,
            ),
        })
    } else {
        None
    };

    Ok(ApplicationTenantLifecyclePlan {
        decision,
        audit_storage,
        tenant_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationUserLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub user: UserLifecycleCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationUserLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationUserLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("user", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationUserLifecycleStoragePlan {
    Postgres(PostgresUserLifecycleSqlPlan),
    Sqlite(SqliteUserLifecycleSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationUserLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub user_storage: Option<ApplicationUserLifecycleStoragePlan>,
}

impl fmt::Debug for ApplicationUserLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationUserLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "user_storage",
                &self.user_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationUserLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        command_tenant: TenantId,
    },
    KindMismatch {
        operation: ControlPlaneOperation,
        kind: UserLifecycleKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    Storage(prodex_storage_postgres::PostgresStoragePlanError),
    SqliteStorage(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationUserLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("command_tenant", &"<redacted>")
                .finish(),
            Self::KindMismatch { .. } => f
                .debug_struct("KindMismatch")
                .field("operation", &"<redacted>")
                .field("kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::Storage(_) => f.debug_tuple("Storage").field(&"<redacted>").finish(),
            Self::SqliteStorage(_) => f.debug_tuple("SqliteStorage").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationUserLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(f, "control-plane operation is not user lifecycle")
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not user")
            }
            Self::TenantMismatch { .. } => write!(f, "user lifecycle request is invalid"),
            Self::KindMismatch { .. } => {
                write!(f, "user lifecycle operation does not match command kind")
            }
            Self::Audit(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::SqliteStorage(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationUserLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationUserLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationUserLifecycleErrorResponsePlan {
    pub status: ApplicationUserLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_user_lifecycle_error_response(
    error: &ApplicationUserLifecycleError,
) -> ApplicationUserLifecycleErrorResponsePlan {
    match error {
        ApplicationUserLifecycleError::WrongOperation(_)
        | ApplicationUserLifecycleError::WrongResourceKind(_)
        | ApplicationUserLifecycleError::TenantMismatch { .. }
        | ApplicationUserLifecycleError::KindMismatch { .. } => {
            ApplicationUserLifecycleErrorResponsePlan {
                status: ApplicationUserLifecycleErrorStatus::BadRequest,
                code: "user_lifecycle_invalid",
                message: "user lifecycle request is invalid",
            }
        }
        ApplicationUserLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationUserLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationUserLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationUserLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "user_lifecycle_audit_unavailable",
                message: "user lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationUserLifecycleError::Storage(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationUserLifecycleErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationUserLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "user_lifecycle_storage_unavailable",
                message: "user lifecycle storage is temporarily unavailable",
            }
        }
        ApplicationUserLifecycleError::SqliteStorage(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationUserLifecycleErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationUserLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "user_lifecycle_storage_unavailable",
                message: "user lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_user_lifecycle(
    request: ApplicationUserLifecycleRequest,
) -> Result<ApplicationUserLifecyclePlan, ApplicationUserLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::UserInvite
            | ControlPlaneOperation::ScimUserCreate
            | ControlPlaneOperation::ScimUserUpdate
            | ControlPlaneOperation::ScimUserDelete
    ) {
        return Err(ApplicationUserLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::User {
        return Err(ApplicationUserLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.user.tenant_id {
        return Err(ApplicationUserLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            command_tenant: request.user.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::UserInvite | ControlPlaneOperation::ScimUserCreate => {
            UserLifecycleKind::Create
        }
        ControlPlaneOperation::ScimUserUpdate => UserLifecycleKind::Update,
        ControlPlaneOperation::ScimUserDelete => UserLifecycleKind::Delete,
        _ => unreachable!("user lifecycle operation was already validated"),
    };
    if request.user.kind != expected_kind {
        return Err(ApplicationUserLifecycleError::KindMismatch {
            operation: request.action.operation,
            kind: request.user.kind,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationUserLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationUserLifecycleError::Audit)?,
        ),
    };
    let user_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        Some(match request.durable_store {
            DurableStoreKind::Postgres => ApplicationUserLifecycleStoragePlan::Postgres(
                plan_postgres_user_lifecycle(request.user)
                    .map_err(ApplicationUserLifecycleError::Storage)?,
            ),
            DurableStoreKind::Sqlite => ApplicationUserLifecycleStoragePlan::Sqlite(
                plan_sqlite_user_lifecycle(request.user)
                    .map_err(ApplicationUserLifecycleError::SqliteStorage)?,
            ),
        })
    } else {
        None
    };

    Ok(ApplicationUserLifecyclePlan {
        decision,
        audit_storage,
        user_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityCreateRequest {
    pub durable_store: DurableStoreKind,
    pub identity: ServiceIdentityCreateCommand,
}

impl fmt::Debug for ApplicationServiceIdentityCreateRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityCreateRequest")
            .field("durable_store", &self.durable_store)
            .field("identity", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationServiceIdentityCreateStoragePlan {
    Postgres(PostgresServiceIdentityCreateSqlPlan),
    Sqlite(SqliteServiceIdentityCreateSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityCreatePlan {
    pub storage: ApplicationServiceIdentityCreateStoragePlan,
}

impl fmt::Debug for ApplicationServiceIdentityCreatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityCreatePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationServiceIdentityCreateError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationServiceIdentityCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationServiceIdentityCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationServiceIdentityCreateError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationServiceIdentityCreateErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationServiceIdentityCreateErrorResponsePlan {
    pub status: ApplicationServiceIdentityCreateErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_service_identity_create_error_response(
    error: &ApplicationServiceIdentityCreateError,
) -> ApplicationServiceIdentityCreateErrorResponsePlan {
    match error {
        ApplicationServiceIdentityCreateError::Postgres(error) => {
            application_service_identity_create_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationServiceIdentityCreateError::Sqlite(error) => {
            application_service_identity_create_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_service_identity_create_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationServiceIdentityCreateErrorResponsePlan {
    ApplicationServiceIdentityCreateErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationServiceIdentityCreateErrorStatus::ServiceUnavailable
            }
        },
        code: "service_identity_storage_unavailable",
        message: "service identity storage is temporarily unavailable",
    }
}

fn application_service_identity_create_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationServiceIdentityCreateErrorResponsePlan {
    ApplicationServiceIdentityCreateErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationServiceIdentityCreateErrorStatus::ServiceUnavailable
            }
        },
        code: "service_identity_storage_unavailable",
        message: "service identity storage is temporarily unavailable",
    }
}

pub fn plan_application_service_identity_create(
    request: ApplicationServiceIdentityCreateRequest,
) -> Result<ApplicationServiceIdentityCreatePlan, ApplicationServiceIdentityCreateError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationServiceIdentityCreateStoragePlan::Postgres(
            plan_postgres_service_identity_create(request.identity)
                .map_err(ApplicationServiceIdentityCreateError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationServiceIdentityCreateStoragePlan::Sqlite(
            plan_sqlite_service_identity_create(request.identity)
                .map_err(ApplicationServiceIdentityCreateError::Sqlite)?,
        ),
    };
    Ok(ApplicationServiceIdentityCreatePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub identity: ServiceIdentityCreateCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationServiceIdentityLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("identity", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub identity_storage: Option<ApplicationServiceIdentityCreateStoragePlan>,
}

impl fmt::Debug for ApplicationServiceIdentityLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "identity_storage",
                &self.identity_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationServiceIdentityLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        identity_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    ServiceIdentityCreate(ApplicationServiceIdentityCreateError),
}

impl fmt::Debug for ApplicationServiceIdentityLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("identity_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::ServiceIdentityCreate(_) => f
                .debug_tuple("ServiceIdentityCreate")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationServiceIdentityLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not service identity creation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not service identity")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "service identity lifecycle request is invalid")
            }
            Self::Audit(error) => error.fmt(f),
            Self::ServiceIdentityCreate(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationServiceIdentityLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationServiceIdentityLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationServiceIdentityLifecycleErrorResponsePlan {
    pub status: ApplicationServiceIdentityLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_service_identity_lifecycle_error_response(
    error: &ApplicationServiceIdentityLifecycleError,
) -> ApplicationServiceIdentityLifecycleErrorResponsePlan {
    match error {
        ApplicationServiceIdentityLifecycleError::WrongOperation(_)
        | ApplicationServiceIdentityLifecycleError::WrongResourceKind(_)
        | ApplicationServiceIdentityLifecycleError::TenantMismatch { .. } => {
            ApplicationServiceIdentityLifecycleErrorResponsePlan {
                status: ApplicationServiceIdentityLifecycleErrorStatus::BadRequest,
                code: "service_identity_lifecycle_invalid",
                message: "service identity lifecycle request is invalid",
            }
        }
        ApplicationServiceIdentityLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationServiceIdentityLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationServiceIdentityLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationServiceIdentityLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "service_identity_lifecycle_audit_unavailable",
                message: "service identity lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationServiceIdentityLifecycleError::ServiceIdentityCreate(error) => {
            let response = plan_application_service_identity_create_error_response(error);
            ApplicationServiceIdentityLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationServiceIdentityCreateErrorStatus::ServiceUnavailable => {
                        ApplicationServiceIdentityLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "service_identity_lifecycle_storage_unavailable",
                message: "service identity lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_service_identity_lifecycle(
    request: ApplicationServiceIdentityLifecycleRequest,
) -> Result<ApplicationServiceIdentityLifecyclePlan, ApplicationServiceIdentityLifecycleError> {
    if request.action.operation != ControlPlaneOperation::ServiceIdentityCreate {
        return Err(ApplicationServiceIdentityLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::ServiceIdentity {
        return Err(ApplicationServiceIdentityLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.identity.tenant_id {
        return Err(ApplicationServiceIdentityLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            identity_tenant: request.identity.tenant_id,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationServiceIdentityLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationServiceIdentityLifecycleError::Audit)?,
        ),
    };
    let identity_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let identity_plan =
            plan_application_service_identity_create(ApplicationServiceIdentityCreateRequest {
                durable_store: request.durable_store,
                identity: request.identity,
            })
            .map_err(ApplicationServiceIdentityLifecycleError::ServiceIdentityCreate)?;
        Some(identity_plan.storage)
    } else {
        None
    };

    Ok(ApplicationServiceIdentityLifecyclePlan {
        decision,
        audit_storage,
        identity_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyUpdateRequest {
    pub durable_store: DurableStoreKind,
    pub policy: BudgetPolicyUpdateCommand,
}

impl fmt::Debug for ApplicationBudgetPolicyUpdateRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyUpdateRequest")
            .field("durable_store", &self.durable_store)
            .field("policy", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyUpdateStoragePlan {
    Postgres(PostgresBudgetPolicyUpdateSqlPlan),
    Sqlite(SqliteBudgetPolicyUpdateSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyUpdatePlan {
    pub storage: ApplicationBudgetPolicyUpdateStoragePlan,
}

impl fmt::Debug for ApplicationBudgetPolicyUpdatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyUpdatePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyUpdateError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationBudgetPolicyUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationBudgetPolicyUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationBudgetPolicyUpdateError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyUpdateErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyUpdateErrorResponsePlan {
    pub status: ApplicationBudgetPolicyUpdateErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_budget_policy_update_error_response(
    error: &ApplicationBudgetPolicyUpdateError,
) -> ApplicationBudgetPolicyUpdateErrorResponsePlan {
    match error {
        ApplicationBudgetPolicyUpdateError::Postgres(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationBudgetPolicyUpdateErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyUpdateErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_storage_unavailable",
                message: "budget policy storage is temporarily unavailable",
            }
        }
        ApplicationBudgetPolicyUpdateError::Sqlite(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationBudgetPolicyUpdateErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyUpdateErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_storage_unavailable",
                message: "budget policy storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_budget_policy_update(
    request: ApplicationBudgetPolicyUpdateRequest,
) -> Result<ApplicationBudgetPolicyUpdatePlan, ApplicationBudgetPolicyUpdateError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationBudgetPolicyUpdateStoragePlan::Postgres(
            plan_postgres_budget_policy_update(request.policy)
                .map_err(ApplicationBudgetPolicyUpdateError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationBudgetPolicyUpdateStoragePlan::Sqlite(
            plan_sqlite_budget_policy_update(request.policy)
                .map_err(ApplicationBudgetPolicyUpdateError::Sqlite)?,
        ),
    };
    Ok(ApplicationBudgetPolicyUpdatePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub policy: BudgetPolicyUpdateCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationBudgetPolicyLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("policy", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub policy_storage: Option<ApplicationBudgetPolicyUpdateStoragePlan>,
}

impl fmt::Debug for ApplicationBudgetPolicyLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "policy_storage",
                &self.policy_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        policy_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    BudgetPolicyUpdate(ApplicationBudgetPolicyUpdateError),
}

impl fmt::Debug for ApplicationBudgetPolicyLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("policy_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::BudgetPolicyUpdate(_) => f
                .debug_tuple("BudgetPolicyUpdate")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationBudgetPolicyLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(f, "control-plane operation is not budget policy update")
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not budget")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "budget policy lifecycle request is invalid")
            }
            Self::Audit(error) => error.fmt(f),
            Self::BudgetPolicyUpdate(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationBudgetPolicyLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyLifecycleErrorResponsePlan {
    pub status: ApplicationBudgetPolicyLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_budget_policy_lifecycle_error_response(
    error: &ApplicationBudgetPolicyLifecycleError,
) -> ApplicationBudgetPolicyLifecycleErrorResponsePlan {
    match error {
        ApplicationBudgetPolicyLifecycleError::WrongOperation(_)
        | ApplicationBudgetPolicyLifecycleError::WrongResourceKind(_)
        | ApplicationBudgetPolicyLifecycleError::TenantMismatch { .. } => {
            ApplicationBudgetPolicyLifecycleErrorResponsePlan {
                status: ApplicationBudgetPolicyLifecycleErrorStatus::BadRequest,
                code: "budget_policy_lifecycle_invalid",
                message: "budget policy lifecycle request is invalid",
            }
        }
        ApplicationBudgetPolicyLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationBudgetPolicyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationBudgetPolicyLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_lifecycle_audit_unavailable",
                message: "budget policy lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationBudgetPolicyLifecycleError::BudgetPolicyUpdate(error) => {
            let response = plan_application_budget_policy_update_error_response(error);
            ApplicationBudgetPolicyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationBudgetPolicyUpdateErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_lifecycle_storage_unavailable",
                message: "budget policy lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_budget_policy_lifecycle(
    request: ApplicationBudgetPolicyLifecycleRequest,
) -> Result<ApplicationBudgetPolicyLifecyclePlan, ApplicationBudgetPolicyLifecycleError> {
    if request.action.operation != ControlPlaneOperation::BudgetUpdate {
        return Err(ApplicationBudgetPolicyLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::Budget {
        return Err(ApplicationBudgetPolicyLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.policy.tenant_id {
        return Err(ApplicationBudgetPolicyLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            policy_tenant: request.policy.tenant_id,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationBudgetPolicyLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationBudgetPolicyLifecycleError::Audit)?,
        ),
    };
    let policy_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let policy_plan =
            plan_application_budget_policy_update(ApplicationBudgetPolicyUpdateRequest {
                durable_store: request.durable_store,
                policy: request.policy,
            })
            .map_err(ApplicationBudgetPolicyLifecycleError::BudgetPolicyUpdate)?;
        Some(policy_plan.storage)
    } else {
        None
    };

    Ok(ApplicationBudgetPolicyLifecyclePlan {
        decision,
        audit_storage,
        policy_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingMutationRequest {
    pub durable_store: DurableStoreKind,
    pub mutation: RoleBindingMutationCommand,
}

impl fmt::Debug for ApplicationRoleBindingMutationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingMutationRequest")
            .field("durable_store", &self.durable_store)
            .field("mutation", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRoleBindingMutationStoragePlan {
    Postgres(PostgresRoleBindingMutationSqlPlan),
    Sqlite(SqliteRoleBindingMutationSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingMutationPlan {
    pub storage: ApplicationRoleBindingMutationStoragePlan,
}

impl fmt::Debug for ApplicationRoleBindingMutationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingMutationPlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRoleBindingMutationError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationRoleBindingMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationRoleBindingMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationRoleBindingMutationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRoleBindingMutationErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRoleBindingMutationErrorResponsePlan {
    pub status: ApplicationRoleBindingMutationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_role_binding_mutation_error_response(
    error: &ApplicationRoleBindingMutationError,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    match error {
        ApplicationRoleBindingMutationError::Postgres(error) => {
            application_role_binding_mutation_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationRoleBindingMutationError::Sqlite(error) => {
            application_role_binding_mutation_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_role_binding_mutation_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    ApplicationRoleBindingMutationErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable
            }
        },
        code: "role_binding_storage_unavailable",
        message: "role-binding storage is temporarily unavailable",
    }
}

fn application_role_binding_mutation_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    ApplicationRoleBindingMutationErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable
            }
        },
        code: "role_binding_storage_unavailable",
        message: "role-binding storage is temporarily unavailable",
    }
}

pub fn plan_application_role_binding_mutation(
    request: ApplicationRoleBindingMutationRequest,
) -> Result<ApplicationRoleBindingMutationPlan, ApplicationRoleBindingMutationError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationRoleBindingMutationStoragePlan::Postgres(
            plan_postgres_role_binding_mutation(request.mutation)
                .map_err(ApplicationRoleBindingMutationError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationRoleBindingMutationStoragePlan::Sqlite(
            plan_sqlite_role_binding_mutation(request.mutation)
                .map_err(ApplicationRoleBindingMutationError::Sqlite)?,
        ),
    };
    Ok(ApplicationRoleBindingMutationPlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub mutation: RoleBindingMutationCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationRoleBindingLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("mutation", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub mutation_storage: Option<ApplicationRoleBindingMutationStoragePlan>,
}

impl fmt::Debug for ApplicationRoleBindingLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "mutation_storage",
                &self.mutation_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRoleBindingLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        mutation_tenant: TenantId,
    },
    MutationKindMismatch {
        operation: ControlPlaneOperation,
        mutation_kind: RoleBindingMutationKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    RoleBindingMutation(ApplicationRoleBindingMutationError),
}

impl fmt::Debug for ApplicationRoleBindingLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("mutation_tenant", &"<redacted>")
                .finish(),
            Self::MutationKindMismatch { .. } => f
                .debug_struct("MutationKindMismatch")
                .field("operation", &"<redacted>")
                .field("mutation_kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::RoleBindingMutation(_) => f
                .debug_tuple("RoleBindingMutation")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationRoleBindingLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not a role-binding lifecycle operation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not role binding")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "role-binding lifecycle request is invalid")
            }
            Self::MutationKindMismatch { .. } => {
                write!(
                    f,
                    "role-binding lifecycle operation does not match mutation kind"
                )
            }
            Self::Audit(error) => error.fmt(f),
            Self::RoleBindingMutation(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationRoleBindingLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRoleBindingLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRoleBindingLifecycleErrorResponsePlan {
    pub status: ApplicationRoleBindingLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_role_binding_lifecycle_error_response(
    error: &ApplicationRoleBindingLifecycleError,
) -> ApplicationRoleBindingLifecycleErrorResponsePlan {
    match error {
        ApplicationRoleBindingLifecycleError::WrongOperation(_)
        | ApplicationRoleBindingLifecycleError::WrongResourceKind(_)
        | ApplicationRoleBindingLifecycleError::TenantMismatch { .. }
        | ApplicationRoleBindingLifecycleError::MutationKindMismatch { .. } => {
            ApplicationRoleBindingLifecycleErrorResponsePlan {
                status: ApplicationRoleBindingLifecycleErrorStatus::BadRequest,
                code: "role_binding_lifecycle_invalid",
                message: "role-binding lifecycle request is invalid",
            }
        }
        ApplicationRoleBindingLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationRoleBindingLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationRoleBindingLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationRoleBindingLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "role_binding_lifecycle_audit_unavailable",
                message: "role-binding lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationRoleBindingLifecycleError::RoleBindingMutation(error) => {
            let response = plan_application_role_binding_mutation_error_response(error);
            ApplicationRoleBindingLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable => {
                        ApplicationRoleBindingLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "role_binding_lifecycle_storage_unavailable",
                message: "role-binding lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_role_binding_lifecycle(
    request: ApplicationRoleBindingLifecycleRequest,
) -> Result<ApplicationRoleBindingLifecyclePlan, ApplicationRoleBindingLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::RoleBindingGrant | ControlPlaneOperation::RoleBindingRevoke
    ) {
        return Err(ApplicationRoleBindingLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::RoleBinding {
        return Err(ApplicationRoleBindingLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.mutation.tenant_id {
        return Err(ApplicationRoleBindingLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            mutation_tenant: request.mutation.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::RoleBindingGrant => RoleBindingMutationKind::Grant,
        ControlPlaneOperation::RoleBindingRevoke => RoleBindingMutationKind::Revoke,
        _ => unreachable!("role-binding lifecycle operation was already validated"),
    };
    if request.mutation.kind != expected_kind {
        return Err(ApplicationRoleBindingLifecycleError::MutationKindMismatch {
            operation: request.action.operation,
            mutation_kind: request.mutation.kind,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationRoleBindingLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationRoleBindingLifecycleError::Audit)?,
        ),
    };
    let mutation_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let mutation_plan =
            plan_application_role_binding_mutation(ApplicationRoleBindingMutationRequest {
                durable_store: request.durable_store,
                mutation: request.mutation,
            })
            .map_err(ApplicationRoleBindingLifecycleError::RoleBindingMutation)?;
        Some(mutation_plan.storage)
    } else {
        None
    };

    Ok(ApplicationRoleBindingLifecyclePlan {
        decision,
        audit_storage,
        mutation_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeySecretReferenceRequest {
    pub durable_store: DurableStoreKind,
    pub reference: VirtualKeySecretReferenceCommand,
}

impl fmt::Debug for ApplicationVirtualKeySecretReferenceRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeySecretReferenceRequest")
            .field("durable_store", &self.durable_store)
            .field("reference", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationVirtualKeySecretReferenceStoragePlan {
    Postgres(PostgresVirtualKeySecretReferenceSqlPlan),
    Sqlite(SqliteVirtualKeySecretReferenceSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeySecretReferencePlan {
    pub storage: ApplicationVirtualKeySecretReferenceStoragePlan,
}

impl fmt::Debug for ApplicationVirtualKeySecretReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeySecretReferencePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationVirtualKeySecretReferenceError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationVirtualKeySecretReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationVirtualKeySecretReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationVirtualKeySecretReferenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationVirtualKeySecretReferenceErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    pub status: ApplicationVirtualKeySecretReferenceErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_virtual_key_secret_reference_error_response(
    error: &ApplicationVirtualKeySecretReferenceError,
) -> ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    match error {
        ApplicationVirtualKeySecretReferenceError::Postgres(error) => {
            application_virtual_key_secret_reference_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationVirtualKeySecretReferenceError::Sqlite(error) => {
            application_virtual_key_secret_reference_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_virtual_key_secret_reference_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    ApplicationVirtualKeySecretReferenceErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationVirtualKeySecretReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "virtual_key_secret_storage_unavailable",
        message: "virtual-key secret storage is temporarily unavailable",
    }
}

fn application_virtual_key_secret_reference_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    ApplicationVirtualKeySecretReferenceErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationVirtualKeySecretReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "virtual_key_secret_storage_unavailable",
        message: "virtual-key secret storage is temporarily unavailable",
    }
}

pub fn plan_application_virtual_key_secret_reference(
    request: ApplicationVirtualKeySecretReferenceRequest,
) -> Result<ApplicationVirtualKeySecretReferencePlan, ApplicationVirtualKeySecretReferenceError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationVirtualKeySecretReferenceStoragePlan::Postgres(
            plan_postgres_virtual_key_secret_reference(request.reference)
                .map_err(ApplicationVirtualKeySecretReferenceError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationVirtualKeySecretReferenceStoragePlan::Sqlite(
            plan_sqlite_virtual_key_secret_reference(request.reference)
                .map_err(ApplicationVirtualKeySecretReferenceError::Sqlite)?,
        ),
    };
    Ok(ApplicationVirtualKeySecretReferencePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeyLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub reference: VirtualKeySecretReferenceCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationVirtualKeyLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeyLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("reference", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeyLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub reference_storage: Option<ApplicationVirtualKeySecretReferenceStoragePlan>,
}

impl fmt::Debug for ApplicationVirtualKeyLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeyLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "reference_storage",
                &self.reference_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationVirtualKeyLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        reference_tenant: TenantId,
    },
    ReferenceKindMismatch {
        operation: ControlPlaneOperation,
        reference_kind: VirtualKeySecretReferenceKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    VirtualKeySecretReference(ApplicationVirtualKeySecretReferenceError),
}

impl fmt::Debug for ApplicationVirtualKeyLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("reference_tenant", &"<redacted>")
                .finish(),
            Self::ReferenceKindMismatch { .. } => f
                .debug_struct("ReferenceKindMismatch")
                .field("operation", &"<redacted>")
                .field("reference_kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::VirtualKeySecretReference(_) => f
                .debug_tuple("VirtualKeySecretReference")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationVirtualKeyLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not a virtual-key lifecycle operation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not virtual key")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "virtual-key lifecycle request is invalid")
            }
            Self::ReferenceKindMismatch { .. } => {
                write!(
                    f,
                    "virtual-key lifecycle operation does not match reference kind"
                )
            }
            Self::Audit(error) => error.fmt(f),
            Self::VirtualKeySecretReference(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationVirtualKeyLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationVirtualKeyLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationVirtualKeyLifecycleErrorResponsePlan {
    pub status: ApplicationVirtualKeyLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_virtual_key_lifecycle_error_response(
    error: &ApplicationVirtualKeyLifecycleError,
) -> ApplicationVirtualKeyLifecycleErrorResponsePlan {
    match error {
        ApplicationVirtualKeyLifecycleError::WrongOperation(_)
        | ApplicationVirtualKeyLifecycleError::WrongResourceKind(_)
        | ApplicationVirtualKeyLifecycleError::TenantMismatch { .. }
        | ApplicationVirtualKeyLifecycleError::ReferenceKindMismatch { .. } => {
            ApplicationVirtualKeyLifecycleErrorResponsePlan {
                status: ApplicationVirtualKeyLifecycleErrorStatus::BadRequest,
                code: "virtual_key_lifecycle_invalid",
                message: "virtual-key lifecycle request is invalid",
            }
        }
        ApplicationVirtualKeyLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationVirtualKeyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationVirtualKeyLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationVirtualKeyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "virtual_key_lifecycle_audit_unavailable",
                message: "virtual-key lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationVirtualKeyLifecycleError::VirtualKeySecretReference(error) => {
            let response = plan_application_virtual_key_secret_reference_error_response(error);
            ApplicationVirtualKeyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationVirtualKeySecretReferenceErrorStatus::ServiceUnavailable => {
                        ApplicationVirtualKeyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "virtual_key_lifecycle_storage_unavailable",
                message: "virtual-key lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_virtual_key_lifecycle(
    request: ApplicationVirtualKeyLifecycleRequest,
) -> Result<ApplicationVirtualKeyLifecyclePlan, ApplicationVirtualKeyLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::VirtualKeyCreate | ControlPlaneOperation::VirtualKeyRotateSecret
    ) {
        return Err(ApplicationVirtualKeyLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::VirtualKey {
        return Err(ApplicationVirtualKeyLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.reference.tenant_id {
        return Err(ApplicationVirtualKeyLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            reference_tenant: request.reference.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::VirtualKeyCreate => VirtualKeySecretReferenceKind::Create,
        ControlPlaneOperation::VirtualKeyRotateSecret => VirtualKeySecretReferenceKind::Rotate,
        _ => unreachable!("virtual-key lifecycle operation was already validated"),
    };
    if request.reference.kind != expected_kind {
        return Err(ApplicationVirtualKeyLifecycleError::ReferenceKindMismatch {
            operation: request.action.operation,
            reference_kind: request.reference.kind,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationVirtualKeyLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationVirtualKeyLifecycleError::Audit)?,
        ),
    };
    let reference_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let reference_plan = plan_application_virtual_key_secret_reference(
            ApplicationVirtualKeySecretReferenceRequest {
                durable_store: request.durable_store,
                reference: request.reference,
            },
        )
        .map_err(ApplicationVirtualKeyLifecycleError::VirtualKeySecretReference)?;
        Some(reference_plan.storage)
    } else {
        None
    };

    Ok(ApplicationVirtualKeyLifecyclePlan {
        decision,
        audit_storage,
        reference_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialReferenceRequest {
    pub durable_store: DurableStoreKind,
    pub reference: ProviderCredentialReferenceCommand,
}

impl fmt::Debug for ApplicationProviderCredentialReferenceRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialReferenceRequest")
            .field("durable_store", &self.durable_store)
            .field("reference", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationProviderCredentialReferenceStoragePlan {
    Postgres(PostgresProviderCredentialReferenceSqlPlan),
    Sqlite(SqliteProviderCredentialReferenceSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialReferencePlan {
    pub storage: ApplicationProviderCredentialReferenceStoragePlan,
}

impl fmt::Debug for ApplicationProviderCredentialReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialReferencePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationProviderCredentialReferenceError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationProviderCredentialReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationProviderCredentialReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationProviderCredentialReferenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationProviderCredentialReferenceErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCredentialReferenceErrorResponsePlan {
    pub status: ApplicationProviderCredentialReferenceErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_provider_credential_reference_error_response(
    error: &ApplicationProviderCredentialReferenceError,
) -> ApplicationProviderCredentialReferenceErrorResponsePlan {
    match error {
        ApplicationProviderCredentialReferenceError::Postgres(error) => {
            application_provider_credential_reference_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationProviderCredentialReferenceError::Sqlite(error) => {
            application_provider_credential_reference_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_provider_credential_reference_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationProviderCredentialReferenceErrorResponsePlan {
    ApplicationProviderCredentialReferenceErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationProviderCredentialReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "provider_credential_storage_unavailable",
        message: "provider credential storage is temporarily unavailable",
    }
}

fn application_provider_credential_reference_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationProviderCredentialReferenceErrorResponsePlan {
    ApplicationProviderCredentialReferenceErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationProviderCredentialReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "provider_credential_storage_unavailable",
        message: "provider credential storage is temporarily unavailable",
    }
}

pub fn plan_application_provider_credential_reference(
    request: ApplicationProviderCredentialReferenceRequest,
) -> Result<ApplicationProviderCredentialReferencePlan, ApplicationProviderCredentialReferenceError>
{
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationProviderCredentialReferenceStoragePlan::Postgres(
            plan_postgres_provider_credential_reference(request.reference)
                .map_err(ApplicationProviderCredentialReferenceError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationProviderCredentialReferenceStoragePlan::Sqlite(
            plan_sqlite_provider_credential_reference(request.reference)
                .map_err(ApplicationProviderCredentialReferenceError::Sqlite)?,
        ),
    };
    Ok(ApplicationProviderCredentialReferencePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialRotationRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub reference: ProviderCredentialReferenceCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationProviderCredentialRotationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialRotationRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("reference", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialRotationPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub reference_storage: Option<ApplicationProviderCredentialReferenceStoragePlan>,
}

impl fmt::Debug for ApplicationProviderCredentialRotationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialRotationPlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "reference_storage",
                &self.reference_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationProviderCredentialRotationError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        reference_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    ProviderCredentialReference(ApplicationProviderCredentialReferenceError),
}

impl fmt::Debug for ApplicationProviderCredentialRotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("reference_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::ProviderCredentialReference(_) => f
                .debug_tuple("ProviderCredentialReference")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationProviderCredentialRotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not provider credential rotation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not provider credential")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "provider credential rotation request is invalid")
            }
            Self::Audit(error) => error.fmt(f),
            Self::ProviderCredentialReference(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationProviderCredentialRotationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationProviderCredentialRotationErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCredentialRotationErrorResponsePlan {
    pub status: ApplicationProviderCredentialRotationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_provider_credential_rotation_error_response(
    error: &ApplicationProviderCredentialRotationError,
) -> ApplicationProviderCredentialRotationErrorResponsePlan {
    match error {
        ApplicationProviderCredentialRotationError::WrongOperation(_)
        | ApplicationProviderCredentialRotationError::WrongResourceKind(_)
        | ApplicationProviderCredentialRotationError::TenantMismatch { .. } => {
            ApplicationProviderCredentialRotationErrorResponsePlan {
                status: ApplicationProviderCredentialRotationErrorStatus::BadRequest,
                code: "provider_credential_rotation_invalid",
                message: "provider credential rotation request is invalid",
            }
        }
        ApplicationProviderCredentialRotationError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationProviderCredentialRotationErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationProviderCredentialRotationErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationProviderCredentialRotationErrorStatus::ServiceUnavailable
                    }
                },
                code: "provider_credential_rotation_audit_unavailable",
                message: "provider credential rotation audit is temporarily unavailable",
            }
        }
        ApplicationProviderCredentialRotationError::ProviderCredentialReference(error) => {
            let response = plan_application_provider_credential_reference_error_response(error);
            ApplicationProviderCredentialRotationErrorResponsePlan {
                status: match response.status {
                    ApplicationProviderCredentialReferenceErrorStatus::ServiceUnavailable => {
                        ApplicationProviderCredentialRotationErrorStatus::ServiceUnavailable
                    }
                },
                code: "provider_credential_rotation_storage_unavailable",
                message: "provider credential rotation storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_provider_credential_rotation(
    request: ApplicationProviderCredentialRotationRequest,
) -> Result<ApplicationProviderCredentialRotationPlan, ApplicationProviderCredentialRotationError> {
    if request.action.operation != ControlPlaneOperation::ProviderCredentialRotate {
        return Err(ApplicationProviderCredentialRotationError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::ProviderCredential {
        return Err(
            ApplicationProviderCredentialRotationError::WrongResourceKind(
                request.action.resource.kind,
            ),
        );
    }
    if request.action.resource.tenant_id != request.reference.tenant_id {
        return Err(ApplicationProviderCredentialRotationError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            reference_tenant: request.reference.tenant_id,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationProviderCredentialRotationError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationProviderCredentialRotationError::Audit)?,
        ),
    };
    let reference_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let reference_plan = plan_application_provider_credential_reference(
            ApplicationProviderCredentialReferenceRequest {
                durable_store: request.durable_store,
                reference: request.reference,
            },
        )
        .map_err(ApplicationProviderCredentialRotationError::ProviderCredentialReference)?;
        Some(reference_plan.storage)
    } else {
        None
    };

    Ok(ApplicationProviderCredentialRotationPlan {
        decision,
        audit_storage,
        reference_storage,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationPublicationRequest<T> {
    pub durable_store: DurableStoreKind,
    pub publication: ConfigurationPublicationRequest<T>,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationPublicationAuditStoragePlan {
    Postgres(PostgresAppendOnlyAuditSqlPlan),
    Sqlite(SqliteAppendOnlyAuditSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationPublicationPlan<T> {
    pub decision: ConfigurationPublicationDecision<T>,
    pub audit_storage: ApplicationConfigurationPublicationAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationActivationRequest<'a, T> {
    pub cache_state: &'a ConfigCacheState,
    pub publication_decision: &'a ConfigurationPublicationDecision<T>,
    pub redis_cache_ttl_seconds: Option<u64>,
    pub event_delivery: ConfigPublicationEventDelivery,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationActivationPlan {
    pub activation: Option<ConfigActivationPlan>,
    pub redis_cache: Option<RedisCachePlan>,
    pub publication_event: Option<ConfigPublicationEventPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationReadinessRequest {
    pub live: bool,
    pub startup_complete: bool,
    pub draining: bool,
    pub cache_state: ConfigCacheState,
    pub now_unix_ms: u64,
    pub checks: Vec<HealthCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationReadinessPlan {
    pub refresh_decision: ConfigRefreshDecision,
    pub snapshot: HealthSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationActivationError {
    Publication(ConfigPublicationError),
    PublicationEvent(ConfigPublicationEventError),
    Redis(prodex_storage_redis::RedisPlanError),
}

impl fmt::Display for ApplicationConfigurationActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publication(err) => err.fmt(f),
            Self::PublicationEvent(err) => err.fmt(f),
            Self::Redis(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationConfigurationActivationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationActivationErrorStatus {
    BadRequest,
    Conflict,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationActivationErrorResponsePlan {
    pub status: ApplicationConfigurationActivationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_configuration_activation_error_response(
    error: &ApplicationConfigurationActivationError,
) -> ApplicationConfigurationActivationErrorResponsePlan {
    match error {
        ApplicationConfigurationActivationError::Publication(error) => {
            application_configuration_activation_response_from_config(
                plan_config_activation_error_response(error),
            )
        }
        ApplicationConfigurationActivationError::PublicationEvent(error) => {
            let response = plan_config_publication_event_error_response(error);
            ApplicationConfigurationActivationErrorResponsePlan {
                status: match response.status {
                    prodex_config::ConfigPublicationEventErrorStatus::InvalidConfiguration => {
                        ApplicationConfigurationActivationErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationConfigurationActivationError::Redis(error) => {
            application_configuration_activation_response_from_redis(plan_redis_error_response(
                error,
            ))
        }
    }
}

pub fn plan_application_configuration_readiness_snapshot(
    request: ApplicationConfigurationReadinessRequest,
) -> ApplicationConfigurationReadinessPlan {
    let refresh_decision = evaluate_config_refresh(&request.cache_state, request.now_unix_ms);
    let active_policy_revision = match refresh_decision {
        ConfigRefreshDecision::UseActive | ConfigRefreshDecision::RefreshAsync => {
            request.cache_state.active_revision_id
        }
        ConfigRefreshDecision::UseLastKnownGood => request.cache_state.last_known_good_revision_id,
        ConfigRefreshDecision::RefreshRequired | ConfigRefreshDecision::RejectedInvalidated => None,
    };
    let mut checks = request.checks;
    checks.push(application_configuration_readiness_check(refresh_decision));
    let snapshot = HealthSnapshot::new(
        request.live,
        request.startup_complete,
        request.draining,
        active_policy_revision,
        checks,
    );
    ApplicationConfigurationReadinessPlan {
        refresh_decision,
        snapshot,
    }
}

fn application_configuration_readiness_check(
    refresh_decision: ConfigRefreshDecision,
) -> HealthCheck {
    match refresh_decision {
        ConfigRefreshDecision::UseActive => {
            HealthCheck::new("configuration", HealthState::Passing, None::<String>)
        }
        ConfigRefreshDecision::RefreshAsync => HealthCheck::new(
            "configuration",
            HealthState::Degraded,
            Some("configuration refresh is pending"),
        ),
        ConfigRefreshDecision::UseLastKnownGood => HealthCheck::new(
            "configuration",
            HealthState::Degraded,
            Some("using last known good configuration"),
        ),
        ConfigRefreshDecision::RefreshRequired => HealthCheck::new(
            "configuration",
            HealthState::Failing,
            Some("configuration refresh is required"),
        ),
        ConfigRefreshDecision::RejectedInvalidated => HealthCheck::new(
            "configuration",
            HealthState::Failing,
            Some("configuration revision is invalidated"),
        ),
    }
}

fn application_configuration_activation_response_from_config(
    response: ConfigPublicationErrorResponsePlan,
) -> ApplicationConfigurationActivationErrorResponsePlan {
    ApplicationConfigurationActivationErrorResponsePlan {
        status: match response.status {
            ConfigPublicationErrorStatus::BadRequest => {
                ApplicationConfigurationActivationErrorStatus::BadRequest
            }
            ConfigPublicationErrorStatus::Conflict => {
                ApplicationConfigurationActivationErrorStatus::Conflict
            }
        },
        code: response.code,
        message: response.message,
    }
}

fn application_configuration_activation_response_from_redis(
    response: RedisPlanErrorResponsePlan,
) -> ApplicationConfigurationActivationErrorResponsePlan {
    ApplicationConfigurationActivationErrorResponsePlan {
        status: match response.status {
            RedisPlanErrorStatus::ServiceUnavailable => {
                ApplicationConfigurationActivationErrorStatus::ServiceUnavailable
            }
        },
        code: "configuration_cache_unavailable",
        message: "configuration cache is temporarily unavailable",
    }
}

pub fn plan_application_configuration_activation<T>(
    request: ApplicationConfigurationActivationRequest<'_, T>,
) -> Result<ApplicationConfigurationActivationPlan, ApplicationConfigurationActivationError> {
    match request.publication_decision {
        ConfigurationPublicationDecision::Authorized(publication) => {
            let activation = plan_config_activation(request.cache_state, &publication.candidate)
                .map_err(ApplicationConfigurationActivationError::Publication)?;
            let redis_cache = request
                .redis_cache_ttl_seconds
                .map(|ttl_seconds| {
                    plan_policy_revision_cache(
                        activation.next_state.tenant_id,
                        activation.activated_revision_id,
                        ttl_seconds,
                    )
                    .map_err(ApplicationConfigurationActivationError::Redis)
                })
                .transpose()?;
            let publication_event =
                plan_config_publication_event(&activation, request.event_delivery)
                    .map_err(ApplicationConfigurationActivationError::PublicationEvent)?;
            Ok(ApplicationConfigurationActivationPlan {
                activation: Some(activation),
                redis_cache,
                publication_event: Some(publication_event),
            })
        }
        ConfigurationPublicationDecision::Denied { .. } => {
            Ok(ApplicationConfigurationActivationPlan {
                activation: None,
                redis_cache: None,
                publication_event: None,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationPublicationError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationConfigurationPublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationConfigurationPublicationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationPublicationErrorStatus {
    BadRequest,
    Conflict,
    Forbidden,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationPublicationErrorResponsePlan {
    pub status: ApplicationConfigurationPublicationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_configuration_publication_decision_error_response<T>(
    decision: &ConfigurationPublicationDecision<T>,
) -> Option<ApplicationConfigurationPublicationErrorResponsePlan> {
    match decision {
        ConfigurationPublicationDecision::Authorized(_) => None,
        ConfigurationPublicationDecision::Denied { error, .. } => Some(
            application_configuration_publication_response_from_control_plane(
                plan_configuration_publication_error_response(error),
            ),
        ),
    }
}

pub fn plan_application_configuration_publication_error_response(
    error: &ApplicationConfigurationPublicationError,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    match error {
        ApplicationConfigurationPublicationError::Postgres(error) => {
            application_configuration_publication_response_from_postgres(
                plan_postgres_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
        ApplicationConfigurationPublicationError::Sqlite(error) => {
            application_configuration_publication_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
    }
}

fn application_configuration_publication_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    ApplicationConfigurationPublicationErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationConfigurationPublicationErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_configuration_publication_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    ApplicationConfigurationPublicationErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationConfigurationPublicationErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_configuration_publication_response_from_control_plane(
    response: ConfigurationPublicationErrorResponsePlan,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    ApplicationConfigurationPublicationErrorResponsePlan {
        status: match response.status {
            ConfigurationPublicationErrorStatus::BadRequest => {
                ApplicationConfigurationPublicationErrorStatus::BadRequest
            }
            ConfigurationPublicationErrorStatus::Conflict => {
                ApplicationConfigurationPublicationErrorStatus::Conflict
            }
            ConfigurationPublicationErrorStatus::Forbidden => {
                ApplicationConfigurationPublicationErrorStatus::Forbidden
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_application_configuration_publication<T>(
    request: ApplicationConfigurationPublicationRequest<T>,
) -> Result<ApplicationConfigurationPublicationPlan<T>, ApplicationConfigurationPublicationError> {
    let decision = decide_configuration_publication(request.publication);
    let audit_write = configuration_publication_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => {
            ApplicationConfigurationPublicationAuditStoragePlan::Postgres(
                plan_postgres_append_only_audit(audit_command)
                    .map_err(ApplicationConfigurationPublicationError::Postgres)?,
            )
        }
        DurableStoreKind::Sqlite => ApplicationConfigurationPublicationAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationConfigurationPublicationError::Sqlite)?,
        ),
    };
    Ok(ApplicationConfigurationPublicationPlan {
        decision,
        audit_storage,
    })
}

fn configuration_publication_audit_write<T>(
    decision: &ConfigurationPublicationDecision<T>,
) -> &ControlPlaneAuditWritePlan {
    match decision {
        ConfigurationPublicationDecision::Authorized(plan) => &plan.action.audit_write,
        ConfigurationPublicationDecision::Denied { audit_write, .. } => audit_write,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimeTopology {
    pub storage: StorageTopology,
    pub migrations: ApplicationMigrationMode,
    pub gateway_replica_count: u16,
    pub require_multi_replica_accounting_checks: bool,
    pub redis_rate_limit: Option<RedisRateLimitPlan>,
    pub redis_cache: Vec<RedisCachePlan>,
    pub redis_coordination: Vec<RedisCoordinationPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationMigrationMode {
    ExternalOnly,
    RequestPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimePlan {
    pub durable_store: DurableStoreKind,
    pub migrations_are_external: bool,
    pub redis_is_rebuildable: bool,
    pub accounting_concurrency: Option<MultiReplicaAccountingConcurrencySpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRuntimePlanError {
    MigrationOnRequestPath,
    Storage(prodex_storage::StorageTopologyError),
    AccountingConcurrency(prodex_storage::MultiReplicaAccountingConcurrencySpecError),
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationRuntimePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MigrationOnRequestPath => write!(
                f,
                "application topology cannot run migrations on request paths"
            ),
            Self::Storage(err) => err.fmt(f),
            Self::AccountingConcurrency(err) => err.fmt(f),
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationRuntimePlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRuntimePlanErrorStatus {
    InvalidConfiguration,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimePlanErrorResponsePlan {
    pub status: ApplicationRuntimePlanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_runtime_error_response(
    error: &ApplicationRuntimePlanError,
) -> ApplicationRuntimePlanErrorResponsePlan {
    match error {
        ApplicationRuntimePlanError::MigrationOnRequestPath => {
            ApplicationRuntimePlanErrorResponsePlan {
                status: ApplicationRuntimePlanErrorStatus::InvalidConfiguration,
                code: "runtime_topology_invalid",
                message: "runtime topology configuration is invalid",
            }
        }
        ApplicationRuntimePlanError::Storage(error) => application_runtime_response_from_storage(
            plan_storage_topology_error_response(error),
            "runtime_topology_invalid",
            "runtime topology configuration is invalid",
        ),
        ApplicationRuntimePlanError::AccountingConcurrency(error) => {
            application_runtime_response_from_storage(
                plan_multi_replica_accounting_error_response(error),
                "runtime_topology_invalid",
                "runtime topology configuration is invalid",
            )
        }
        ApplicationRuntimePlanError::Postgres(error) => application_runtime_response_from_postgres(
            plan_postgres_storage_error_response(error),
            "runtime_storage_plan_unavailable",
            "runtime storage planning is temporarily unavailable",
        ),
        ApplicationRuntimePlanError::Sqlite(error) => application_runtime_response_from_sqlite(
            plan_sqlite_storage_error_response(error),
            "runtime_storage_plan_unavailable",
            "runtime storage planning is temporarily unavailable",
        ),
    }
}

fn application_runtime_response_from_storage(
    response: StoragePlanErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: match response.status {
            StoragePlanErrorStatus::BadRequest | StoragePlanErrorStatus::InvalidConfiguration => {
                ApplicationRuntimePlanErrorStatus::InvalidConfiguration
            }
        },
        code,
        message,
    }
}

fn application_runtime_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationRuntimePlanErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_runtime_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationRuntimePlanErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_runtime(
    topology: ApplicationRuntimeTopology,
) -> Result<ApplicationRuntimePlan, ApplicationRuntimePlanError> {
    topology
        .storage
        .validate_for_gateway()
        .map_err(ApplicationRuntimePlanError::Storage)?;
    if topology.migrations != ApplicationMigrationMode::ExternalOnly {
        return Err(ApplicationRuntimePlanError::MigrationOnRequestPath);
    }
    match topology.storage.durable_store {
        DurableStoreKind::Postgres => {
            plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator)
                .map_err(ApplicationRuntimePlanError::Postgres)?;
        }
        DurableStoreKind::Sqlite => {
            plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator)
                .map_err(ApplicationRuntimePlanError::Sqlite)?;
        }
    }
    let accounting_concurrency = if topology.require_multi_replica_accounting_checks {
        Some(
            plan_multi_replica_accounting_concurrency_spec(
                topology.storage,
                topology.gateway_replica_count,
            )
            .map_err(ApplicationRuntimePlanError::AccountingConcurrency)?,
        )
    } else {
        None
    };
    Ok(ApplicationRuntimePlan {
        durable_store: topology.storage.durable_store,
        migrations_are_external: true,
        redis_is_rebuildable: topology.redis_rate_limit.is_some()
            || !topology.redis_cache.is_empty()
            || !topology.redis_coordination.is_empty(),
        accounting_concurrency,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimeAccountingVerificationRequest<'a> {
    pub runtime_plan: &'a ApplicationRuntimePlan,
    pub evidence: MultiReplicaAccountingEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimeAccountingVerificationPlan {
    pub verification: MultiReplicaAccountingVerificationPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRuntimeAccountingVerificationError {
    AccountingNotRequired,
    AccountingConcurrency(prodex_storage::MultiReplicaAccountingConcurrencySpecError),
}

impl fmt::Display for ApplicationRuntimeAccountingVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountingNotRequired => write!(
                f,
                "runtime topology did not require multi-replica accounting verification"
            ),
            Self::AccountingConcurrency(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationRuntimeAccountingVerificationError {}

pub fn plan_application_runtime_accounting_verification_error_response(
    error: &ApplicationRuntimeAccountingVerificationError,
) -> ApplicationRuntimePlanErrorResponsePlan {
    match error {
        ApplicationRuntimeAccountingVerificationError::AccountingNotRequired => {
            ApplicationRuntimePlanErrorResponsePlan {
                status: ApplicationRuntimePlanErrorStatus::InvalidConfiguration,
                code: "runtime_accounting_verification_invalid",
                message: "runtime accounting verification is invalid",
            }
        }
        ApplicationRuntimeAccountingVerificationError::AccountingConcurrency(error) => {
            application_runtime_response_from_storage(
                plan_multi_replica_accounting_error_response(error),
                "runtime_accounting_verification_invalid",
                "runtime accounting verification is invalid",
            )
        }
    }
}

pub fn plan_application_runtime_accounting_verification_required_response()
-> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: ApplicationRuntimePlanErrorStatus::InvalidConfiguration,
        code: "runtime_accounting_verification_invalid",
        message: "runtime accounting verification is invalid",
    }
}

pub fn plan_application_runtime_accounting_verification(
    request: ApplicationRuntimeAccountingVerificationRequest<'_>,
) -> Result<
    ApplicationRuntimeAccountingVerificationPlan,
    ApplicationRuntimeAccountingVerificationError,
> {
    let spec = request
        .runtime_plan
        .accounting_concurrency
        .as_ref()
        .ok_or(ApplicationRuntimeAccountingVerificationError::AccountingNotRequired)?;
    let verification = plan_multi_replica_accounting_verification(spec, request.evidence)
        .map_err(ApplicationRuntimeAccountingVerificationError::AccountingConcurrency)?;

    Ok(ApplicationRuntimeAccountingVerificationPlan { verification })
}

pub fn assert_app_is_composition_root_only_marker() -> bool {
    true
}
