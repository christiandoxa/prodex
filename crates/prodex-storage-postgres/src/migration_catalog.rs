use crate::{
    INITIAL_TENANT_ACCOUNTING_MIGRATION, PostgresBackendOpenMode, PostgresBackendOpenPlan,
    PostgresMigration, PostgresMigrationPhase, PostgresMigrationPlan, PostgresMigrationVersion,
    PostgresRuntimeMode, PostgresStoragePlanError,
};

pub const GROUPED_REQUEST_BUDGET_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(2),
    phase: PostgresMigrationPhase::Expand,
    name: "002_grouped_request_budget_counter",
    sql: r#"
ALTER TABLE prodex_budget_counters
    ADD COLUMN IF NOT EXISTS request_count BIGINT NOT NULL DEFAULT 0
    CHECK (request_count >= 0);
"#,
};

pub const ENTERPRISE_GOVERNANCE_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(3),
    phase: PostgresMigrationPhase::Expand,
    name: "003_enterprise_governance",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_policy_revisions (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    revision_id UUID NOT NULL,
    artifact_checksum TEXT NOT NULL,
    compiled_metadata JSONB NOT NULL,
    lifecycle_state TEXT NOT NULL,
    created_by UUID NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum),
    CHECK (artifact_checksum <> ''),
    CHECK (lifecycle_state IN ('draft', 'pending_approval', 'approved', 'active', 'superseded', 'rolled_back', 'rejected', 'expired'))
);

CREATE TABLE IF NOT EXISTS prodex_policy_pointers (
    tenant_id UUID PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    active_revision_id UUID,
    last_known_good_revision_id UUID,
    etag TEXT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    CHECK (etag <> '')
);

CREATE TABLE IF NOT EXISTS prodex_policy_activation_history (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    activation_id UUID NOT NULL,
    revision_id UUID NOT NULL,
    previous_revision_id UUID,
    action TEXT NOT NULL,
    actor_id UUID NOT NULL,
    occurred_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, activation_id),
    CHECK (action IN ('activate', 'rollback'))
);

CREATE TABLE IF NOT EXISTS prodex_approvals (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    approval_id TEXT NOT NULL,
    approval_kind TEXT NOT NULL,
    approval_scope TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    maker_id UUID NOT NULL,
    lifecycle_state TEXT NOT NULL,
    required_quorum SMALLINT NOT NULL,
    expires_at_unix_ms BIGINT NOT NULL,
    activated_at_unix_ms BIGINT,
    resource_version BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, approval_id),
    UNIQUE (tenant_id, approval_kind, approval_scope, fingerprint),
    CHECK (approval_id <> '' AND approval_scope <> '' AND fingerprint <> ''),
    CHECK (required_quorum BETWEEN 1 AND 16),
    CHECK (resource_version > 0)
);

CREATE TABLE IF NOT EXISTS prodex_approval_votes (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    approval_id TEXT NOT NULL,
    checker_id UUID NOT NULL,
    approved_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, approval_id, checker_id),
    FOREIGN KEY (tenant_id, approval_id) REFERENCES prodex_approvals(tenant_id, approval_id)
);

CREATE TABLE IF NOT EXISTS prodex_classification_rule_revisions (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    compiled_metadata JSONB NOT NULL,
    lifecycle_state TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum),
    CHECK (revision_id <> '' AND artifact_checksum <> '')
);

CREATE TABLE IF NOT EXISTS prodex_provider_registry_revisions (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum),
    CHECK (revision_id <> '' AND artifact_checksum <> '')
);

CREATE TABLE IF NOT EXISTS prodex_provider_descriptors (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    registry_revision_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    adapter_kind TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL,
    trust_tier TEXT NOT NULL,
    deployment_type TEXT NOT NULL,
    approved_regions TEXT[] NOT NULL,
    capability_metadata JSONB NOT NULL,
    data_handling_metadata JSONB NOT NULL,
    static_risk_basis_points INTEGER NOT NULL,
    pricing_revision TEXT NOT NULL,
    secret_provider TEXT NOT NULL,
    secret_name TEXT NOT NULL,
    secret_version TEXT,
    PRIMARY KEY (tenant_id, registry_revision_id, provider_id),
    FOREIGN KEY (tenant_id, registry_revision_id)
        REFERENCES prodex_provider_registry_revisions(tenant_id, revision_id),
    CHECK (provider_id <> '' AND adapter_kind <> '' AND pricing_revision <> ''),
    CHECK (secret_provider <> '' AND secret_name <> ''),
    CHECK (static_risk_basis_points BETWEEN 0 AND 10000)
);

CREATE TABLE IF NOT EXISTS prodex_routing_score_revisions (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    fixed_point_weights JSONB NOT NULL,
    lifecycle_state TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum),
    CHECK (revision_id <> '' AND artifact_checksum <> '')
);

CREATE TABLE IF NOT EXISTS prodex_governance_sessions (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    session_id_hash TEXT NOT NULL,
    principal_id UUID NOT NULL,
    channel TEXT NOT NULL,
    credential_scope TEXT NOT NULL,
    classification TEXT NOT NULL,
    policy_revision_id UUID NOT NULL,
    registry_revision_id TEXT NOT NULL,
    provider_affinity TEXT,
    created_at_unix_ms BIGINT NOT NULL,
    last_seen_at_unix_ms BIGINT NOT NULL,
    absolute_expires_at_unix_ms BIGINT NOT NULL,
    idle_expires_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, session_id_hash),
    CHECK (session_id_hash <> ''),
    CHECK (classification IN ('public', 'internal', 'confidential', 'restricted'))
);

CREATE TABLE IF NOT EXISTS prodex_session_revocations (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    session_id_hash TEXT NOT NULL,
    revoked_at_unix_ms BIGINT NOT NULL,
    reason_code TEXT NOT NULL,
    PRIMARY KEY (tenant_id, session_id_hash),
    CHECK (session_id_hash <> '' AND reason_code <> '')
);

CREATE TABLE IF NOT EXISTS prodex_siem_outbox (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    event_id UUID NOT NULL,
    audit_event_id UUID NOT NULL,
    event_envelope JSONB NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_unix_ms BIGINT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    delivered_at_unix_ms BIGINT,
    PRIMARY KEY (tenant_id, event_id),
    UNIQUE (tenant_id, audit_event_id),
    CHECK (attempt_count >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_siem_dead_letters (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    event_id UUID NOT NULL,
    audit_event_id UUID NOT NULL,
    event_envelope JSONB NOT NULL,
    attempt_count INTEGER NOT NULL,
    stable_reason_code TEXT NOT NULL,
    failed_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, event_id),
    UNIQUE (tenant_id, audit_event_id),
    CHECK (attempt_count > 0 AND stable_reason_code <> '')
);

DO $migration$
DECLARE tenant_table TEXT;
BEGIN
    FOREACH tenant_table IN ARRAY ARRAY[
        'prodex_policy_revisions',
        'prodex_policy_pointers',
        'prodex_policy_activation_history',
        'prodex_approvals',
        'prodex_approval_votes',
        'prodex_classification_rule_revisions',
        'prodex_provider_registry_revisions',
        'prodex_provider_descriptors',
        'prodex_routing_score_revisions',
        'prodex_governance_sessions',
        'prodex_session_revocations',
        'prodex_siem_outbox',
        'prodex_siem_dead_letters'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', tenant_table);
        IF NOT EXISTS (
            SELECT 1 FROM pg_policies
            WHERE schemaname = current_schema()
              AND tablename = tenant_table
              AND policyname = tenant_table || '_tenant_isolation'
        ) THEN
            EXECUTE format(
                'CREATE POLICY %I ON %I USING (tenant_id = current_setting(''prodex.tenant_id'', true)::uuid) WITH CHECK (tenant_id = current_setting(''prodex.tenant_id'', true)::uuid)',
                tenant_table || '_tenant_isolation',
                tenant_table
            );
        END IF;
    END LOOP;
END $migration$;
"#,
};

pub const POSTGRES_MIGRATIONS: &[PostgresMigration] = &[
    INITIAL_TENANT_ACCOUNTING_MIGRATION,
    GROUPED_REQUEST_BUDGET_MIGRATION,
    ENTERPRISE_GOVERNANCE_MIGRATION,
];
pub const REQUIRED_POSTGRES_SCHEMA_VERSION: PostgresMigrationVersion = PostgresMigrationVersion(3);

pub fn plan_postgres_migrations(
    mode: PostgresRuntimeMode,
) -> Result<PostgresMigrationPlan, PostgresStoragePlanError> {
    match mode {
        PostgresRuntimeMode::ExternalMigrator => Ok(PostgresMigrationPlan {
            migrations: POSTGRES_MIGRATIONS.to_vec(),
        }),
        PostgresRuntimeMode::GatewayRequestPath => {
            Err(PostgresStoragePlanError::DdlForbiddenOnRequestPath)
        }
    }
}

pub fn plan_postgres_backend_open(
    mode: PostgresBackendOpenMode,
    observed_schema_version: Option<PostgresMigrationVersion>,
) -> Result<PostgresBackendOpenPlan, PostgresStoragePlanError> {
    match mode {
        PostgresBackendOpenMode::ExternalMigrator => Ok(PostgresBackendOpenPlan {
            mode,
            required_schema_version: REQUIRED_POSTGRES_SCHEMA_VERSION,
            ddl_allowed: true,
            migration_count: POSTGRES_MIGRATIONS.len(),
        }),
        PostgresBackendOpenMode::GatewayStartup | PostgresBackendOpenMode::GatewayRequestPath => {
            let observed =
                observed_schema_version.ok_or(PostgresStoragePlanError::MissingSchemaVersion)?;
            if observed < REQUIRED_POSTGRES_SCHEMA_VERSION {
                return Err(PostgresStoragePlanError::SchemaVersionTooOld {
                    observed,
                    required: REQUIRED_POSTGRES_SCHEMA_VERSION,
                });
            }
            Ok(PostgresBackendOpenPlan {
                mode,
                required_schema_version: REQUIRED_POSTGRES_SCHEMA_VERSION,
                ddl_allowed: false,
                migration_count: 0,
            })
        }
    }
}
