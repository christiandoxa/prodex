use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditAppendMode {
    HashChain,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AppendOnlyAuditCommand {
    pub storage_key: TenantStorageKey,
    pub event: AuditEvent,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for AppendOnlyAuditCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppendOnlyAuditCommand")
            .field("storage_key", &"<redacted>")
            .field("event", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AppendOnlyAuditPlan {
    pub storage_key: TenantStorageKey,
    pub mode: AuditAppendMode,
    pub envelope: AuditEnvelope,
}

impl fmt::Debug for AppendOnlyAuditPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppendOnlyAuditPlan")
            .field("storage_key", &"<redacted>")
            .field("mode", &self.mode)
            .field("envelope", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AppendOnlyAuditPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        event_tenant: TenantId,
    },
}

impl fmt::Debug for AppendOnlyAuditPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("event_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AppendOnlyAuditPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "append-only audit request is invalid"),
        }
    }
}

impl Error for AppendOnlyAuditPlanError {}

pub fn plan_append_only_audit_error_response(
    _error: &AppendOnlyAuditPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "append_only_audit_rejected",
        message: "append-only audit request is invalid",
    }
}

pub fn plan_append_only_audit(
    command: AppendOnlyAuditCommand,
) -> Result<AppendOnlyAuditPlan, AppendOnlyAuditPlanError> {
    if command.storage_key.tenant_id != command.event.tenant_id {
        return Err(AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            event_tenant: command.event.tenant_id,
        });
    }
    Ok(AppendOnlyAuditPlan {
        storage_key: command.storage_key,
        mode: AuditAppendMode::HashChain,
        envelope: AuditEnvelope::new(command.event, command.previous_digest, command.event_digest),
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditRetentionPurgeCommand {
    pub storage_key: TenantStorageKey,
    pub batch: AuditRetentionPurgeBatch,
}

impl fmt::Debug for AuditRetentionPurgeCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPurgeCommand")
            .field("storage_key", &"<redacted>")
            .field("batch", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditRetentionPurgePlan {
    pub storage_key: TenantStorageKey,
    pub batch: AuditRetentionPurgeBatch,
}

impl fmt::Debug for AuditRetentionPurgePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPurgePlan")
            .field("storage_key", &"<redacted>")
            .field("batch", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionPurgePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        batch_tenant: TenantId,
    },
}

impl fmt::Debug for AuditRetentionPurgePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("batch_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditRetentionPurgePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "audit retention purge request is invalid"),
        }
    }
}

impl Error for AuditRetentionPurgePlanError {}

pub fn plan_audit_retention_purge_error_response(
    _error: &AuditRetentionPurgePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "audit_retention_purge_rejected",
        message: "audit retention purge request is invalid",
    }
}

pub fn plan_audit_retention_purge(
    command: AuditRetentionPurgeCommand,
) -> Result<AuditRetentionPurgePlan, AuditRetentionPurgePlanError> {
    if command.storage_key.tenant_id != command.batch.tenant_id {
        return Err(AuditRetentionPurgePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            batch_tenant: command.batch.tenant_id,
        });
    }
    Ok(AuditRetentionPurgePlan {
        storage_key: command.storage_key,
        batch: command.batch,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditExportQueryCommand {
    pub storage_key: TenantStorageKey,
    pub export: AuditExportPlan,
}

impl fmt::Debug for AuditExportQueryCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditExportQueryCommand")
            .field("storage_key", &"<redacted>")
            .field("export", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditExportQueryPlan {
    pub storage_key: TenantStorageKey,
    pub export: AuditExportPlan,
    pub tenant_id: TenantId,
    pub sort_order: AuditSortOrder,
    pub page_limit: u16,
}

impl fmt::Debug for AuditExportQueryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditExportQueryPlan")
            .field("storage_key", &"<redacted>")
            .field("export", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("sort_order", &self.sort_order)
            .field("page_limit", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditExportQueryPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        query_tenant: TenantId,
    },
}

impl fmt::Debug for AuditExportQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("query_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditExportQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "audit export query request is invalid"),
        }
    }
}

impl Error for AuditExportQueryPlanError {}

pub fn plan_audit_export_query_error_response(
    _error: &AuditExportQueryPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "audit_export_query_rejected",
        message: "audit export query request is invalid",
    }
}

pub fn plan_audit_export_query(
    command: AuditExportQueryCommand,
) -> Result<AuditExportQueryPlan, AuditExportQueryPlanError> {
    let query_tenant = command.export.query.scope.tenant_id;
    if command.storage_key.tenant_id != query_tenant {
        return Err(AuditExportQueryPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            query_tenant,
        });
    }
    Ok(AuditExportQueryPlan {
        storage_key: command.storage_key,
        tenant_id: query_tenant,
        sort_order: command.export.query.sort_order,
        page_limit: command.export.query.page_limit.get(),
        export: command.export,
    })
}
