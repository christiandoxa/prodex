use super::*;

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
