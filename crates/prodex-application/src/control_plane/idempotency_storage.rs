//! Durable idempotency record planning.

use super::super::*;

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
