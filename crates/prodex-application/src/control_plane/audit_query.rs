//! Audit retention and export planning.

use super::super::*;
use super::*;

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
