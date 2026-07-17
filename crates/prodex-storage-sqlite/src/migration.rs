use super::*;

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

pub const SQLITE_MIGRATIONS: &[SqliteMigration] = &[
    INITIAL_LOCAL_ACCOUNTING_MIGRATION,
    LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION,
    LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION,
    LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION,
    LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION,
    LOCAL_GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION,
    LOCAL_APPROVAL_TERMINATION_REASON_MIGRATION,
    LOCAL_SESSION_REVOCATION_EPOCH_MIGRATION,
];
pub const REQUIRED_SQLITE_SCHEMA_VERSION: SqliteMigrationVersion = SqliteMigrationVersion(8);

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
