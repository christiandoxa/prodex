#![forbid(unsafe_code)]
//! SQLite storage plans for local compatibility and tests.
//!
//! SQLite remains useful for local/single-node compatibility, but migration DDL
//! must be planned explicitly and kept out of request-serving open paths. This
//! crate is driver-free and owns SQL text only; adapter crates decide when and
//! how to execute it.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    AuditSortOrder, BudgetLimit, IdempotencyKey, PrincipalId, ProviderCredentialId,
    ReservationRequest, Role, RoleBindingId, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AtomicReservationCommand, AuditExportQueryCommand,
    AuditRetentionPurgeCommand, BillingLedgerQueryCommand, BillingLedgerSortOrder,
    BudgetPolicyUpdateCommand, ExpiredReservationRecoveryCommand,
    IdempotencyCompletedRecordCommand, IdempotencyPendingRecordCommand,
    IdempotencyRecordLookupCommand, ProviderCredentialReferenceCommand, RoleBindingMutationCommand,
    RoleBindingMutationKind, ServiceIdentityCreateCommand, TenantLifecycleCommand,
    TenantLifecycleKind, TenantStorageKey, UsageReconciliationCommand, UserLifecycleCommand,
    UserLifecycleKind, VirtualKeySecretReferenceCommand, VirtualKeySecretReferenceKind,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SqliteMigrationVersion(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteMigrationPhase {
    Expand,
    Backfill,
    Contract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqliteMigration {
    pub version: SqliteMigrationVersion,
    pub phase: SqliteMigrationPhase,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const INITIAL_LOCAL_ACCOUNTING_MIGRATION: SqliteMigration = SqliteMigration {
    version: SqliteMigrationVersion(1),
    phase: SqliteMigrationPhase::Expand,
    name: "001_local_tenant_accounting",
    sql: r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

CREATE TABLE IF NOT EXISTS prodex_tenants (
    tenant_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL,
    CHECK (display_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_virtual_keys (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    virtual_key_id TEXT NOT NULL,
    principal_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    secret_provider TEXT NOT NULL,
    secret_name TEXT NOT NULL,
    secret_version TEXT,
    secret_rotated_at_unix_ms INTEGER NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    revoked_at_unix_ms INTEGER,
    PRIMARY KEY (tenant_id, virtual_key_id),
    CHECK (display_name <> ''),
    CHECK (secret_provider <> ''),
    CHECK (secret_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_service_identities (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    principal_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    disabled_at_unix_ms INTEGER,
    PRIMARY KEY (tenant_id, principal_id),
    CHECK (display_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_users (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    principal_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL,
    deleted_at_unix_ms INTEGER,
    PRIMARY KEY (tenant_id, principal_id),
    UNIQUE (tenant_id, external_id),
    CHECK (external_id <> ''),
    CHECK (display_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_role_bindings (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    role_binding_id TEXT NOT NULL,
    principal_id TEXT NOT NULL,
    role_name TEXT NOT NULL,
    granted_at_unix_ms INTEGER NOT NULL,
    revoked_at_unix_ms INTEGER,
    PRIMARY KEY (tenant_id, role_binding_id),
    UNIQUE (tenant_id, principal_id, role_name),
    CHECK (role_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_provider_credentials (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    provider_credential_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    secret_provider TEXT NOT NULL,
    secret_name TEXT NOT NULL,
    secret_version TEXT,
    rotated_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, provider_credential_id),
    UNIQUE (tenant_id, provider_name),
    CHECK (provider_name <> ''),
    CHECK (secret_provider <> ''),
    CHECK (secret_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_budget_counters (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    storage_scope TEXT NOT NULL,
    virtual_key_id TEXT,
    reserved_tokens INTEGER NOT NULL DEFAULT 0,
    reserved_cost_micros INTEGER NOT NULL DEFAULT 0,
    committed_tokens INTEGER NOT NULL DEFAULT 0,
    committed_cost_micros INTEGER NOT NULL DEFAULT 0,
    updated_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, storage_scope),
    CHECK (storage_scope <> ''),
    CHECK (reserved_tokens >= 0),
    CHECK (reserved_cost_micros >= 0),
    CHECK (committed_tokens >= 0),
    CHECK (committed_cost_micros >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_budget_policies (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    budget_scope TEXT NOT NULL,
    max_tokens INTEGER NOT NULL,
    max_cost_micros INTEGER NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, budget_scope),
    CHECK (budget_scope <> ''),
    CHECK (max_tokens >= 0),
    CHECK (max_cost_micros >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_reservations (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    reservation_id TEXT NOT NULL,
    call_id TEXT NOT NULL,
    virtual_key_id TEXT,
    idempotency_key TEXT NOT NULL,
    reserved_tokens INTEGER NOT NULL,
    reserved_cost_micros INTEGER NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    expires_at_unix_ms INTEGER NOT NULL,
    committed_at_unix_ms INTEGER,
    released_at_unix_ms INTEGER,
    PRIMARY KEY (tenant_id, reservation_id),
    UNIQUE (tenant_id, call_id),
    UNIQUE (tenant_id, idempotency_key),
    CHECK (reserved_tokens >= 0),
    CHECK (reserved_cost_micros >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_usage_ledger (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    ledger_event_id TEXT NOT NULL,
    reservation_id TEXT NOT NULL,
    call_id TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    tokens INTEGER NOT NULL,
    cost_micros INTEGER NOT NULL,
    occurred_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, ledger_event_id),
    UNIQUE (tenant_id, reservation_id, event_kind),
    CHECK (tokens >= 0),
    CHECK (cost_micros >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_audit_log (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    audit_event_id TEXT NOT NULL,
    previous_digest TEXT,
    event_digest TEXT NOT NULL,
    occurred_at_unix_ms INTEGER NOT NULL,
    principal_id TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_kind TEXT NOT NULL,
    resource_id TEXT,
    outcome TEXT NOT NULL,
    reason_code TEXT,
    PRIMARY KEY (tenant_id, audit_event_id),
    UNIQUE (tenant_id, event_digest),
    UNIQUE (tenant_id, previous_digest),
    CHECK (event_digest <> ''),
    CHECK (action <> ''),
    CHECK (resource_kind <> ''),
    CHECK (outcome <> '')
);

CREATE TABLE IF NOT EXISTS prodex_idempotency_records (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    entry_status TEXT NOT NULL,
    started_at_unix_ms INTEGER NOT NULL,
    completed_at_unix_ms INTEGER,
    response_body BLOB,
    PRIMARY KEY (tenant_id, idempotency_key),
    CHECK (idempotency_key <> ''),
    CHECK (request_fingerprint <> ''),
    CHECK (entry_status IN ('pending', 'completed'))
);

CREATE INDEX IF NOT EXISTS prodex_virtual_keys_tenant_name_idx
    ON prodex_virtual_keys (tenant_id, display_name);
CREATE INDEX IF NOT EXISTS prodex_service_identities_tenant_name_idx
    ON prodex_service_identities (tenant_id, display_name);
CREATE INDEX IF NOT EXISTS prodex_users_tenant_external_idx
    ON prodex_users (tenant_id, external_id, deleted_at_unix_ms);
CREATE INDEX IF NOT EXISTS prodex_role_bindings_tenant_principal_idx
    ON prodex_role_bindings (tenant_id, principal_id, revoked_at_unix_ms);
CREATE INDEX IF NOT EXISTS prodex_provider_credentials_tenant_provider_idx
    ON prodex_provider_credentials (tenant_id, provider_name);
CREATE INDEX IF NOT EXISTS prodex_budget_policies_tenant_scope_idx
    ON prodex_budget_policies (tenant_id, budget_scope);
CREATE INDEX IF NOT EXISTS prodex_reservations_tenant_call_idx
    ON prodex_reservations (tenant_id, call_id);
CREATE INDEX IF NOT EXISTS prodex_usage_ledger_tenant_call_idx
    ON prodex_usage_ledger (tenant_id, call_id);
CREATE INDEX IF NOT EXISTS prodex_audit_log_tenant_time_idx
    ON prodex_audit_log (tenant_id, occurred_at_unix_ms);
CREATE INDEX IF NOT EXISTS prodex_idempotency_records_tenant_status_idx
    ON prodex_idempotency_records (tenant_id, entry_status, started_at_unix_ms);
"#,
};

pub const SQLITE_MIGRATIONS: &[SqliteMigration] = &[INITIAL_LOCAL_ACCOUNTING_MIGRATION];
pub const REQUIRED_SQLITE_SCHEMA_VERSION: SqliteMigrationVersion = SqliteMigrationVersion(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteRuntimeMode {
    ExternalMigrator,
    GatewayRequestPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteBackendOpenMode {
    ExternalMigrator,
    GatewayStartup,
    GatewayRequestPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqliteStoragePlanError {
    DdlForbiddenOnRequestPath,
    MissingSchemaVersion,
    SchemaVersionTooOld {
        observed: SqliteMigrationVersion,
        required: SqliteMigrationVersion,
    },
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    VirtualKeyMismatch {
        key_virtual_key_id: Option<VirtualKeyId>,
        request_virtual_key_id: VirtualKeyId,
    },
    ReservationExceedsLimit {
        requested: UsageAmount,
        limit: UsageAmount,
    },
    ActualUsageExceedsReserved {
        reserved: UsageAmount,
        actual: UsageAmount,
    },
    CommittedUsageOverflow {
        committed: UsageAmount,
        actual: UsageAmount,
    },
    ReservationNotExpired,
    InvalidReservationRecord,
    ReservedBalanceUnderflow {
        reserved: UsageAmount,
        available: UsageAmount,
    },
    EmptyIdempotencyKey,
    EmptyBudgetScope,
    EmptyTenantDisplayName,
    EmptyUserExternalId,
    EmptyUserDisplayName,
    InvalidTimeRange,
}

impl fmt::Display for SqliteStoragePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DdlForbiddenOnRequestPath => {
                write!(f, "SQLite DDL must run only from explicit migration paths")
            }
            Self::MissingSchemaVersion => write!(
                f,
                "SQLite schema version must be checked before opening request-serving backends"
            ),
            Self::SchemaVersionTooOld { .. } => {
                write!(f, "SQLite schema version is too old")
            }
            Self::TenantMismatch { .. } => write!(f, "SQLite tenant mismatch"),
            Self::VirtualKeyMismatch { .. } => write!(f, "SQLite virtual key mismatch"),
            Self::ReservationExceedsLimit { .. } => {
                write!(f, "reservation exceeds configured limit")
            }
            Self::ActualUsageExceedsReserved { .. } => {
                write!(f, "actual usage exceeds reserved usage")
            }
            Self::CommittedUsageOverflow { .. } => write!(f, "committed usage overflow"),
            Self::ReservationNotExpired => write!(f, "reservation is not expired"),
            Self::InvalidReservationRecord => write!(f, "reservation record is invalid"),
            Self::ReservedBalanceUnderflow { .. } => write!(f, "reserved usage underflow"),
            Self::EmptyIdempotencyKey => write!(f, "idempotency key must not be empty"),
            Self::EmptyBudgetScope => write!(f, "budget scope must not be empty"),
            Self::EmptyTenantDisplayName => write!(f, "tenant display name must not be empty"),
            Self::EmptyUserExternalId => write!(f, "user external id must not be empty"),
            Self::EmptyUserDisplayName => write!(f, "user display name must not be empty"),
            Self::InvalidTimeRange => write!(f, "time range is invalid"),
        }
    }
}

impl Error for SqliteStoragePlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteStorageErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteStorageErrorResponsePlan {
    pub status: SqliteStorageErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_sqlite_storage_error_response(
    _error: &SqliteStoragePlanError,
) -> SqliteStorageErrorResponsePlan {
    SqliteStorageErrorResponsePlan {
        status: SqliteStorageErrorStatus::ServiceUnavailable,
        code: "sqlite_storage_unavailable",
        message: "sqlite storage planning is temporarily unavailable",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteMigrationPlan {
    pub migrations: Vec<SqliteMigration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBackendOpenPlan {
    pub mode: SqliteBackendOpenMode,
    pub required_schema_version: SqliteMigrationVersion,
    pub ddl_allowed: bool,
    pub migration_count: usize,
}

pub fn plan_sqlite_migrations(
    mode: SqliteRuntimeMode,
) -> Result<SqliteMigrationPlan, SqliteStoragePlanError> {
    match mode {
        SqliteRuntimeMode::ExternalMigrator => Ok(SqliteMigrationPlan {
            migrations: SQLITE_MIGRATIONS.to_vec(),
        }),
        SqliteRuntimeMode::GatewayRequestPath => {
            Err(SqliteStoragePlanError::DdlForbiddenOnRequestPath)
        }
    }
}

pub fn plan_sqlite_backend_open(
    mode: SqliteBackendOpenMode,
    observed_schema_version: Option<SqliteMigrationVersion>,
) -> Result<SqliteBackendOpenPlan, SqliteStoragePlanError> {
    match mode {
        SqliteBackendOpenMode::ExternalMigrator => Ok(SqliteBackendOpenPlan {
            mode,
            required_schema_version: REQUIRED_SQLITE_SCHEMA_VERSION,
            ddl_allowed: true,
            migration_count: SQLITE_MIGRATIONS.len(),
        }),
        SqliteBackendOpenMode::GatewayStartup | SqliteBackendOpenMode::GatewayRequestPath => {
            let observed =
                observed_schema_version.ok_or(SqliteStoragePlanError::MissingSchemaVersion)?;
            if observed < REQUIRED_SQLITE_SCHEMA_VERSION {
                return Err(SqliteStoragePlanError::SchemaVersionTooOld {
                    observed,
                    required: REQUIRED_SQLITE_SCHEMA_VERSION,
                });
            }
            Ok(SqliteBackendOpenPlan {
                mode,
                required_schema_version: REQUIRED_SQLITE_SCHEMA_VERSION,
                ddl_allowed: false,
                migration_count: 0,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAtomicReservationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAppendOnlyAuditSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAuditRetentionPurgeSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub event_count: usize,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAuditExportQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: AuditSortOrder,
    pub page_limit: u16,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteRoleBindingMutationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub role_binding_id: RoleBindingId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteServiceIdentityCreateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteUserLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBudgetPolicyUpdateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteTenantLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteVirtualKeySecretReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub virtual_key_id: VirtualKeyId,
    pub display_name: String,
    pub kind: VirtualKeySecretReferenceKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteProviderCredentialReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteUsageReconciliationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub ledger_event_count: usize,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBillingLedgerQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: BillingLedgerSortOrder,
    pub page_limit: u16,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteExpiredReservationRecoverySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteIdempotencyPendingRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteIdempotencyCompletedRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub response_byte_count: usize,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteIdempotencyRecordLookupSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqliteStatement {
    pub name: &'static str,
    pub sql: &'static str,
}

pub const SQLITE_BEGIN_IMMEDIATE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_reservation",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_AUDIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_audit_append",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_AUDIT_RETENTION_PURGE_STATEMENT: SqliteStatement =
    SqliteStatement {
        name: "begin_immediate_audit_retention_purge",
        sql: "BEGIN IMMEDIATE",
    };

pub const SQLITE_BEGIN_IMMEDIATE_RECONCILIATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_usage_reconciliation",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_RECOVERY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_expired_recovery",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_PENDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_idempotency_pending",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_COMPLETION_STATEMENT: SqliteStatement =
    SqliteStatement {
        name: "begin_immediate_idempotency_completion",
        sql: "BEGIN IMMEDIATE",
    };

pub const SQLITE_BEGIN_IMMEDIATE_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_role_binding_mutation",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_SERVICE_IDENTITY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_service_identity_create",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_user_lifecycle",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_BUDGET_POLICY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_budget_policy_update",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_TENANT_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_tenant_lifecycle",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_VIRTUAL_KEY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_virtual_key_secret_reference",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_PROVIDER_CREDENTIAL_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_provider_credential_reference",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_APPEND_AUDIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "append_audit_hash_chain_locally",
    sql: r#"
INSERT OR IGNORE INTO prodex_audit_log (
    tenant_id,
    audit_event_id,
    previous_digest,
    event_digest,
    occurred_at_unix_ms,
    principal_id,
    action,
    resource_kind,
    resource_id,
    outcome,
    reason_code
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);
"#,
};

pub const SQLITE_PURGE_AUDIT_RETENTION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "purge_audit_retention_batch_locally",
    sql: r#"
DELETE FROM prodex_audit_log
WHERE tenant_id = ?1
  AND audit_event_id IN (?2);
"#,
};

pub const SQLITE_QUERY_AUDIT_EXPORT_ASC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_audit_export_asc_locally",
    sql: r#"
SELECT
    tenant_id,
    audit_event_id,
    previous_digest,
    event_digest,
    occurred_at_unix_ms,
    principal_id,
    action,
    resource_kind,
    resource_id,
    outcome,
    reason_code
FROM prodex_audit_log
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms ASC, audit_event_id ASC
LIMIT ?4;
"#,
};

pub const SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_audit_export_desc_locally",
    sql: r#"
SELECT
    tenant_id,
    audit_event_id,
    previous_digest,
    event_digest,
    occurred_at_unix_ms,
    principal_id,
    action,
    resource_kind,
    resource_id,
    outcome,
    reason_code
FROM prodex_audit_log
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC
LIMIT ?4;
"#,
};

pub const SQLITE_QUERY_BILLING_LEDGER_ASC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_billing_ledger_asc_locally",
    sql: r#"
SELECT
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
FROM prodex_usage_ledger
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms ASC, ledger_event_id ASC
LIMIT ?4;
"#,
};

pub const SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_billing_ledger_desc_locally",
    sql: r#"
SELECT
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
FROM prodex_usage_ledger
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms DESC, ledger_event_id DESC
LIMIT ?4;
"#,
};

pub const SQLITE_INSERT_IDEMPOTENCY_PENDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "insert_idempotency_pending_locally",
    sql: r#"
INSERT OR IGNORE INTO prodex_idempotency_records (
    tenant_id,
    idempotency_key,
    request_fingerprint,
    entry_status,
    started_at_unix_ms
) VALUES (?1, ?2, ?3, 'pending', ?4);
"#,
};

pub const SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT: SqliteStatement = SqliteStatement {
    name: "complete_idempotency_record_locally",
    sql: r#"
UPDATE prodex_idempotency_records
SET entry_status = 'completed',
    completed_at_unix_ms = ?4,
    response_body = ?5
WHERE tenant_id = ?1
  AND idempotency_key = ?2
  AND request_fingerprint = ?3
  AND entry_status = 'pending';
"#,
};

pub const SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT: SqliteStatement = SqliteStatement {
    name: "lookup_idempotency_record_locally",
    sql: r#"
SELECT
    tenant_id,
    idempotency_key,
    request_fingerprint,
    entry_status,
    started_at_unix_ms,
    completed_at_unix_ms,
    response_body
FROM prodex_idempotency_records
WHERE tenant_id = ?1
  AND idempotency_key = ?2;
"#,
};

pub const SQLITE_GRANT_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "grant_role_binding_locally",
    sql: r#"
INSERT INTO prodex_role_bindings (
    tenant_id,
    role_binding_id,
    principal_id,
    role_name,
    granted_at_unix_ms,
    revoked_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, NULL)
ON CONFLICT(tenant_id, principal_id, role_name) DO UPDATE SET
    role_binding_id = excluded.role_binding_id,
    granted_at_unix_ms = excluded.granted_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_role_bindings.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_REVOKE_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "revoke_role_binding_locally",
    sql: r#"
UPDATE prodex_role_bindings
SET revoked_at_unix_ms = ?5
WHERE tenant_id = ?1
  AND role_binding_id = ?2
  AND principal_id = ?3
  AND role_name = ?4
  AND revoked_at_unix_ms IS NULL;
"#,
};

pub const SQLITE_UPSERT_SERVICE_IDENTITY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_service_identity_locally",
    sql: r#"
INSERT INTO prodex_service_identities (
    tenant_id,
    principal_id,
    display_name,
    created_at_unix_ms,
    disabled_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, NULL)
ON CONFLICT(tenant_id, principal_id) DO UPDATE SET
    display_name = excluded.display_name,
    created_at_unix_ms = excluded.created_at_unix_ms,
    disabled_at_unix_ms = NULL
WHERE prodex_service_identities.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_user_lifecycle_locally",
    sql: r#"
INSERT INTO prodex_users (
    tenant_id,
    principal_id,
    external_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms,
    deleted_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?5, NULL)
ON CONFLICT(tenant_id, principal_id) DO UPDATE SET
    external_id = excluded.external_id,
    display_name = excluded.display_name,
    updated_at_unix_ms = excluded.updated_at_unix_ms,
    deleted_at_unix_ms = NULL
WHERE prodex_users.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_DELETE_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "delete_user_lifecycle_locally",
    sql: r#"
UPDATE prodex_users
SET deleted_at_unix_ms = ?5,
    updated_at_unix_ms = ?5
WHERE tenant_id = ?1
  AND principal_id = ?2
  AND external_id = ?3
  AND deleted_at_unix_ms IS NULL;
"#,
};

pub const SQLITE_UPSERT_BUDGET_POLICY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_budget_policy_locally",
    sql: r#"
INSERT INTO prodex_budget_policies (
    tenant_id,
    budget_scope,
    max_tokens,
    max_cost_micros,
    updated_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(tenant_id, budget_scope) DO UPDATE SET
    max_tokens = excluded.max_tokens,
    max_cost_micros = excluded.max_cost_micros,
    updated_at_unix_ms = excluded.updated_at_unix_ms
WHERE prodex_budget_policies.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_TENANT_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_tenant_lifecycle_locally",
    sql: r#"
INSERT INTO prodex_tenants (
    tenant_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms
) VALUES (?1, ?2, ?3, ?3)
ON CONFLICT(tenant_id) DO UPDATE SET
    display_name = excluded.display_name,
    updated_at_unix_ms = excluded.updated_at_unix_ms
WHERE prodex_tenants.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_virtual_key_secret_reference_locally",
    sql: r#"
INSERT INTO prodex_virtual_keys (
    tenant_id,
    virtual_key_id,
    principal_id,
    display_name,
    secret_provider,
    secret_name,
    secret_version,
    secret_rotated_at_unix_ms,
    created_at_unix_ms,
    revoked_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, NULL)
ON CONFLICT(tenant_id, virtual_key_id) DO UPDATE SET
    principal_id = excluded.principal_id,
    display_name = excluded.display_name,
    secret_provider = excluded.secret_provider,
    secret_name = excluded.secret_name,
    secret_version = excluded.secret_version,
    secret_rotated_at_unix_ms = excluded.secret_rotated_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_virtual_keys.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT: SqliteStatement =
    SqliteStatement {
        name: "upsert_provider_credential_reference_locally",
        sql: r#"
INSERT INTO prodex_provider_credentials (
    tenant_id,
    provider_credential_id,
    provider_name,
    secret_provider,
    secret_name,
    secret_version,
    rotated_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(tenant_id, provider_name) DO UPDATE SET
    provider_credential_id = excluded.provider_credential_id,
    secret_provider = excluded.secret_provider,
    secret_name = excluded.secret_name,
    secret_version = excluded.secret_version,
    rotated_at_unix_ms = excluded.rotated_at_unix_ms
WHERE prodex_provider_credentials.tenant_id = excluded.tenant_id;
"#,
    };

pub const SQLITE_ATOMIC_RESERVATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "reserve_usage_locally",
    sql: r#"
INSERT INTO prodex_budget_counters (
    tenant_id,
    storage_scope,
    virtual_key_id,
    reserved_tokens,
    reserved_cost_micros,
    committed_tokens,
    committed_cost_micros,
    updated_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6)
ON CONFLICT(tenant_id, storage_scope) DO UPDATE SET
    reserved_tokens = reserved_tokens + excluded.reserved_tokens,
    reserved_cost_micros = reserved_cost_micros + excluded.reserved_cost_micros,
    updated_at_unix_ms = excluded.updated_at_unix_ms
WHERE prodex_budget_counters.tenant_id = excluded.tenant_id
  AND prodex_budget_counters.reserved_tokens + prodex_budget_counters.committed_tokens + excluded.reserved_tokens <= ?7
  AND prodex_budget_counters.reserved_cost_micros + prodex_budget_counters.committed_cost_micros + excluded.reserved_cost_micros <= ?8;

INSERT OR IGNORE INTO prodex_reservations (
    tenant_id,
    reservation_id,
    call_id,
    virtual_key_id,
    idempotency_key,
    reserved_tokens,
    reserved_cost_micros,
    created_at_unix_ms,
    expires_at_unix_ms
) VALUES (?1, ?9, ?10, ?3, ?11, ?4, ?5, ?6, ?12);

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
) VALUES (?1, ?13, ?9, ?10, 'reserved', ?4, ?5, ?6);
"#,
};

pub const SQLITE_RECONCILE_USAGE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "reconcile_usage_locally",
    sql: r#"
UPDATE prodex_budget_counters
SET reserved_tokens = reserved_tokens - ?4,
    reserved_cost_micros = reserved_cost_micros - ?5,
    committed_tokens = committed_tokens + ?6,
    committed_cost_micros = committed_cost_micros + ?7,
    updated_at_unix_ms = ?8
WHERE tenant_id = ?1
  AND storage_scope = ?9
  AND reserved_tokens >= ?4
  AND reserved_cost_micros >= ?5
  AND EXISTS (
      SELECT 1
      FROM prodex_reservations
      WHERE tenant_id = ?1
        AND reservation_id = ?2
        AND call_id = ?3
        AND committed_at_unix_ms IS NULL
  );

UPDATE prodex_reservations
SET committed_at_unix_ms = ?8,
    released_at_unix_ms = CASE WHEN ?10 > 0 OR ?11 > 0 THEN ?8 ELSE released_at_unix_ms END
WHERE tenant_id = ?1
  AND reservation_id = ?2
  AND call_id = ?3
  AND committed_at_unix_ms IS NULL;

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
) VALUES (?1, ?12, ?2, ?3, 'committed', ?6, ?7, ?8);

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
)
SELECT ?1, ?13, ?2, ?3, 'released', ?10, ?11, ?8
WHERE ?10 > 0 OR ?11 > 0;
"#,
};

pub const SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "recover_expired_reservation_locally",
    sql: r#"
UPDATE prodex_budget_counters
SET reserved_tokens = reserved_tokens - ?5,
    reserved_cost_micros = reserved_cost_micros - ?6,
    updated_at_unix_ms = ?4
WHERE tenant_id = ?1
  AND storage_scope = ?7
  AND reserved_tokens >= ?5
  AND reserved_cost_micros >= ?6
  AND EXISTS (
      SELECT 1
      FROM prodex_reservations
      WHERE tenant_id = ?1
        AND reservation_id = ?2
        AND call_id = ?3
        AND committed_at_unix_ms IS NULL
        AND released_at_unix_ms IS NULL
        AND expires_at_unix_ms <= ?4
  );

UPDATE prodex_reservations
SET released_at_unix_ms = ?4
WHERE tenant_id = ?1
  AND reservation_id = ?2
  AND call_id = ?3
  AND committed_at_unix_ms IS NULL
  AND released_at_unix_ms IS NULL
  AND expires_at_unix_ms <= ?4;

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
) VALUES (?1, ?8, ?2, ?3, 'released', ?5, ?6, ?4);
"#,
};

pub const SQLITE_COMMIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_reservation",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_AUDIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_audit_append",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_RECONCILIATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_usage_reconciliation",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_RECOVERY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_expired_recovery",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_IDEMPOTENCY_PENDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_idempotency_pending",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_IDEMPOTENCY_COMPLETION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_idempotency_completion",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_role_binding_mutation",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_SERVICE_IDENTITY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_service_identity_create",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_user_lifecycle",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_BUDGET_POLICY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_budget_policy_update",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_TENANT_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_tenant_lifecycle",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_VIRTUAL_KEY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_virtual_key_secret_reference",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_PROVIDER_CREDENTIAL_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_provider_credential_reference",
    sql: "COMMIT",
};

pub fn plan_sqlite_atomic_reservation(
    command: AtomicReservationCommand,
) -> Result<SqliteAtomicReservationSqlPlan, SqliteStoragePlanError> {
    validate_sqlite_reservation_inputs(
        command.storage_key,
        &command.idempotency_key,
        command.limit,
        command.request,
    )?;
    Ok(SqliteAtomicReservationSqlPlan {
        tenant_id: command.request.tenant_id,
        storage_key: command.storage_key,
        idempotency_key: command.idempotency_key,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_STATEMENT,
            SQLITE_ATOMIC_RESERVATION_STATEMENT,
            SQLITE_COMMIT_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_append_only_audit(
    command: AppendOnlyAuditCommand,
) -> Result<SqliteAppendOnlyAuditSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_append_only_audit(command).map_err(|error| match error {
        prodex_storage::AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant,
            event_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: event_tenant,
        },
    })?;
    Ok(SqliteAppendOnlyAuditSqlPlan {
        tenant_id: plan.envelope.event.tenant_id,
        storage_key: plan.storage_key,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_AUDIT_STATEMENT,
            SQLITE_APPEND_AUDIT_STATEMENT,
            SQLITE_COMMIT_AUDIT_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_audit_retention_purge(
    command: AuditRetentionPurgeCommand,
) -> Result<SqliteAuditRetentionPurgeSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_audit_retention_purge(command).map_err(|error| match error {
            prodex_storage::AuditRetentionPurgePlanError::TenantMismatch {
                key_tenant,
                batch_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: batch_tenant,
            },
        })?;
    Ok(SqliteAuditRetentionPurgeSqlPlan {
        tenant_id: plan.batch.tenant_id,
        storage_key: plan.storage_key,
        event_count: plan.batch.len(),
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_AUDIT_RETENTION_PURGE_STATEMENT,
            SQLITE_PURGE_AUDIT_RETENTION_STATEMENT,
            SQLITE_COMMIT_AUDIT_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_audit_export_query(
    command: AuditExportQueryCommand,
) -> Result<SqliteAuditExportQuerySqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_audit_export_query(command).map_err(|error| match error {
        prodex_storage::AuditExportQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
    })?;
    let statement = match plan.sort_order {
        AuditSortOrder::OccurredAtAsc => SQLITE_QUERY_AUDIT_EXPORT_ASC_STATEMENT,
        AuditSortOrder::OccurredAtDesc => SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT,
    };
    Ok(SqliteAuditExportQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: vec![statement],
    })
}

pub fn plan_sqlite_role_binding_mutation(
    command: RoleBindingMutationCommand,
) -> Result<SqliteRoleBindingMutationSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_role_binding_mutation(command).map_err(|error| match error {
            prodex_storage::RoleBindingMutationPlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    let operation = match plan.kind {
        RoleBindingMutationKind::Grant => SQLITE_GRANT_ROLE_BINDING_STATEMENT,
        RoleBindingMutationKind::Revoke => SQLITE_REVOKE_ROLE_BINDING_STATEMENT,
    };
    Ok(SqliteRoleBindingMutationSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        role_binding_id: plan.role_binding_id,
        role: plan.role,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_ROLE_BINDING_STATEMENT,
            operation,
            SQLITE_COMMIT_ROLE_BINDING_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_service_identity_create(
    command: ServiceIdentityCreateCommand,
) -> Result<SqliteServiceIdentityCreateSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_service_identity_create(command).map_err(|error| match error {
            prodex_storage::ServiceIdentityCreatePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    Ok(SqliteServiceIdentityCreateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        display_name: plan.display_name,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_SERVICE_IDENTITY_STATEMENT,
            SQLITE_UPSERT_SERVICE_IDENTITY_STATEMENT,
            SQLITE_COMMIT_SERVICE_IDENTITY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_user_lifecycle(
    command: UserLifecycleCommand,
) -> Result<SqliteUserLifecycleSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_user_lifecycle(command).map_err(|error| match error {
        prodex_storage::UserLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::UserLifecyclePlanError::EmptyExternalId => {
            SqliteStoragePlanError::EmptyUserExternalId
        }
        prodex_storage::UserLifecyclePlanError::EmptyDisplayName => {
            SqliteStoragePlanError::EmptyUserDisplayName
        }
    })?;
    let operation = match plan.kind {
        UserLifecycleKind::Create | UserLifecycleKind::Update => {
            SQLITE_UPSERT_USER_LIFECYCLE_STATEMENT
        }
        UserLifecycleKind::Delete => SQLITE_DELETE_USER_LIFECYCLE_STATEMENT,
    };
    Ok(SqliteUserLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        external_id: plan.external_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_USER_LIFECYCLE_STATEMENT,
            operation,
            SQLITE_COMMIT_USER_LIFECYCLE_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_budget_policy_update(
    command: BudgetPolicyUpdateCommand,
) -> Result<SqliteBudgetPolicyUpdateSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_budget_policy_update(command).map_err(|error| match error {
        prodex_storage::BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::BudgetPolicyUpdatePlanError::EmptyBudgetScope => {
            SqliteStoragePlanError::EmptyBudgetScope
        }
    })?;
    Ok(SqliteBudgetPolicyUpdateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        budget_scope: plan.budget_scope,
        limit: plan.limit,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_BUDGET_POLICY_STATEMENT,
            SQLITE_UPSERT_BUDGET_POLICY_STATEMENT,
            SQLITE_COMMIT_BUDGET_POLICY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_tenant_lifecycle(
    command: TenantLifecycleCommand,
) -> Result<SqliteTenantLifecycleSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_tenant_lifecycle(command).map_err(|error| match error {
        prodex_storage::TenantLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::TenantLifecyclePlanError::EmptyDisplayName => {
            SqliteStoragePlanError::EmptyTenantDisplayName
        }
    })?;
    Ok(SqliteTenantLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_TENANT_LIFECYCLE_STATEMENT,
            SQLITE_UPSERT_TENANT_LIFECYCLE_STATEMENT,
            SQLITE_COMMIT_TENANT_LIFECYCLE_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_virtual_key_secret_reference(
    command: VirtualKeySecretReferenceCommand,
) -> Result<SqliteVirtualKeySecretReferenceSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_virtual_key_secret_reference(command).map_err(
            |error| match error {
                prodex_storage::VirtualKeySecretReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
                prodex_storage::VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                } => SqliteStoragePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                },
            },
        )?;
    Ok(SqliteVirtualKeySecretReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        virtual_key_id: plan.virtual_key_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_VIRTUAL_KEY_STATEMENT,
            SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT,
            SQLITE_COMMIT_VIRTUAL_KEY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_provider_credential_reference(
    command: ProviderCredentialReferenceCommand,
) -> Result<SqliteProviderCredentialReferenceSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_provider_credential_reference(command).map_err(
            |error| match error {
                prodex_storage::ProviderCredentialReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
            },
        )?;
    Ok(SqliteProviderCredentialReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        provider_credential_id: plan.provider_credential_id,
        provider_name: plan.provider_name,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_PROVIDER_CREDENTIAL_STATEMENT,
            SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT,
            SQLITE_COMMIT_PROVIDER_CREDENTIAL_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_idempotency_pending_record(
    command: IdempotencyPendingRecordCommand,
) -> Result<SqliteIdempotencyPendingRecordSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_pending_record(command).map_err(|error| match error {
            prodex_storage::IdempotencyPendingRecordPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(SqliteIdempotencyPendingRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_PENDING_STATEMENT,
            SQLITE_INSERT_IDEMPOTENCY_PENDING_STATEMENT,
            SQLITE_COMMIT_IDEMPOTENCY_PENDING_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_idempotency_completed_record(
    command: IdempotencyCompletedRecordCommand,
) -> Result<SqliteIdempotencyCompletedRecordSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_completed_record(command).map_err(
            |error| match error {
                prodex_storage::IdempotencyCompletedRecordPlanError::TenantMismatch {
                    key_tenant,
                    operation_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: operation_tenant,
                },
            },
        )?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    let response_byte_count = match &plan.entry {
        prodex_domain::IdempotencyEntry::Completed(record) => record.response.len(),
        prodex_domain::IdempotencyEntry::Pending { .. } => 0,
    };
    Ok(SqliteIdempotencyCompletedRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        response_byte_count,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_COMPLETION_STATEMENT,
            SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT,
            SQLITE_COMMIT_IDEMPOTENCY_COMPLETION_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_idempotency_record_lookup(
    command: IdempotencyRecordLookupCommand,
) -> Result<SqliteIdempotencyRecordLookupSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_record_lookup(command).map_err(|error| match error {
            prodex_storage::IdempotencyRecordLookupPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    if plan.operation.key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(SqliteIdempotencyRecordLookupSqlPlan {
        tenant_id: plan.operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: plan.operation.key,
        statements: vec![SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT],
    })
}

pub fn plan_sqlite_expired_reservation_recovery(
    command: ExpiredReservationRecoveryCommand,
) -> Result<SqliteExpiredReservationRecoverySqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_expired_reservation_recovery(command).map_err(
            |error| match error {
                prodex_storage::ExpiredReservationRecoveryPlanError::TenantMismatch {
                    key_tenant,
                    record_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: record_tenant,
                },
                prodex_storage::ExpiredReservationRecoveryPlanError::NotExpired => {
                    SqliteStoragePlanError::ReservationNotExpired
                }
                prodex_storage::ExpiredReservationRecoveryPlanError::InvalidRecord => {
                    SqliteStoragePlanError::InvalidReservationRecord
                }
            },
        )?;
    Ok(SqliteExpiredReservationRecoverySqlPlan {
        tenant_id: plan.ledger_event.tenant_id,
        storage_key: plan.storage_key,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_RECOVERY_STATEMENT,
            SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT,
            SQLITE_COMMIT_RECOVERY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_usage_reconciliation(
    command: UsageReconciliationCommand,
) -> Result<SqliteUsageReconciliationSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_usage_reconciliation(command).map_err(|error| match error {
        prodex_storage::UsageReconciliationPlanError::TenantMismatch {
            key_tenant,
            record_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        },
        prodex_storage::UsageReconciliationPlanError::ActualExceedsReserved {
            reserved,
            actual,
        } => SqliteStoragePlanError::ActualUsageExceedsReserved { reserved, actual },
        prodex_storage::UsageReconciliationPlanError::CommittedUsageOverflow {
            committed,
            actual,
        } => SqliteStoragePlanError::CommittedUsageOverflow { committed, actual },
        prodex_storage::UsageReconciliationPlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        } => SqliteStoragePlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        },
    })?;
    Ok(SqliteUsageReconciliationSqlPlan {
        tenant_id: plan.reconciliation.commit.tenant_id,
        storage_key: plan.storage_key,
        ledger_event_count: plan.ledger_events.len(),
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_RECONCILIATION_STATEMENT,
            SQLITE_RECONCILE_USAGE_STATEMENT,
            SQLITE_COMMIT_RECONCILIATION_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_billing_ledger_query(
    command: BillingLedgerQueryCommand,
) -> Result<SqliteBillingLedgerQuerySqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_billing_ledger_query(command).map_err(|error| match error {
        prodex_storage::BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
        prodex_storage::BillingLedgerQueryPlanError::StartAfterEnd { .. } => {
            SqliteStoragePlanError::InvalidTimeRange
        }
    })?;
    let statement = match plan.sort_order {
        BillingLedgerSortOrder::OccurredAtAsc => SQLITE_QUERY_BILLING_LEDGER_ASC_STATEMENT,
        BillingLedgerSortOrder::OccurredAtDesc => SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT,
    };
    Ok(SqliteBillingLedgerQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: vec![statement],
    })
}

fn validate_sqlite_reservation_inputs(
    storage_key: TenantStorageKey,
    idempotency_key: &IdempotencyKey,
    limit: BudgetLimit,
    request: ReservationRequest,
) -> Result<(), SqliteStoragePlanError> {
    if storage_key.tenant_id != request.tenant_id {
        return Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant: storage_key.tenant_id,
            request_tenant: request.tenant_id,
        });
    }
    if request.estimate.exceeds(limit.max) {
        return Err(SqliteStoragePlanError::ReservationExceedsLimit {
            requested: request.estimate,
            limit: limit.max,
        });
    }
    if idempotency_key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(())
}

pub fn statement_contains_ddl(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    [
        "create table",
        "alter table",
        "create index",
        "drop table",
        "pragma journal_mode",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}
