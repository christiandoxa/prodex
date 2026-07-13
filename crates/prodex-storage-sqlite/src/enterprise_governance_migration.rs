use super::{SqliteMigration, SqliteMigrationPhase, SqliteMigrationVersion};

pub const LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION: SqliteMigration = SqliteMigration {
    version: SqliteMigrationVersion(2),
    phase: SqliteMigrationPhase::Expand,
    name: "002_local_enterprise_governance",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_policy_revisions (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL, compiled_metadata TEXT NOT NULL, lifecycle_state TEXT NOT NULL,
    created_by TEXT NOT NULL, created_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, revision_id), UNIQUE (tenant_id, artifact_checksum)
);
CREATE TABLE IF NOT EXISTS prodex_policy_pointers (
    tenant_id TEXT PRIMARY KEY REFERENCES prodex_tenants(tenant_id), active_revision_id TEXT,
    last_known_good_revision_id TEXT, etag TEXT NOT NULL, updated_at_unix_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS prodex_policy_activation_history (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), activation_id TEXT NOT NULL,
    revision_id TEXT NOT NULL, previous_revision_id TEXT, action TEXT NOT NULL, actor_id TEXT NOT NULL,
    occurred_at_unix_ms INTEGER NOT NULL, PRIMARY KEY (tenant_id, activation_id)
);
CREATE TABLE IF NOT EXISTS prodex_approvals (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), approval_id TEXT NOT NULL,
    approval_kind TEXT NOT NULL, approval_scope TEXT NOT NULL, fingerprint TEXT NOT NULL,
    maker_id TEXT NOT NULL, lifecycle_state TEXT NOT NULL, required_quorum INTEGER NOT NULL,
    expires_at_unix_ms INTEGER NOT NULL, activated_at_unix_ms INTEGER, resource_version INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, approval_id),
    UNIQUE (tenant_id, approval_kind, approval_scope, fingerprint),
    CHECK (required_quorum BETWEEN 1 AND 16), CHECK (resource_version > 0)
);
CREATE TABLE IF NOT EXISTS prodex_approval_votes (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), approval_id TEXT NOT NULL,
    checker_id TEXT NOT NULL, approved_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, approval_id, checker_id),
    FOREIGN KEY (tenant_id, approval_id) REFERENCES prodex_approvals(tenant_id, approval_id)
);
CREATE TABLE IF NOT EXISTS prodex_classification_rule_revisions (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL, compiled_metadata TEXT NOT NULL, lifecycle_state TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL, PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum)
);
CREATE TABLE IF NOT EXISTS prodex_provider_registry_revisions (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL, lifecycle_state TEXT NOT NULL, created_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, revision_id), UNIQUE (tenant_id, artifact_checksum)
);
CREATE TABLE IF NOT EXISTS prodex_provider_descriptors (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), registry_revision_id TEXT NOT NULL,
    provider_id TEXT NOT NULL, adapter_kind TEXT NOT NULL, lifecycle_state TEXT NOT NULL,
    trust_tier TEXT NOT NULL, deployment_type TEXT NOT NULL, approved_regions TEXT NOT NULL,
    capability_metadata TEXT NOT NULL, data_handling_metadata TEXT NOT NULL,
    static_risk_basis_points INTEGER NOT NULL, pricing_revision TEXT NOT NULL,
    secret_provider TEXT NOT NULL, secret_name TEXT NOT NULL, secret_version TEXT,
    PRIMARY KEY (tenant_id, registry_revision_id, provider_id),
    FOREIGN KEY (tenant_id, registry_revision_id)
        REFERENCES prodex_provider_registry_revisions(tenant_id, revision_id),
    CHECK (static_risk_basis_points BETWEEN 0 AND 10000)
);
CREATE TABLE IF NOT EXISTS prodex_routing_score_revisions (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL, fixed_point_weights TEXT NOT NULL, lifecycle_state TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL, PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum)
);
CREATE TABLE IF NOT EXISTS prodex_governance_sessions (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), session_id_hash TEXT NOT NULL,
    principal_id TEXT NOT NULL, channel TEXT NOT NULL, credential_scope TEXT NOT NULL,
    classification TEXT NOT NULL, policy_revision_id TEXT NOT NULL, registry_revision_id TEXT NOT NULL,
    provider_affinity TEXT, created_at_unix_ms INTEGER NOT NULL, last_seen_at_unix_ms INTEGER NOT NULL,
    absolute_expires_at_unix_ms INTEGER NOT NULL, idle_expires_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, session_id_hash)
);
CREATE TABLE IF NOT EXISTS prodex_session_revocations (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), session_id_hash TEXT NOT NULL,
    revoked_at_unix_ms INTEGER NOT NULL, reason_code TEXT NOT NULL,
    PRIMARY KEY (tenant_id, session_id_hash)
);
CREATE TABLE IF NOT EXISTS prodex_siem_outbox (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), event_id TEXT NOT NULL,
    audit_event_id TEXT NOT NULL, event_envelope TEXT NOT NULL, attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_unix_ms INTEGER NOT NULL, created_at_unix_ms INTEGER NOT NULL,
    delivered_at_unix_ms INTEGER, PRIMARY KEY (tenant_id, event_id),
    UNIQUE (tenant_id, audit_event_id), CHECK (attempt_count >= 0)
);
CREATE TABLE IF NOT EXISTS prodex_siem_dead_letters (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id), event_id TEXT NOT NULL,
    audit_event_id TEXT NOT NULL, event_envelope TEXT NOT NULL, attempt_count INTEGER NOT NULL,
    stable_reason_code TEXT NOT NULL, failed_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, event_id), UNIQUE (tenant_id, audit_event_id),
    CHECK (attempt_count > 0)
);
"#,
};
