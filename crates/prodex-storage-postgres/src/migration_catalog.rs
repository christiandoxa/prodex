use crate::{
    CONFIG_PUBLICATION_TRANSPORT_MIGRATION, INITIAL_TENANT_ACCOUNTING_MIGRATION,
    PostgresBackendOpenMode, PostgresBackendOpenPlan, PostgresMigration, PostgresMigrationPhase,
    PostgresMigrationPlan, PostgresMigrationVersion, PostgresRuntimeMode, PostgresStoragePlanError,
    REQUIRED_POSTGRES_SCHEMA_VERSION,
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

pub const ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(4),
    phase: PostgresMigrationPhase::Expand,
    name: "004_enterprise_governance_hardening",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_pricing_revisions (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    pricing_metadata JSONB NOT NULL,
    lifecycle_state TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, revision_id),
    UNIQUE (tenant_id, artifact_checksum),
    CHECK (char_length(revision_id) BETWEEN 1 AND 128),
    CHECK (char_length(artifact_checksum) BETWEEN 1 AND 128),
    CHECK (char_length(lifecycle_state) BETWEEN 1 AND 32),
    CHECK (octet_length(pricing_metadata::text) <= 1048576),
    CHECK (created_at_unix_ms >= 0)
);

DO $migration$
DECLARE
    target_table REGCLASS;
    constraint_name TEXT;
    constraint_sql TEXT;
BEGIN
    FOR target_table, constraint_name, constraint_sql IN
        SELECT table_name::regclass, name, expression
        FROM (VALUES
            ('prodex_policy_pointers', 'prodex_policy_pointers_active_revision_fk',
             'FOREIGN KEY (tenant_id, active_revision_id) REFERENCES prodex_policy_revisions(tenant_id, revision_id) NOT VALID'),
            ('prodex_policy_pointers', 'prodex_policy_pointers_lkg_revision_fk',
             'FOREIGN KEY (tenant_id, last_known_good_revision_id) REFERENCES prodex_policy_revisions(tenant_id, revision_id) NOT VALID'),
            ('prodex_policy_activation_history', 'prodex_policy_activation_revision_fk',
             'FOREIGN KEY (tenant_id, revision_id) REFERENCES prodex_policy_revisions(tenant_id, revision_id) NOT VALID'),
            ('prodex_policy_activation_history', 'prodex_policy_activation_previous_revision_fk',
             'FOREIGN KEY (tenant_id, previous_revision_id) REFERENCES prodex_policy_revisions(tenant_id, revision_id) NOT VALID'),
            ('prodex_provider_descriptors', 'prodex_provider_descriptors_pricing_revision_fk',
             'FOREIGN KEY (tenant_id, pricing_revision) REFERENCES prodex_pricing_revisions(tenant_id, revision_id) NOT VALID'),
            ('prodex_governance_sessions', 'prodex_governance_sessions_policy_revision_fk',
             'FOREIGN KEY (tenant_id, policy_revision_id) REFERENCES prodex_policy_revisions(tenant_id, revision_id) NOT VALID'),
            ('prodex_governance_sessions', 'prodex_governance_sessions_registry_revision_fk',
             'FOREIGN KEY (tenant_id, registry_revision_id) REFERENCES prodex_provider_registry_revisions(tenant_id, revision_id) NOT VALID'),
            ('prodex_session_revocations', 'prodex_session_revocations_session_fk',
             'FOREIGN KEY (tenant_id, session_id_hash) REFERENCES prodex_governance_sessions(tenant_id, session_id_hash) NOT VALID'),
            ('prodex_siem_outbox', 'prodex_siem_outbox_audit_event_fk',
             'FOREIGN KEY (tenant_id, audit_event_id) REFERENCES prodex_audit_log(tenant_id, audit_event_id) NOT VALID'),
            ('prodex_siem_dead_letters', 'prodex_siem_dead_letters_audit_event_fk',
             'FOREIGN KEY (tenant_id, audit_event_id) REFERENCES prodex_audit_log(tenant_id, audit_event_id) NOT VALID')
        ) AS constraints(table_name, name, expression)
    LOOP
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint
            WHERE conrelid = target_table AND conname = constraint_name
        ) THEN
            EXECUTE format(
                'ALTER TABLE %s ADD CONSTRAINT %I %s',
                target_table,
                constraint_name,
                constraint_sql
            );
        END IF;
    END LOOP;

    FOR target_table, constraint_name, constraint_sql IN
        SELECT table_name::regclass, name, expression
        FROM (VALUES
            ('prodex_policy_revisions', 'prodex_policy_revisions_bounded',
             'char_length(artifact_checksum) BETWEEN 1 AND 128 AND char_length(lifecycle_state) BETWEEN 1 AND 32 AND octet_length(compiled_metadata::text) <= 1048576 AND created_at_unix_ms >= 0'),
            ('prodex_policy_pointers', 'prodex_policy_pointers_bounded',
             'char_length(etag) BETWEEN 1 AND 256 AND updated_at_unix_ms >= 0'),
            ('prodex_policy_activation_history', 'prodex_policy_activation_history_bounded',
             'occurred_at_unix_ms >= 0'),
            ('prodex_approvals', 'prodex_approvals_bounded',
             'char_length(approval_id) BETWEEN 1 AND 128 AND char_length(approval_kind) BETWEEN 1 AND 64 AND char_length(approval_scope) BETWEEN 1 AND 256 AND char_length(fingerprint) BETWEEN 1 AND 128 AND char_length(lifecycle_state) BETWEEN 1 AND 32 AND expires_at_unix_ms >= 0 AND (activated_at_unix_ms IS NULL OR activated_at_unix_ms >= 0)'),
            ('prodex_classification_rule_revisions', 'prodex_classification_rule_revisions_bounded',
             'char_length(revision_id) BETWEEN 1 AND 128 AND char_length(artifact_checksum) BETWEEN 1 AND 128 AND char_length(lifecycle_state) BETWEEN 1 AND 32 AND octet_length(compiled_metadata::text) <= 1048576 AND created_at_unix_ms >= 0'),
            ('prodex_provider_registry_revisions', 'prodex_provider_registry_revisions_bounded',
             'char_length(revision_id) BETWEEN 1 AND 128 AND char_length(artifact_checksum) BETWEEN 1 AND 128 AND char_length(lifecycle_state) BETWEEN 1 AND 32 AND created_at_unix_ms >= 0'),
            ('prodex_provider_descriptors', 'prodex_provider_descriptors_bounded',
             'char_length(registry_revision_id) BETWEEN 1 AND 128 AND char_length(provider_id) BETWEEN 1 AND 128 AND char_length(adapter_kind) BETWEEN 1 AND 64 AND char_length(lifecycle_state) BETWEEN 1 AND 32 AND char_length(trust_tier) BETWEEN 1 AND 32 AND char_length(deployment_type) BETWEEN 1 AND 32 AND cardinality(approved_regions) <= 32 AND octet_length(capability_metadata::text) <= 1048576 AND octet_length(data_handling_metadata::text) <= 1048576 AND char_length(pricing_revision) BETWEEN 1 AND 128 AND char_length(secret_provider) BETWEEN 1 AND 128 AND char_length(secret_name) BETWEEN 1 AND 256 AND (secret_version IS NULL OR char_length(secret_version) BETWEEN 1 AND 128)'),
            ('prodex_routing_score_revisions', 'prodex_routing_score_revisions_bounded',
             'char_length(revision_id) BETWEEN 1 AND 128 AND char_length(artifact_checksum) BETWEEN 1 AND 128 AND char_length(lifecycle_state) BETWEEN 1 AND 32 AND octet_length(fixed_point_weights::text) <= 1048576 AND created_at_unix_ms >= 0'),
            ('prodex_governance_sessions', 'prodex_governance_sessions_bounded',
             'char_length(session_id_hash) BETWEEN 1 AND 128 AND char_length(channel) BETWEEN 1 AND 64 AND char_length(credential_scope) BETWEEN 1 AND 256 AND (provider_affinity IS NULL OR char_length(provider_affinity) BETWEEN 1 AND 128) AND created_at_unix_ms >= 0 AND last_seen_at_unix_ms >= created_at_unix_ms AND idle_expires_at_unix_ms >= last_seen_at_unix_ms AND absolute_expires_at_unix_ms >= created_at_unix_ms'),
            ('prodex_session_revocations', 'prodex_session_revocations_bounded',
             'char_length(reason_code) BETWEEN 1 AND 64 AND revoked_at_unix_ms >= 0'),
            ('prodex_siem_outbox', 'prodex_siem_outbox_bounded',
             'octet_length(event_envelope::text) <= 1048576 AND next_attempt_at_unix_ms >= 0 AND created_at_unix_ms >= 0 AND (delivered_at_unix_ms IS NULL OR delivered_at_unix_ms >= created_at_unix_ms)'),
            ('prodex_siem_dead_letters', 'prodex_siem_dead_letters_bounded',
             'octet_length(event_envelope::text) <= 1048576 AND char_length(stable_reason_code) BETWEEN 1 AND 64 AND failed_at_unix_ms >= 0')
        ) AS constraints(table_name, name, expression)
    LOOP
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint
            WHERE conrelid = target_table AND conname = constraint_name
        ) THEN
            EXECUTE format(
                'ALTER TABLE %s ADD CONSTRAINT %I CHECK (%s) NOT VALID',
                target_table,
                constraint_name,
                constraint_sql
            );
        END IF;
    END LOOP;
END $migration$;

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
        'prodex_pricing_revisions',
        'prodex_provider_descriptors',
        'prodex_routing_score_revisions',
        'prodex_governance_sessions',
        'prodex_session_revocations',
        'prodex_siem_outbox',
        'prodex_siem_dead_letters'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', tenant_table);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', tenant_table);
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

pub const GOVERNANCE_LIFECYCLE_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(5),
    phase: PostgresMigrationPhase::Expand,
    name: "005_governance_lifecycle",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_governance_revision_artifacts (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    artifact_kind TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    compiled_artifact BYTEA NOT NULL,
    created_by UUID NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, artifact_kind, revision_id),
    UNIQUE (tenant_id, artifact_kind, artifact_checksum),
    CHECK (artifact_kind IN ('policy', 'classification_rules', 'provider_registry', 'routing_scores')),
    CHECK (char_length(revision_id) BETWEEN 1 AND 128),
    CHECK (char_length(artifact_checksum) BETWEEN 1 AND 128),
    CHECK (octet_length(compiled_artifact) BETWEEN 1 AND 1048576),
    CHECK (created_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_classification_rule_pointers (
    tenant_id UUID PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    active_revision_id TEXT,
    last_known_good_revision_id TEXT,
    etag TEXT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    CHECK (char_length(etag) BETWEEN 1 AND 128),
    CHECK (updated_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_provider_registry_pointers (
    tenant_id UUID PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    active_revision_id TEXT,
    last_known_good_revision_id TEXT,
    etag TEXT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    CHECK (char_length(etag) BETWEEN 1 AND 128),
    CHECK (updated_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_routing_score_pointers (
    tenant_id UUID PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    active_revision_id TEXT,
    last_known_good_revision_id TEXT,
    etag TEXT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    CHECK (char_length(etag) BETWEEN 1 AND 128),
    CHECK (updated_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_governance_activation_history (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    activation_id UUID NOT NULL,
    artifact_kind TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    previous_revision_id TEXT,
    action TEXT NOT NULL,
    actor_id UUID NOT NULL,
    idempotency_key TEXT NOT NULL,
    occurred_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, activation_id),
    UNIQUE (tenant_id, artifact_kind, idempotency_key),
    CHECK (artifact_kind IN ('classification_rules', 'provider_registry', 'routing_scores')),
    CHECK (action IN ('activate', 'rollback')),
    CHECK (char_length(revision_id) BETWEEN 1 AND 128),
    CHECK (char_length(idempotency_key) BETWEEN 1 AND 256),
    CHECK (occurred_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_governance_mutation_idempotency (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    artifact_kind TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    action TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    resulting_etag TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, artifact_kind, idempotency_key),
    CHECK (artifact_kind IN ('policy', 'classification_rules', 'provider_registry', 'routing_scores')),
    CHECK (action IN ('activate', 'rollback')),
    CHECK (char_length(idempotency_key) BETWEEN 1 AND 256),
    CHECK (char_length(request_fingerprint) BETWEEN 1 AND 256),
    CHECK (char_length(revision_id) BETWEEN 1 AND 128),
    CHECK (char_length(resulting_etag) BETWEEN 1 AND 128),
    CHECK (created_at_unix_ms >= 0)
);

CREATE OR REPLACE FUNCTION prodex_reject_governance_revision_mutation()
RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    RAISE EXCEPTION 'governance revision content is immutable';
END $function$;

DO $migration$
DECLARE trigger_spec RECORD;
BEGIN
    FOR trigger_spec IN
        SELECT * FROM (VALUES
            ('prodex_governance_revision_artifacts', 'prodex_governance_revision_artifacts_immutable', 'UPDATE OR DELETE', NULL),
            ('prodex_policy_revisions', 'prodex_policy_revision_content_immutable', 'UPDATE', 'artifact_checksum, compiled_metadata, created_by, created_at_unix_ms'),
            ('prodex_classification_rule_revisions', 'prodex_classification_revision_content_immutable', 'UPDATE', 'artifact_checksum, compiled_metadata, created_at_unix_ms'),
            ('prodex_provider_registry_revisions', 'prodex_provider_registry_revision_content_immutable', 'UPDATE', 'artifact_checksum, created_at_unix_ms'),
            ('prodex_routing_score_revisions', 'prodex_routing_score_revision_content_immutable', 'UPDATE', 'artifact_checksum, fixed_point_weights, created_at_unix_ms')
        ) AS specs(table_name, trigger_name, event_kind, columns)
    LOOP
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgrelid = trigger_spec.table_name::regclass
              AND tgname = trigger_spec.trigger_name
              AND NOT tgisinternal
        ) THEN
            IF trigger_spec.columns IS NULL THEN
                EXECUTE format(
                    'CREATE TRIGGER %I BEFORE %s ON %I FOR EACH ROW EXECUTE FUNCTION prodex_reject_governance_revision_mutation()',
                    trigger_spec.trigger_name,
                    trigger_spec.event_kind,
                    trigger_spec.table_name
                );
            ELSE
                EXECUTE format(
                    'CREATE TRIGGER %I BEFORE %s OF %s ON %I FOR EACH ROW EXECUTE FUNCTION prodex_reject_governance_revision_mutation()',
                    trigger_spec.trigger_name,
                    trigger_spec.event_kind,
                    trigger_spec.columns,
                    trigger_spec.table_name
                );
            END IF;
        END IF;
    END LOOP;
END $migration$;

DO $migration$
DECLARE tenant_table TEXT;
BEGIN
    FOREACH tenant_table IN ARRAY ARRAY[
        'prodex_governance_revision_artifacts',
        'prodex_classification_rule_pointers',
        'prodex_provider_registry_pointers',
        'prodex_routing_score_pointers',
        'prodex_governance_activation_history',
        'prodex_governance_mutation_idempotency'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', tenant_table);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', tenant_table);
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

pub const SIEM_OUTBOX_LEASING_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(6),
    phase: PostgresMigrationPhase::Expand,
    name: "006_siem_outbox_leasing",
    sql: r#"
ALTER TABLE prodex_siem_outbox
    ADD COLUMN IF NOT EXISTS claim_token UUID,
    ADD COLUMN IF NOT EXISTS claim_expires_at_unix_ms BIGINT;

ALTER TABLE prodex_siem_outbox
    DROP CONSTRAINT IF EXISTS prodex_siem_outbox_claim_pair;
ALTER TABLE prodex_siem_outbox
    ADD CONSTRAINT prodex_siem_outbox_claim_pair CHECK (
        (claim_token IS NULL AND claim_expires_at_unix_ms IS NULL)
        OR (claim_token IS NOT NULL AND claim_expires_at_unix_ms IS NOT NULL)
    ) NOT VALID;

CREATE INDEX IF NOT EXISTS prodex_siem_outbox_due_claim_idx
    ON prodex_siem_outbox (
        tenant_id, delivered_at_unix_ms, next_attempt_at_unix_ms,
        claim_expires_at_unix_ms, event_id
    );
"#,
};

pub const GOVERNANCE_SESSION_INDEX_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(7),
    phase: PostgresMigrationPhase::Expand,
    name: "007_governance_session_indexes",
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

pub const TENANT_RLS_AND_AUDIT_IMMUTABILITY_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(8),
    phase: PostgresMigrationPhase::Expand,
    name: "008_tenant_rls_and_audit_immutability",
    sql: r#"
DO $migration$
DECLARE tenant_table TEXT;
BEGIN
    FOR tenant_table IN
        SELECT tablename
        FROM pg_policies
        WHERE schemaname = current_schema()
          AND policyname = tablename || '_tenant_isolation'
    LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', tenant_table);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', tenant_table);
    END LOOP;
END $migration$;

CREATE OR REPLACE FUNCTION prodex_reject_audit_mutation()
RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    RAISE EXCEPTION 'audit events are immutable';
END $function$;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgrelid = 'prodex_audit_log'::regclass
          AND tgname = 'prodex_audit_log_immutable'
          AND NOT tgisinternal
    ) THEN
        CREATE TRIGGER prodex_audit_log_immutable
        BEFORE UPDATE OR DELETE ON prodex_audit_log
        FOR EACH ROW EXECUTE FUNCTION prodex_reject_audit_mutation();
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgrelid = 'prodex_audit_log'::regclass
          AND tgname = 'prodex_audit_log_no_truncate'
          AND NOT tgisinternal
    ) THEN
        CREATE TRIGGER prodex_audit_log_no_truncate
        BEFORE TRUNCATE ON prodex_audit_log
        FOR EACH STATEMENT EXECUTE FUNCTION prodex_reject_audit_mutation();
    END IF;
END $migration$;

REVOKE UPDATE, DELETE, TRUNCATE ON prodex_audit_log FROM PUBLIC;
"#,
};

pub const GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(9),
    phase: PostgresMigrationPhase::Expand,
    name: "009_governance_session_provider_revisions",
    sql: r#"
ALTER TABLE prodex_governance_sessions
    ADD COLUMN IF NOT EXISTS provider_descriptor_revision BIGINT NOT NULL DEFAULT 0;

DO $migration$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = current_schema()
          AND table_name = 'prodex_governance_sessions'
          AND column_name = 'registry_revision_id'
    ) THEN
        UPDATE prodex_governance_sessions
        SET provider_descriptor_revision = split_part(registry_revision_id, ':', 2)::BIGINT
        WHERE registry_revision_id ~ '^[0-9]+:[0-9]+$';

        ALTER TABLE prodex_governance_sessions
            RENAME COLUMN registry_revision_id TO provider_registry_revision;
    END IF;
END $migration$;

-- Split only rows whose registry half satisfies the existing tenant-scoped FK.
UPDATE prodex_governance_sessions session
SET provider_registry_revision = split_part(session.provider_registry_revision, ':', 1)
WHERE session.provider_registry_revision ~ '^[0-9]+:[0-9]+$'
  AND EXISTS (
      SELECT 1 FROM prodex_provider_registry_revisions registry
      WHERE registry.tenant_id = session.tenant_id
        AND registry.revision_id = split_part(session.provider_registry_revision, ':', 1)
  );

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prodex_governance_sessions'::regclass
          AND conname = 'prodex_governance_sessions_provider_descriptor_revision_check'
    ) THEN
        ALTER TABLE prodex_governance_sessions
            ADD CONSTRAINT prodex_governance_sessions_provider_descriptor_revision_check
            CHECK (provider_descriptor_revision >= 0) NOT VALID;
    END IF;
END $migration$;

-- Legacy packed-row backfill above can be removed after every deployment has crossed schema v9.
"#,
};

pub const APPROVAL_TERMINATION_REASON_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(10),
    phase: PostgresMigrationPhase::Expand,
    name: "010_approval_termination_reason",
    sql: r#"
ALTER TABLE prodex_approvals
    ADD COLUMN IF NOT EXISTS termination_reason TEXT;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prodex_approvals'::regclass
          AND conname = 'prodex_approvals_termination_reason_bounded'
    ) THEN
        ALTER TABLE prodex_approvals
            ADD CONSTRAINT prodex_approvals_termination_reason_bounded
            CHECK (termination_reason IS NULL OR char_length(termination_reason) BETWEEN 1 AND 128)
            NOT VALID;
    END IF;
END $migration$;
"#,
};

pub const SESSION_REVOCATION_EPOCH_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(11),
    phase: PostgresMigrationPhase::Expand,
    name: "011_session_revocation_epoch",
    sql: r#"
ALTER TABLE prodex_tenants
    ADD COLUMN IF NOT EXISTS session_revocation_epoch BIGINT NOT NULL DEFAULT 0;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prodex_tenants'::regclass
          AND conname = 'prodex_tenants_session_revocation_epoch_nonnegative'
    ) THEN
        ALTER TABLE prodex_tenants
            ADD CONSTRAINT prodex_tenants_session_revocation_epoch_nonnegative
            CHECK (session_revocation_epoch >= 0) NOT VALID;
    END IF;
END $migration$;
"#,
};

pub const RESERVATION_STORAGE_SCOPE_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(12),
    phase: PostgresMigrationPhase::Expand,
    name: "012_reservation_storage_scope",
    sql: r#"
ALTER TABLE prodex_reservations
    ADD COLUMN IF NOT EXISTS storage_scope TEXT NOT NULL DEFAULT 'tenant-default';

UPDATE prodex_reservations reservation
SET storage_scope = COALESCE(
    (
        SELECT counter.storage_scope
        FROM prodex_budget_counters counter
        WHERE counter.tenant_id = reservation.tenant_id
          AND counter.virtual_key_id IS NOT DISTINCT FROM reservation.virtual_key_id
        ORDER BY counter.updated_at_unix_ms DESC, counter.storage_scope
        LIMIT 1
    ),
    CASE
        WHEN reservation.virtual_key_id IS NULL THEN 'tenant-default'
        ELSE 'virtual_key:' || reservation.virtual_key_id::TEXT
    END
);

CREATE INDEX IF NOT EXISTS prodex_reservations_expired_active_idx
    ON prodex_reservations (expires_at_unix_ms, tenant_id)
    WHERE committed_at_unix_ms IS NULL AND released_at_unix_ms IS NULL;
"#,
};

pub const AUDIT_LEGAL_HOLD_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(13),
    phase: PostgresMigrationPhase::Expand,
    name: "013_audit_legal_holds",
    sql: r#"
CREATE TABLE IF NOT EXISTS prodex_audit_legal_holds (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    audit_event_id UUID NOT NULL,
    reason_code TEXT NOT NULL,
    expires_at_unix_ms BIGINT,
    created_by UUID NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, audit_event_id),
    FOREIGN KEY (tenant_id, audit_event_id)
        REFERENCES prodex_audit_log(tenant_id, audit_event_id),
    CHECK (char_length(reason_code) BETWEEN 1 AND 128),
    CHECK (expires_at_unix_ms IS NULL OR expires_at_unix_ms > 0),
    CHECK (created_at_unix_ms >= 0)
);

ALTER TABLE prodex_audit_legal_holds ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_audit_legal_holds FORCE ROW LEVEL SECURITY;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies
        WHERE schemaname = current_schema()
          AND tablename = 'prodex_audit_legal_holds'
          AND policyname = 'prodex_audit_legal_holds_tenant_isolation'
    ) THEN
        CREATE POLICY prodex_audit_legal_holds_tenant_isolation
            ON prodex_audit_legal_holds
            USING (tenant_id = current_setting('prodex.tenant_id', true)::uuid)
            WITH CHECK (tenant_id = current_setting('prodex.tenant_id', true)::uuid);
    END IF;
END $migration$;

CREATE INDEX IF NOT EXISTS prodex_audit_legal_holds_active_idx
    ON prodex_audit_legal_holds (tenant_id, expires_at_unix_ms, audit_event_id);

CREATE TABLE IF NOT EXISTS prodex_audit_retention_anchors (
    tenant_id UUID PRIMARY KEY REFERENCES prodex_tenants(tenant_id),
    last_purged_digest TEXT NOT NULL,
    updated_at_unix_ms BIGINT NOT NULL,
    CHECK (char_length(last_purged_digest) BETWEEN 1 AND 128),
    CHECK (updated_at_unix_ms >= 0)
);

ALTER TABLE prodex_audit_retention_anchors ENABLE ROW LEVEL SECURITY;
ALTER TABLE prodex_audit_retention_anchors FORCE ROW LEVEL SECURITY;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies
        WHERE schemaname = current_schema()
          AND tablename = 'prodex_audit_retention_anchors'
          AND policyname = 'prodex_audit_retention_anchors_tenant_isolation'
    ) THEN
        CREATE POLICY prodex_audit_retention_anchors_tenant_isolation
            ON prodex_audit_retention_anchors
            USING (tenant_id = current_setting('prodex.tenant_id', true)::uuid)
            WITH CHECK (tenant_id = current_setting('prodex.tenant_id', true)::uuid);
    END IF;
END $migration$;

CREATE OR REPLACE FUNCTION prodex_reject_audit_mutation()
RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    IF TG_OP = 'DELETE'
       AND current_setting('prodex.audit_retention_purge_tenant', true) = OLD.tenant_id::text
    THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'audit events are immutable';
END $function$;
"#,
};

pub const POSTGRES_MIGRATIONS: &[PostgresMigration] = &[
    INITIAL_TENANT_ACCOUNTING_MIGRATION,
    GROUPED_REQUEST_BUDGET_MIGRATION,
    ENTERPRISE_GOVERNANCE_MIGRATION,
    ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION,
    GOVERNANCE_LIFECYCLE_MIGRATION,
    SIEM_OUTBOX_LEASING_MIGRATION,
    GOVERNANCE_SESSION_INDEX_MIGRATION,
    TENANT_RLS_AND_AUDIT_IMMUTABILITY_MIGRATION,
    GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION,
    APPROVAL_TERMINATION_REASON_MIGRATION,
    SESSION_REVOCATION_EPOCH_MIGRATION,
    RESERVATION_STORAGE_SCOPE_MIGRATION,
    AUDIT_LEGAL_HOLD_MIGRATION,
    CONFIG_PUBLICATION_TRANSPORT_MIGRATION,
];
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
