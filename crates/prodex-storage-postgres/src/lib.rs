#![forbid(unsafe_code)]
//! PostgreSQL storage plans for tenant-scoped accounting.
//!
//! This crate is intentionally driver-free. It owns versioned SQL text and
//! request-path SQL plans, while external migrators and adapter crates decide
//! how to execute them. Gateway open/request paths must use prepared DML plans
//! only; DDL lives in explicit migration plans.

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

mod governance_sql;
mod migration_catalog;
pub use governance_sql::*;
pub use migration_catalog::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PostgresMigrationVersion(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresMigrationPhase {
    Expand,
    Backfill,
    Contract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostgresMigration {
    pub version: PostgresMigrationVersion,
    pub phase: PostgresMigrationPhase,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const INITIAL_TENANT_ACCOUNTING_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(1),
    phase: PostgresMigrationPhase::Expand,
    name: "001_tenant_accounting_rls",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_tenants (
    tenant_id UUID PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    CHECK (display_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_virtual_keys (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    virtual_key_id UUID NOT NULL,
    principal_id UUID NOT NULL,
    display_name TEXT NOT NULL,
    secret_provider TEXT NOT NULL,
    secret_name TEXT NOT NULL,
    secret_version TEXT,
    secret_rotated_at_unix_ms BIGINT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    revoked_at_unix_ms BIGINT,
    PRIMARY KEY (tenant_id, virtual_key_id),
    CHECK (display_name <> ''),
    CHECK (secret_provider <> ''),
    CHECK (secret_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_service_identities (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    principal_id UUID NOT NULL,
    display_name TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    disabled_at_unix_ms BIGINT,
    PRIMARY KEY (tenant_id, principal_id),
    CHECK (display_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_users (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    principal_id UUID NOT NULL,
    external_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    deleted_at_unix_ms BIGINT,
    PRIMARY KEY (tenant_id, principal_id),
    UNIQUE (tenant_id, external_id),
    CHECK (external_id <> ''),
    CHECK (display_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_role_bindings (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    role_binding_id UUID NOT NULL,
    principal_id UUID NOT NULL,
    role_name TEXT NOT NULL,
    granted_at_unix_ms BIGINT NOT NULL,
    revoked_at_unix_ms BIGINT,
    PRIMARY KEY (tenant_id, role_binding_id),
    UNIQUE (tenant_id, principal_id, role_name),
    CHECK (role_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_provider_credentials (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    provider_credential_id UUID NOT NULL,
    provider_name TEXT NOT NULL,
    secret_provider TEXT NOT NULL,
    secret_name TEXT NOT NULL,
    secret_version TEXT,
    rotated_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, provider_credential_id),
    UNIQUE (tenant_id, provider_name),
    CHECK (provider_name <> ''),
    CHECK (secret_provider <> ''),
    CHECK (secret_name <> '')
);

CREATE TABLE IF NOT EXISTS prodex_budget_counters (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    storage_scope TEXT NOT NULL,
    virtual_key_id UUID,
    reserved_tokens BIGINT NOT NULL DEFAULT 0,
    reserved_cost_micros BIGINT NOT NULL DEFAULT 0,
    committed_tokens BIGINT NOT NULL DEFAULT 0,
    committed_cost_micros BIGINT NOT NULL DEFAULT 0,
    request_count BIGINT NOT NULL DEFAULT 0,
    updated_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, storage_scope),
    CHECK (storage_scope <> ''),
    CHECK (reserved_tokens >= 0),
    CHECK (reserved_cost_micros >= 0),
    CHECK (committed_tokens >= 0),
    CHECK (committed_cost_micros >= 0),
    CHECK (request_count >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_budget_policies (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    budget_scope TEXT NOT NULL,
    max_tokens BIGINT NOT NULL,
    max_cost_micros BIGINT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, budget_scope),
    CHECK (budget_scope <> ''),
    CHECK (max_tokens >= 0),
    CHECK (max_cost_micros >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_reservations (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    reservation_id UUID NOT NULL,
    call_id UUID NOT NULL,
    virtual_key_id UUID,
    idempotency_key TEXT NOT NULL,
    reserved_tokens BIGINT NOT NULL,
    reserved_cost_micros BIGINT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    expires_at_unix_ms BIGINT NOT NULL,
    committed_at_unix_ms BIGINT,
    released_at_unix_ms BIGINT,
    PRIMARY KEY (tenant_id, reservation_id),
    UNIQUE (tenant_id, call_id),
    UNIQUE (tenant_id, idempotency_key),
    CHECK (reserved_tokens >= 0),
    CHECK (reserved_cost_micros >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_usage_ledger (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    ledger_event_id UUID NOT NULL,
    reservation_id UUID NOT NULL,
    call_id UUID NOT NULL,
    event_kind TEXT NOT NULL,
    tokens BIGINT NOT NULL,
    cost_micros BIGINT NOT NULL,
    occurred_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, ledger_event_id),
    UNIQUE (tenant_id, reservation_id, event_kind),
    CHECK (tokens >= 0),
    CHECK (cost_micros >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_audit_log (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    audit_event_id UUID NOT NULL,
    previous_digest TEXT,
    event_digest TEXT NOT NULL,
    occurred_at_unix_ms BIGINT NOT NULL,
    principal_id UUID NOT NULL,
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
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    entry_status TEXT NOT NULL,
    started_at_unix_ms BIGINT NOT NULL,
    completed_at_unix_ms BIGINT,
    response_body BYTEA,
    PRIMARY KEY (tenant_id, idempotency_key),
    CHECK (idempotency_key <> ''),
    CHECK (request_fingerprint <> ''),
    CHECK (entry_status IN ('pending', 'completed'))
);

ALTER TABLE prodex_tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_virtual_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_service_identities ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_users ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_role_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_provider_credentials ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_budget_counters ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_budget_policies ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_reservations ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_usage_ledger ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_idempotency_records ENABLE ROW LEVEL SECURITY;

DO $migration$
DECLARE tenant_table TEXT;
BEGIN
    FOREACH tenant_table IN ARRAY ARRAY[
        'prodex_tenants',
        'prodex_virtual_keys',
        'prodex_service_identities',
        'prodex_users',
        'prodex_role_bindings',
        'prodex_provider_credentials',
        'prodex_budget_counters',
        'prodex_budget_policies',
        'prodex_reservations',
        'prodex_usage_ledger',
        'prodex_audit_log',
        'prodex_idempotency_records'
    ] LOOP
        IF NOT EXISTS (
            SELECT 1
            FROM pg_policies
            WHERE schemaname = current_schema()
              AND tablename = tenant_table
              AND policyname = tenant_table || '_tenant_isolation'
        ) THEN
            EXECUTE format(
                $policy$
                CREATE POLICY %I ON %I
                    USING (tenant_id = current_setting('prodex.tenant_id', true)::uuid)
                    WITH CHECK (tenant_id = current_setting('prodex.tenant_id', true)::uuid)
                $policy$,
                tenant_table || '_tenant_isolation',
                tenant_table
            );
        END IF;
    END LOOP;
END $migration$;
"#,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresRuntimeMode {
    ExternalMigrator,
    GatewayRequestPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresBackendOpenMode {
    ExternalMigrator,
    GatewayStartup,
    GatewayRequestPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostgresStoragePlanError {
    DdlForbiddenOnRequestPath,
    MissingSchemaVersion,
    SchemaVersionTooOld {
        observed: PostgresMigrationVersion,
        required: PostgresMigrationVersion,
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

impl fmt::Display for PostgresStoragePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DdlForbiddenOnRequestPath => {
                write!(f, "PostgreSQL DDL must run only from external migrators")
            }
            Self::MissingSchemaVersion => write!(
                f,
                "PostgreSQL schema version must be checked before opening request-serving backends"
            ),
            Self::SchemaVersionTooOld { .. } => {
                write!(f, "PostgreSQL schema version is too old")
            }
            Self::TenantMismatch { .. } => write!(f, "PostgreSQL tenant mismatch"),
            Self::VirtualKeyMismatch { .. } => write!(f, "PostgreSQL virtual key mismatch"),
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

impl Error for PostgresStoragePlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresStorageErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresStorageErrorResponsePlan {
    pub status: PostgresStorageErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_postgres_storage_error_response(
    _error: &PostgresStoragePlanError,
) -> PostgresStorageErrorResponsePlan {
    PostgresStorageErrorResponsePlan {
        status: PostgresStorageErrorStatus::ServiceUnavailable,
        code: "postgres_storage_unavailable",
        message: "postgres storage planning is temporarily unavailable",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresMigrationPlan {
    pub migrations: Vec<PostgresMigration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresBackendOpenPlan {
    pub mode: PostgresBackendOpenMode,
    pub required_schema_version: PostgresMigrationVersion,
    pub ddl_allowed: bool,
    pub migration_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresTenantContextSqlPlan {
    pub tenant_id: TenantId,
    pub statement: PostgresStatement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAtomicReservationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAppendOnlyAuditSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAuditRetentionPurgeSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub event_count: usize,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAuditExportQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: AuditSortOrder,
    pub page_limit: u16,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresRoleBindingMutationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub role_binding_id: RoleBindingId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresServiceIdentityCreateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresUserLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresBudgetPolicyUpdateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresTenantLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresVirtualKeySecretReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub virtual_key_id: VirtualKeyId,
    pub display_name: String,
    pub kind: VirtualKeySecretReferenceKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresProviderCredentialReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresUsageReconciliationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub ledger_event_count: usize,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresBillingLedgerQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: BillingLedgerSortOrder,
    pub page_limit: u16,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresExpiredReservationRecoverySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIdempotencyPendingRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIdempotencyCompletedRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub response_byte_count: usize,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIdempotencyRecordLookupSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostgresStatement {
    pub name: &'static str,
    pub sql: &'static str,
}

pub const SET_TENANT_STATEMENT: PostgresStatement = PostgresStatement {
    name: "set_tenant_context",
    sql: "SELECT set_config('prodex.tenant_id', $1, true)",
};

pub fn plan_postgres_tenant_context(tenant_id: TenantId) -> PostgresTenantContextSqlPlan {
    PostgresTenantContextSqlPlan {
        tenant_id,
        statement: SET_TENANT_STATEMENT,
    }
}

pub const APPEND_AUDIT_STATEMENT: PostgresStatement = PostgresStatement {
    name: "append_audit_hash_chain",
    sql: r#"
INSERT INTO prodex_audit_log (
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
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (tenant_id, audit_event_id) DO NOTHING
RETURNING tenant_id
"#,
};

pub const PURGE_AUDIT_RETENTION_STATEMENT: PostgresStatement = PostgresStatement {
    name: "purge_audit_retention_batch",
    sql: r#"
DELETE FROM prodex_audit_log
WHERE tenant_id = $1
  AND audit_event_id = ANY($2)
RETURNING tenant_id, audit_event_id
"#,
};

pub const QUERY_AUDIT_EXPORT_ASC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_audit_export_asc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms ASC, audit_event_id ASC
LIMIT $4
"#,
};

pub const QUERY_AUDIT_EXPORT_DESC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_audit_export_desc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC
LIMIT $4
"#,
};

pub const QUERY_BILLING_LEDGER_ASC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_billing_ledger_asc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms ASC, ledger_event_id ASC
LIMIT $4
"#,
};

pub const QUERY_BILLING_LEDGER_DESC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_billing_ledger_desc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms DESC, ledger_event_id DESC
LIMIT $4
"#,
};

pub const INSERT_IDEMPOTENCY_PENDING_STATEMENT: PostgresStatement = PostgresStatement {
    name: "insert_idempotency_pending",
    sql: r#"
INSERT INTO prodex_idempotency_records (
    tenant_id,
    idempotency_key,
    request_fingerprint,
    entry_status,
    started_at_unix_ms
) VALUES ($1, $2, $3, 'pending', $4)
ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
RETURNING tenant_id
"#,
};

pub const COMPLETE_IDEMPOTENCY_RECORD_STATEMENT: PostgresStatement = PostgresStatement {
    name: "complete_idempotency_record",
    sql: r#"
UPDATE prodex_idempotency_records
SET entry_status = 'completed',
    completed_at_unix_ms = $4,
    response_body = $5
WHERE tenant_id = $1
  AND idempotency_key = $2
  AND request_fingerprint = $3
  AND entry_status = 'pending'
RETURNING tenant_id
"#,
};

pub const LOOKUP_IDEMPOTENCY_RECORD_STATEMENT: PostgresStatement = PostgresStatement {
    name: "lookup_idempotency_record",
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
WHERE tenant_id = $1
  AND idempotency_key = $2
"#,
};

pub const GRANT_ROLE_BINDING_STATEMENT: PostgresStatement = PostgresStatement {
    name: "grant_role_binding",
    sql: r#"
INSERT INTO prodex_role_bindings (
    tenant_id,
    role_binding_id,
    principal_id,
    role_name,
    granted_at_unix_ms,
    revoked_at_unix_ms
) VALUES ($1, $2, $3, $4, $5, NULL)
ON CONFLICT (tenant_id, principal_id, role_name) DO UPDATE SET
    role_binding_id = EXCLUDED.role_binding_id,
    granted_at_unix_ms = EXCLUDED.granted_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_role_bindings.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const REVOKE_ROLE_BINDING_STATEMENT: PostgresStatement = PostgresStatement {
    name: "revoke_role_binding",
    sql: r#"
UPDATE prodex_role_bindings
SET revoked_at_unix_ms = $5
WHERE tenant_id = $1
  AND role_binding_id = $2
  AND principal_id = $3
  AND role_name = $4
  AND revoked_at_unix_ms IS NULL
RETURNING tenant_id
"#,
};

pub const UPSERT_SERVICE_IDENTITY_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_service_identity",
    sql: r#"
INSERT INTO prodex_service_identities (
    tenant_id,
    principal_id,
    display_name,
    created_at_unix_ms,
    disabled_at_unix_ms
) VALUES ($1, $2, $3, $4, NULL)
ON CONFLICT (tenant_id, principal_id) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    created_at_unix_ms = EXCLUDED.created_at_unix_ms,
    disabled_at_unix_ms = NULL
WHERE prodex_service_identities.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_USER_LIFECYCLE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_user_lifecycle",
    sql: r#"
INSERT INTO prodex_users (
    tenant_id,
    principal_id,
    external_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms,
    deleted_at_unix_ms
) VALUES ($1, $2, $3, $4, $5, $5, NULL)
ON CONFLICT (tenant_id, principal_id) DO UPDATE SET
    external_id = EXCLUDED.external_id,
    display_name = EXCLUDED.display_name,
    updated_at_unix_ms = EXCLUDED.updated_at_unix_ms,
    deleted_at_unix_ms = NULL
WHERE prodex_users.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const DELETE_USER_LIFECYCLE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "delete_user_lifecycle",
    sql: r#"
UPDATE prodex_users
SET deleted_at_unix_ms = $5,
    updated_at_unix_ms = $5
WHERE tenant_id = $1
  AND principal_id = $2
  AND external_id = $3
  AND deleted_at_unix_ms IS NULL
RETURNING tenant_id
"#,
};

pub const UPSERT_BUDGET_POLICY_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_budget_policy",
    sql: r#"
INSERT INTO prodex_budget_policies (
    tenant_id,
    budget_scope,
    max_tokens,
    max_cost_micros,
    updated_at_unix_ms
) VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (tenant_id, budget_scope) DO UPDATE SET
    max_tokens = EXCLUDED.max_tokens,
    max_cost_micros = EXCLUDED.max_cost_micros,
    updated_at_unix_ms = EXCLUDED.updated_at_unix_ms
WHERE prodex_budget_policies.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_TENANT_LIFECYCLE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_tenant_lifecycle",
    sql: r#"
INSERT INTO prodex_tenants (
    tenant_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms
) VALUES ($1, $2, $3, $3)
ON CONFLICT (tenant_id) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    updated_at_unix_ms = EXCLUDED.updated_at_unix_ms
WHERE prodex_tenants.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_virtual_key_secret_reference",
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
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, NULL)
ON CONFLICT (tenant_id, virtual_key_id) DO UPDATE SET
    principal_id = EXCLUDED.principal_id,
    display_name = EXCLUDED.display_name,
    secret_provider = EXCLUDED.secret_provider,
    secret_name = EXCLUDED.secret_name,
    secret_version = EXCLUDED.secret_version,
    secret_rotated_at_unix_ms = EXCLUDED.secret_rotated_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_virtual_keys.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_provider_credential_reference",
    sql: r#"
INSERT INTO prodex_provider_credentials (
    tenant_id,
    provider_credential_id,
    provider_name,
    secret_provider,
    secret_name,
    secret_version,
    rotated_at_unix_ms
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (tenant_id, provider_name) DO UPDATE SET
    provider_credential_id = EXCLUDED.provider_credential_id,
    secret_provider = EXCLUDED.secret_provider,
    secret_name = EXCLUDED.secret_name,
    secret_version = EXCLUDED.secret_version,
    rotated_at_unix_ms = EXCLUDED.rotated_at_unix_ms
WHERE prodex_provider_credentials.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const ATOMIC_RESERVATION_STATEMENT: PostgresStatement = PostgresStatement {
    name: "reserve_usage_atomically",
    sql: r#"
WITH existing AS (
    SELECT tenant_id, reservation_id
    FROM prodex_reservations
    WHERE tenant_id = $1 AND idempotency_key = $2
), upsert_counter AS (
    INSERT INTO prodex_budget_counters (
        tenant_id,
        storage_scope,
        virtual_key_id,
        reserved_tokens,
        reserved_cost_micros,
        committed_tokens,
        committed_cost_micros,
        request_count,
        updated_at_unix_ms
    ) VALUES ($1, $3, $4, $5, $6, 0, 0, 1, $7)
    ON CONFLICT (tenant_id, storage_scope) DO UPDATE SET
        reserved_tokens = prodex_budget_counters.reserved_tokens + EXCLUDED.reserved_tokens,
        reserved_cost_micros = prodex_budget_counters.reserved_cost_micros + EXCLUDED.reserved_cost_micros,
        request_count = prodex_budget_counters.request_count + 1,
        updated_at_unix_ms = EXCLUDED.updated_at_unix_ms
    WHERE NOT EXISTS (SELECT 1 FROM existing)
      AND prodex_budget_counters.tenant_id = EXCLUDED.tenant_id
      AND prodex_budget_counters.reserved_tokens + prodex_budget_counters.committed_tokens + EXCLUDED.reserved_tokens <= $8
      AND prodex_budget_counters.reserved_cost_micros + prodex_budget_counters.committed_cost_micros + EXCLUDED.reserved_cost_micros <= $9
      AND prodex_budget_counters.request_count + 1 <= $10
    RETURNING tenant_id
), reservation_insert AS (
    INSERT INTO prodex_reservations (
        tenant_id,
        reservation_id,
        call_id,
        virtual_key_id,
        idempotency_key,
        reserved_tokens,
        reserved_cost_micros,
        created_at_unix_ms,
        expires_at_unix_ms
    )
    SELECT $1, $11, $12, $4, $2, $5, $6, $7, $13
    WHERE EXISTS (SELECT 1 FROM upsert_counter)
    ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
    RETURNING tenant_id
)
INSERT INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
)
SELECT $1, $14, $11, $12, 'reserved', $5, $6, $7
WHERE EXISTS (SELECT 1 FROM reservation_insert)
ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
RETURNING tenant_id
"#,
};

pub const RECONCILE_USAGE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "reconcile_usage_atomically",
    sql: r#"
WITH locked_reservation AS (
    SELECT tenant_id, reservation_id, call_id, virtual_key_id
    FROM prodex_reservations
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND call_id = $3
      AND committed_at_unix_ms IS NULL
    FOR UPDATE
), update_counter AS (
    UPDATE prodex_budget_counters
    SET reserved_tokens = reserved_tokens - $4,
        reserved_cost_micros = reserved_cost_micros - $5,
        committed_tokens = committed_tokens + $6,
        committed_cost_micros = committed_cost_micros + $7,
        updated_at_unix_ms = $8
    WHERE tenant_id = $1
      AND storage_scope = $9
      AND reserved_tokens >= $4
      AND reserved_cost_micros >= $5
      AND EXISTS (SELECT 1 FROM locked_reservation)
    RETURNING tenant_id
), mark_reservation AS (
    UPDATE prodex_reservations
    SET committed_at_unix_ms = $8,
        released_at_unix_ms = CASE WHEN $10::BIGINT > 0 OR $11::BIGINT > 0 THEN $8 ELSE released_at_unix_ms END
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND EXISTS (SELECT 1 FROM update_counter)
    RETURNING tenant_id
), committed_ledger AS (
    INSERT INTO prodex_usage_ledger (
        tenant_id,
        ledger_event_id,
        reservation_id,
        call_id,
        event_kind,
        tokens,
        cost_micros,
        occurred_at_unix_ms
    )
    SELECT $1, $12, $2, $3, 'committed', $6, $7, $8
    WHERE EXISTS (SELECT 1 FROM mark_reservation)
    ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
    RETURNING tenant_id
), released_ledger AS (
    INSERT INTO prodex_usage_ledger (
        tenant_id,
        ledger_event_id,
        reservation_id,
        call_id,
        event_kind,
        tokens,
        cost_micros,
        occurred_at_unix_ms
    )
    SELECT $1, $13, $2, $3, 'released', $10, $11, $8
    WHERE EXISTS (SELECT 1 FROM mark_reservation)
      AND ($10::BIGINT > 0 OR $11::BIGINT > 0)
    ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
    RETURNING tenant_id
)
SELECT tenant_id FROM mark_reservation
"#,
};

pub const RECOVER_EXPIRED_RESERVATION_STATEMENT: PostgresStatement = PostgresStatement {
    name: "recover_expired_reservation",
    sql: r#"
WITH locked_reservation AS (
    SELECT tenant_id, reservation_id, call_id, reserved_tokens, reserved_cost_micros
    FROM prodex_reservations
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND call_id = $3
      AND committed_at_unix_ms IS NULL
      AND released_at_unix_ms IS NULL
      AND expires_at_unix_ms <= $4
    FOR UPDATE
), update_counter AS (
    UPDATE prodex_budget_counters
    SET reserved_tokens = reserved_tokens - $5,
        reserved_cost_micros = reserved_cost_micros - $6,
        updated_at_unix_ms = $4
    WHERE tenant_id = $1
      AND storage_scope = $7
      AND reserved_tokens >= $5
      AND reserved_cost_micros >= $6
      AND EXISTS (SELECT 1 FROM locked_reservation)
    RETURNING tenant_id
), mark_reservation AS (
    UPDATE prodex_reservations
    SET released_at_unix_ms = $4
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND EXISTS (SELECT 1 FROM update_counter)
    RETURNING tenant_id
)
INSERT INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
)
SELECT $1, $8, $2, $3, 'released', $5, $6, $4
WHERE EXISTS (SELECT 1 FROM mark_reservation)
ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
RETURNING tenant_id
"#,
};

pub fn plan_postgres_atomic_reservation(
    command: AtomicReservationCommand,
) -> Result<PostgresAtomicReservationSqlPlan, PostgresStoragePlanError> {
    validate_postgres_reservation_inputs(
        command.storage_key,
        &command.idempotency_key,
        command.limit,
        command.request,
    )?;
    Ok(PostgresAtomicReservationSqlPlan {
        tenant_id: command.request.tenant_id,
        storage_key: command.storage_key,
        idempotency_key: command.idempotency_key,
        statements: postgres_request_path_statements(ATOMIC_RESERVATION_STATEMENT),
    })
}

pub fn plan_postgres_expired_reservation_recovery(
    command: ExpiredReservationRecoveryCommand,
) -> Result<PostgresExpiredReservationRecoverySqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_expired_reservation_recovery(command).map_err(
            |error| match error {
                prodex_storage::ExpiredReservationRecoveryPlanError::TenantMismatch {
                    key_tenant,
                    record_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: record_tenant,
                },
                prodex_storage::ExpiredReservationRecoveryPlanError::NotExpired => {
                    PostgresStoragePlanError::ReservationNotExpired
                }
                prodex_storage::ExpiredReservationRecoveryPlanError::InvalidRecord => {
                    PostgresStoragePlanError::InvalidReservationRecord
                }
            },
        )?;
    Ok(PostgresExpiredReservationRecoverySqlPlan {
        tenant_id: plan.ledger_event.tenant_id,
        storage_key: plan.storage_key,
        statements: postgres_request_path_statements(RECOVER_EXPIRED_RESERVATION_STATEMENT),
    })
}

pub fn plan_postgres_usage_reconciliation(
    command: UsageReconciliationCommand,
) -> Result<PostgresUsageReconciliationSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_usage_reconciliation(command).map_err(|error| match error {
        prodex_storage::UsageReconciliationPlanError::TenantMismatch {
            key_tenant,
            record_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        },
        prodex_storage::UsageReconciliationPlanError::ActualExceedsReserved {
            reserved,
            actual,
        } => PostgresStoragePlanError::ActualUsageExceedsReserved { reserved, actual },
        prodex_storage::UsageReconciliationPlanError::CommittedUsageOverflow {
            committed,
            actual,
        } => PostgresStoragePlanError::CommittedUsageOverflow { committed, actual },
        prodex_storage::UsageReconciliationPlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        } => PostgresStoragePlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        },
    })?;
    Ok(PostgresUsageReconciliationSqlPlan {
        tenant_id: plan.reconciliation.commit.tenant_id,
        storage_key: plan.storage_key,
        ledger_event_count: plan.ledger_events.len(),
        statements: postgres_request_path_statements(RECONCILE_USAGE_STATEMENT),
    })
}

pub fn plan_postgres_billing_ledger_query(
    command: BillingLedgerQueryCommand,
) -> Result<PostgresBillingLedgerQuerySqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_billing_ledger_query(command).map_err(|error| match error {
        prodex_storage::BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
        prodex_storage::BillingLedgerQueryPlanError::StartAfterEnd { .. } => {
            PostgresStoragePlanError::InvalidTimeRange
        }
    })?;
    let statement = match plan.sort_order {
        BillingLedgerSortOrder::OccurredAtAsc => QUERY_BILLING_LEDGER_ASC_STATEMENT,
        BillingLedgerSortOrder::OccurredAtDesc => QUERY_BILLING_LEDGER_DESC_STATEMENT,
    };
    Ok(PostgresBillingLedgerQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: postgres_request_path_statements(statement),
    })
}

pub fn plan_postgres_append_only_audit(
    command: AppendOnlyAuditCommand,
) -> Result<PostgresAppendOnlyAuditSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_append_only_audit(command).map_err(|error| match error {
        prodex_storage::AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant,
            event_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: event_tenant,
        },
    })?;
    Ok(PostgresAppendOnlyAuditSqlPlan {
        tenant_id: plan.envelope.event.tenant_id,
        storage_key: plan.storage_key,
        statements: postgres_request_path_statements(APPEND_AUDIT_STATEMENT),
    })
}

pub fn plan_postgres_audit_retention_purge(
    command: AuditRetentionPurgeCommand,
) -> Result<PostgresAuditRetentionPurgeSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_audit_retention_purge(command).map_err(|error| match error {
            prodex_storage::AuditRetentionPurgePlanError::TenantMismatch {
                key_tenant,
                batch_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: batch_tenant,
            },
        })?;
    Ok(PostgresAuditRetentionPurgeSqlPlan {
        tenant_id: plan.batch.tenant_id,
        storage_key: plan.storage_key,
        event_count: plan.batch.len(),
        statements: postgres_request_path_statements(PURGE_AUDIT_RETENTION_STATEMENT),
    })
}

pub fn plan_postgres_audit_export_query(
    command: AuditExportQueryCommand,
) -> Result<PostgresAuditExportQuerySqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_audit_export_query(command).map_err(|error| match error {
        prodex_storage::AuditExportQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
    })?;
    let statement = match plan.sort_order {
        AuditSortOrder::OccurredAtAsc => QUERY_AUDIT_EXPORT_ASC_STATEMENT,
        AuditSortOrder::OccurredAtDesc => QUERY_AUDIT_EXPORT_DESC_STATEMENT,
    };
    Ok(PostgresAuditExportQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: postgres_request_path_statements(statement),
    })
}

pub fn plan_postgres_role_binding_mutation(
    command: RoleBindingMutationCommand,
) -> Result<PostgresRoleBindingMutationSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_role_binding_mutation(command).map_err(|error| match error {
            prodex_storage::RoleBindingMutationPlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    let operation = match plan.kind {
        RoleBindingMutationKind::Grant => GRANT_ROLE_BINDING_STATEMENT,
        RoleBindingMutationKind::Revoke => REVOKE_ROLE_BINDING_STATEMENT,
    };
    Ok(PostgresRoleBindingMutationSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        role_binding_id: plan.role_binding_id,
        role: plan.role,
        kind: plan.kind,
        statements: postgres_request_path_statements(operation),
    })
}

pub fn plan_postgres_service_identity_create(
    command: ServiceIdentityCreateCommand,
) -> Result<PostgresServiceIdentityCreateSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_service_identity_create(command).map_err(|error| match error {
            prodex_storage::ServiceIdentityCreatePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    Ok(PostgresServiceIdentityCreateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        display_name: plan.display_name,
        statements: postgres_request_path_statements(UPSERT_SERVICE_IDENTITY_STATEMENT),
    })
}

pub fn plan_postgres_user_lifecycle(
    command: UserLifecycleCommand,
) -> Result<PostgresUserLifecycleSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_user_lifecycle(command).map_err(|error| match error {
        prodex_storage::UserLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::UserLifecyclePlanError::EmptyExternalId => {
            PostgresStoragePlanError::EmptyUserExternalId
        }
        prodex_storage::UserLifecyclePlanError::EmptyDisplayName => {
            PostgresStoragePlanError::EmptyUserDisplayName
        }
    })?;
    let operation = match plan.kind {
        UserLifecycleKind::Create | UserLifecycleKind::Update => UPSERT_USER_LIFECYCLE_STATEMENT,
        UserLifecycleKind::Delete => DELETE_USER_LIFECYCLE_STATEMENT,
    };
    Ok(PostgresUserLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        external_id: plan.external_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: postgres_request_path_statements(operation),
    })
}

pub fn plan_postgres_budget_policy_update(
    command: BudgetPolicyUpdateCommand,
) -> Result<PostgresBudgetPolicyUpdateSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_budget_policy_update(command).map_err(|error| match error {
        prodex_storage::BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::BudgetPolicyUpdatePlanError::EmptyBudgetScope => {
            PostgresStoragePlanError::EmptyBudgetScope
        }
    })?;
    Ok(PostgresBudgetPolicyUpdateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        budget_scope: plan.budget_scope,
        limit: plan.limit,
        statements: postgres_request_path_statements(UPSERT_BUDGET_POLICY_STATEMENT),
    })
}

pub fn plan_postgres_tenant_lifecycle(
    command: TenantLifecycleCommand,
) -> Result<PostgresTenantLifecycleSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_tenant_lifecycle(command).map_err(|error| match error {
        prodex_storage::TenantLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::TenantLifecyclePlanError::EmptyDisplayName => {
            PostgresStoragePlanError::EmptyTenantDisplayName
        }
    })?;
    Ok(PostgresTenantLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: postgres_request_path_statements(UPSERT_TENANT_LIFECYCLE_STATEMENT),
    })
}

pub fn plan_postgres_virtual_key_secret_reference(
    command: VirtualKeySecretReferenceCommand,
) -> Result<PostgresVirtualKeySecretReferenceSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_virtual_key_secret_reference(command).map_err(
            |error| match error {
                prodex_storage::VirtualKeySecretReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
                prodex_storage::VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                } => PostgresStoragePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                },
            },
        )?;
    Ok(PostgresVirtualKeySecretReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        virtual_key_id: plan.virtual_key_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: postgres_request_path_statements(UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT),
    })
}

pub fn plan_postgres_provider_credential_reference(
    command: ProviderCredentialReferenceCommand,
) -> Result<PostgresProviderCredentialReferenceSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_provider_credential_reference(command).map_err(
            |error| match error {
                prodex_storage::ProviderCredentialReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
            },
        )?;
    Ok(PostgresProviderCredentialReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        provider_credential_id: plan.provider_credential_id,
        provider_name: plan.provider_name,
        statements: postgres_request_path_statements(
            UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT,
        ),
    })
}

pub fn plan_postgres_idempotency_pending_record(
    command: IdempotencyPendingRecordCommand,
) -> Result<PostgresIdempotencyPendingRecordSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_pending_record(command).map_err(|error| match error {
            prodex_storage::IdempotencyPendingRecordPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(PostgresIdempotencyPendingRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        statements: postgres_request_path_statements(INSERT_IDEMPOTENCY_PENDING_STATEMENT),
    })
}

pub fn plan_postgres_idempotency_completed_record(
    command: IdempotencyCompletedRecordCommand,
) -> Result<PostgresIdempotencyCompletedRecordSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_completed_record(command).map_err(
            |error| match error {
                prodex_storage::IdempotencyCompletedRecordPlanError::TenantMismatch {
                    key_tenant,
                    operation_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: operation_tenant,
                },
            },
        )?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    let response_byte_count = match &plan.entry {
        prodex_domain::IdempotencyEntry::Completed(record) => record.response.len(),
        prodex_domain::IdempotencyEntry::Pending { .. } => 0,
    };
    Ok(PostgresIdempotencyCompletedRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        response_byte_count,
        statements: postgres_request_path_statements(COMPLETE_IDEMPOTENCY_RECORD_STATEMENT),
    })
}

pub fn plan_postgres_idempotency_record_lookup(
    command: IdempotencyRecordLookupCommand,
) -> Result<PostgresIdempotencyRecordLookupSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_record_lookup(command).map_err(|error| match error {
            prodex_storage::IdempotencyRecordLookupPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    if plan.operation.key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(PostgresIdempotencyRecordLookupSqlPlan {
        tenant_id: plan.operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: plan.operation.key,
        statements: postgres_request_path_statements(LOOKUP_IDEMPOTENCY_RECORD_STATEMENT),
    })
}

fn postgres_request_path_statements(operation: PostgresStatement) -> Vec<PostgresStatement> {
    vec![SET_TENANT_STATEMENT, operation]
}

fn validate_postgres_reservation_inputs(
    storage_key: TenantStorageKey,
    idempotency_key: &IdempotencyKey,
    limit: BudgetLimit,
    request: ReservationRequest,
) -> Result<(), PostgresStoragePlanError> {
    if storage_key.tenant_id != request.tenant_id {
        return Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant: storage_key.tenant_id,
            request_tenant: request.tenant_id,
        });
    }
    if request.estimate.exceeds(limit.max) {
        return Err(PostgresStoragePlanError::ReservationExceedsLimit {
            requested: request.estimate,
            limit: limit.max,
        });
    }
    if idempotency_key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(())
}

pub fn statement_contains_ddl(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    [
        "create table",
        "alter table",
        "create policy",
        "drop table",
        "truncate table",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}
