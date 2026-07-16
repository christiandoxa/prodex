//! Admission accounting, reconciliation, recovery, and billing plans.

mod reconciliation_execution;

pub use reconciliation_execution::*;

use super::*;
use crate::config::application_runtime_response_from_storage;
use crate::control_plane::control_plane_audit_write;

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
    let status = match error {
        ApplicationAtomicReservationError::Postgres(error) => {
            match plan_postgres_storage_error_response(error).status {
                PostgresStorageErrorStatus::ServiceUnavailable => {
                    ApplicationAtomicReservationErrorStatus::ServiceUnavailable
                }
            }
        }
        ApplicationAtomicReservationError::Sqlite(error) => {
            match plan_sqlite_storage_error_response(error).status {
                SqliteStorageErrorStatus::ServiceUnavailable => {
                    ApplicationAtomicReservationErrorStatus::ServiceUnavailable
                }
            }
        }
    };
    ApplicationAtomicReservationErrorResponsePlan {
        status,
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
