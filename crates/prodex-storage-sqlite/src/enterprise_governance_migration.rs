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

pub const LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION: SqliteMigration = SqliteMigration {
    version: SqliteMigrationVersion(3),
    phase: SqliteMigrationPhase::Expand,
    name: "003_local_enterprise_governance_hardening",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_pricing_revisions (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    pricing_metadata TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum),
    CHECK (length(revision_id) BETWEEN 1 AND 128),
    CHECK (length(artifact_checksum) BETWEEN 1 AND 128),
    CHECK (length(pricing_metadata) <= 1048576),
    CHECK (length(lifecycle_state) BETWEEN 1 AND 32),
    CHECK (created_at_unix_ms >= 0)
);

CREATE TRIGGER IF NOT EXISTS prodex_provider_descriptors_pricing_revision_insert
BEFORE INSERT ON prodex_provider_descriptors
WHEN NOT EXISTS (
    SELECT 1 FROM prodex_pricing_revisions
    WHERE tenant_id = NEW.tenant_id AND revision_id = NEW.pricing_revision
)
BEGIN
    SELECT RAISE(ABORT, 'provider pricing revision does not exist');
END;

CREATE TRIGGER IF NOT EXISTS prodex_provider_descriptors_pricing_revision_update
BEFORE UPDATE OF tenant_id, pricing_revision ON prodex_provider_descriptors
WHEN NOT EXISTS (
    SELECT 1 FROM prodex_pricing_revisions
    WHERE tenant_id = NEW.tenant_id AND revision_id = NEW.pricing_revision
)
BEGIN
    SELECT RAISE(ABORT, 'provider pricing revision does not exist');
END;
"#,
};

pub const LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION: SqliteMigration = SqliteMigration {
    version: SqliteMigrationVersion(4),
    phase: SqliteMigrationPhase::Expand,
    name: "004_local_governance_lifecycle",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_governance_revision_artifacts (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    artifact_kind TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    compiled_artifact BLOB NOT NULL,
    created_by TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, artifact_kind, revision_id),
    UNIQUE (tenant_id, artifact_kind, artifact_checksum),
    CHECK (artifact_kind IN ('policy', 'classification_rules', 'provider_registry', 'routing_scores')),
    CHECK (length(revision_id) BETWEEN 1 AND 128),
    CHECK (length(artifact_checksum) BETWEEN 1 AND 128),
    CHECK (length(compiled_artifact) BETWEEN 1 AND 1048576),
    CHECK (created_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_classification_rule_pointers (
    tenant_id TEXT PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    active_revision_id TEXT,
    last_known_good_revision_id TEXT,
    etag TEXT NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL,
    CHECK (length(etag) BETWEEN 1 AND 128),
    CHECK (updated_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_provider_registry_pointers (
    tenant_id TEXT PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    active_revision_id TEXT,
    last_known_good_revision_id TEXT,
    etag TEXT NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL,
    CHECK (length(etag) BETWEEN 1 AND 128),
    CHECK (updated_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_routing_score_pointers (
    tenant_id TEXT PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    active_revision_id TEXT,
    last_known_good_revision_id TEXT,
    etag TEXT NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL,
    CHECK (length(etag) BETWEEN 1 AND 128),
    CHECK (updated_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_governance_activation_history (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    activation_id TEXT NOT NULL,
    artifact_kind TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    previous_revision_id TEXT,
    action TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    occurred_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, activation_id),
    UNIQUE (tenant_id, artifact_kind, idempotency_key),
    CHECK (artifact_kind IN ('classification_rules', 'provider_registry', 'routing_scores')),
    CHECK (action IN ('activate', 'rollback')),
    CHECK (length(revision_id) BETWEEN 1 AND 128),
    CHECK (length(idempotency_key) BETWEEN 1 AND 256),
    CHECK (occurred_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_governance_mutation_idempotency (
    tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
    artifact_kind TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    action TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    resulting_etag TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, artifact_kind, idempotency_key),
    CHECK (artifact_kind IN ('policy', 'classification_rules', 'provider_registry', 'routing_scores')),
    CHECK (action IN ('activate', 'rollback')),
    CHECK (length(idempotency_key) BETWEEN 1 AND 256),
    CHECK (length(request_fingerprint) BETWEEN 1 AND 256),
    CHECK (length(revision_id) BETWEEN 1 AND 128),
    CHECK (length(resulting_etag) BETWEEN 1 AND 128),
    CHECK (created_at_unix_ms >= 0)
);

CREATE TRIGGER IF NOT EXISTS prodex_governance_revision_artifacts_immutable_update
BEFORE UPDATE ON prodex_governance_revision_artifacts
BEGIN
    SELECT RAISE(ABORT, 'governance revision artifacts are immutable');
END;

CREATE TRIGGER IF NOT EXISTS prodex_governance_revision_artifacts_immutable_delete
BEFORE DELETE ON prodex_governance_revision_artifacts
BEGIN
    SELECT RAISE(ABORT, 'governance revision artifacts are immutable');
END;

CREATE TRIGGER IF NOT EXISTS prodex_policy_revision_content_immutable
BEFORE UPDATE OF artifact_checksum, compiled_metadata, created_by, created_at_unix_ms
ON prodex_policy_revisions
BEGIN
    SELECT RAISE(ABORT, 'governance revision content is immutable');
END;

CREATE TRIGGER IF NOT EXISTS prodex_classification_revision_content_immutable
BEFORE UPDATE OF artifact_checksum, compiled_metadata, created_at_unix_ms
ON prodex_classification_rule_revisions
BEGIN
    SELECT RAISE(ABORT, 'governance revision content is immutable');
END;

CREATE TRIGGER IF NOT EXISTS prodex_provider_registry_revision_content_immutable
BEFORE UPDATE OF artifact_checksum, created_at_unix_ms
ON prodex_provider_registry_revisions
BEGIN
    SELECT RAISE(ABORT, 'governance revision content is immutable');
END;

CREATE TRIGGER IF NOT EXISTS prodex_routing_score_revision_content_immutable
BEFORE UPDATE OF artifact_checksum, fixed_point_weights, created_at_unix_ms
ON prodex_routing_score_revisions
BEGIN
    SELECT RAISE(ABORT, 'governance revision content is immutable');
END;
"#,
};

pub const LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION: SqliteMigration = SqliteMigration {
    version: SqliteMigrationVersion(5),
    phase: SqliteMigrationPhase::Expand,
    name: "005_governance_session_indexes",
    sql: r#"
CREATE INDEX IF NOT EXISTS prodex_governance_sessions_principal_active_idx
    ON prodex_governance_sessions (
        tenant_id, principal_id, absolute_expires_at_unix_ms,
        idle_expires_at_unix_ms, session_id_hash
    );
CREATE INDEX IF NOT EXISTS prodex_governance_sessions_refresh_idx
    ON prodex_governance_sessions (tenant_id, last_seen_at_unix_ms DESC, session_id_hash);
"#,
};
