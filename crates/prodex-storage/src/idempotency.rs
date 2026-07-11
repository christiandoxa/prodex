use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyPendingRecordCommand {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
    pub started_at_unix_ms: u64,
}

impl fmt::Debug for IdempotencyPendingRecordCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyPendingRecordCommand")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .field("started_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyPendingRecordPlan {
    pub storage_key: TenantStorageKey,
    pub entry: IdempotencyEntry<()>,
}

impl fmt::Debug for IdempotencyPendingRecordPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyPendingRecordPlan")
            .field("storage_key", &"<redacted>")
            .field("entry", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyPendingRecordPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        operation_tenant: TenantId,
    },
}

impl fmt::Debug for IdempotencyPendingRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("operation_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotencyPendingRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "idempotency pending record request is invalid")
            }
        }
    }
}

impl Error for IdempotencyPendingRecordPlanError {}

pub fn plan_idempotency_pending_record_error_response(
    _error: &IdempotencyPendingRecordPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_pending_record_rejected",
        message: "idempotency pending record request is invalid",
    }
}

pub fn plan_idempotency_pending_record(
    command: IdempotencyPendingRecordCommand,
) -> Result<IdempotencyPendingRecordPlan, IdempotencyPendingRecordPlanError> {
    if command.storage_key.tenant_id != command.operation.tenant_id {
        return Err(IdempotencyPendingRecordPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            operation_tenant: command.operation.tenant_id,
        });
    }
    Ok(IdempotencyPendingRecordPlan {
        storage_key: command.storage_key,
        entry: IdempotencyEntry::pending(command.operation, command.started_at_unix_ms),
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyCompletedRecordCommand {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
    pub completed_at_unix_ms: u64,
    pub response_body: Vec<u8>,
}

impl fmt::Debug for IdempotencyCompletedRecordCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyCompletedRecordCommand")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .field("completed_at_unix_ms", &"<redacted>")
            .field("response_body", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyCompletedRecordPlan {
    pub storage_key: TenantStorageKey,
    pub completed_at_unix_ms: u64,
    pub entry: IdempotencyEntry<Vec<u8>>,
}

impl fmt::Debug for IdempotencyCompletedRecordPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyCompletedRecordPlan")
            .field("storage_key", &"<redacted>")
            .field("completed_at_unix_ms", &"<redacted>")
            .field("entry", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyCompletedRecordPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        operation_tenant: TenantId,
    },
}

impl fmt::Debug for IdempotencyCompletedRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("operation_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotencyCompletedRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "idempotency completed record request is invalid")
            }
        }
    }
}

impl Error for IdempotencyCompletedRecordPlanError {}

pub fn plan_idempotency_completed_record_error_response(
    _error: &IdempotencyCompletedRecordPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_completed_record_rejected",
        message: "idempotency completed record request is invalid",
    }
}

pub fn plan_idempotency_completed_record(
    command: IdempotencyCompletedRecordCommand,
) -> Result<IdempotencyCompletedRecordPlan, IdempotencyCompletedRecordPlanError> {
    if command.storage_key.tenant_id != command.operation.tenant_id {
        return Err(IdempotencyCompletedRecordPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            operation_tenant: command.operation.tenant_id,
        });
    }
    Ok(IdempotencyCompletedRecordPlan {
        storage_key: command.storage_key,
        completed_at_unix_ms: command.completed_at_unix_ms,
        entry: IdempotencyEntry::completed(IdempotencyRecord {
            operation: command.operation,
            response: command.response_body,
        }),
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyRecordLookupCommand {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
}

impl fmt::Debug for IdempotencyRecordLookupCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyRecordLookupCommand")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyRecordLookupPlan {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
}

impl fmt::Debug for IdempotencyRecordLookupPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyRecordLookupPlan")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyRecordLookupPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        operation_tenant: TenantId,
    },
}

impl fmt::Debug for IdempotencyRecordLookupPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("operation_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotencyRecordLookupPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "idempotency record lookup request is invalid")
            }
        }
    }
}

impl Error for IdempotencyRecordLookupPlanError {}

pub fn plan_idempotency_record_lookup_error_response(
    _error: &IdempotencyRecordLookupPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_record_lookup_rejected",
        message: "idempotency record lookup request is invalid",
    }
}

pub fn plan_idempotency_record_lookup(
    command: IdempotencyRecordLookupCommand,
) -> Result<IdempotencyRecordLookupPlan, IdempotencyRecordLookupPlanError> {
    if command.storage_key.tenant_id != command.operation.tenant_id {
        return Err(IdempotencyRecordLookupPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            operation_tenant: command.operation.tenant_id,
        });
    }
    Ok(IdempotencyRecordLookupPlan {
        storage_key: command.storage_key,
        operation: command.operation,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotencyRecordLookupRowStatus {
    Pending,
    Completed,
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyRecordLookupRow {
    pub tenant_id: TenantId,
    pub idempotency_key: IdempotencyKey,
    pub request_fingerprint: String,
    pub status: IdempotencyRecordLookupRowStatus,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: Option<u64>,
    pub response_body: Option<Vec<u8>>,
}

impl fmt::Debug for IdempotencyRecordLookupRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyRecordLookupRow")
            .field("tenant_id", &"<redacted>")
            .field("idempotency_key", &"<redacted>")
            .field("request_fingerprint", &"<redacted>")
            .field("status", &self.status)
            .field("started_at_unix_ms", &"<redacted>")
            .field("completed_at_unix_ms", &"<redacted>")
            .field("response_body", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyRecordLookupRowError {
    TenantMismatch {
        operation_tenant: TenantId,
        row_tenant: TenantId,
    },
    KeyMismatch,
    InvalidRequestFingerprint,
    CompletedResponseMissing,
    CompletedTimestampMissing,
}

impl fmt::Debug for IdempotencyRecordLookupRowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("operation_tenant", &"<redacted>")
                .field("row_tenant", &"<redacted>")
                .finish(),
            Self::KeyMismatch => f.write_str("KeyMismatch"),
            Self::InvalidRequestFingerprint => f.write_str("InvalidRequestFingerprint"),
            Self::CompletedResponseMissing => f.write_str("CompletedResponseMissing"),
            Self::CompletedTimestampMissing => f.write_str("CompletedTimestampMissing"),
        }
    }
}

impl fmt::Display for IdempotencyRecordLookupRowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. }
            | Self::KeyMismatch
            | Self::InvalidRequestFingerprint
            | Self::CompletedResponseMissing
            | Self::CompletedTimestampMissing => {
                write!(f, "idempotency record lookup row is invalid")
            }
        }
    }
}

impl Error for IdempotencyRecordLookupRowError {}

pub fn materialize_idempotency_record_lookup_row_error_response(
    _error: &IdempotencyRecordLookupRowError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_record_lookup_row_invalid",
        message: "idempotency record lookup row is invalid",
    }
}

pub fn materialize_idempotency_record_lookup_row(
    operation: &IdempotentOperation,
    row: IdempotencyRecordLookupRow,
) -> Result<IdempotencyEntry<Vec<u8>>, IdempotencyRecordLookupRowError> {
    if row.tenant_id != operation.tenant_id {
        return Err(IdempotencyRecordLookupRowError::TenantMismatch {
            operation_tenant: operation.tenant_id,
            row_tenant: row.tenant_id,
        });
    }
    if row.idempotency_key != operation.key {
        return Err(IdempotencyRecordLookupRowError::KeyMismatch);
    }
    let row_operation =
        IdempotentOperation::new(row.tenant_id, row.idempotency_key, row.request_fingerprint)
            .map_err(|_| IdempotencyRecordLookupRowError::InvalidRequestFingerprint)?;
    match row.status {
        IdempotencyRecordLookupRowStatus::Pending => Ok(IdempotencyEntry::pending(
            row_operation,
            row.started_at_unix_ms,
        )),
        IdempotencyRecordLookupRowStatus::Completed => {
            if row.completed_at_unix_ms.is_none() {
                return Err(IdempotencyRecordLookupRowError::CompletedTimestampMissing);
            }
            let Some(response_body) = row.response_body else {
                return Err(IdempotencyRecordLookupRowError::CompletedResponseMissing);
            };
            Ok(IdempotencyEntry::completed(IdempotencyRecord {
                operation: row_operation,
                response: response_body,
            }))
        }
    }
}
