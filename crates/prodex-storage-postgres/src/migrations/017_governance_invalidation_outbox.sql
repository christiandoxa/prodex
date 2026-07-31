
CREATE TABLE IF NOT EXISTS prodex_governance_invalidation_outbox (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    event_id BIGINT GENERATED ALWAYS AS IDENTITY,
    artifact_kind TEXT NOT NULL,
    created_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, event_id),
    CHECK (artifact_kind IN (
        'policy', 'classification_rules', 'provider_registry', 'routing_scores'
    )),
    CHECK (created_at_unix_ms >= 0)
);

CREATE TABLE IF NOT EXISTS prodex_governance_invalidation_replicas (
    tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
    replica_id TEXT NOT NULL,
    registered_at_unix_ms BIGINT NOT NULL,
    last_seen_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, replica_id),
    CHECK (char_length(replica_id) BETWEEN 1 AND 128),
    CHECK (replica_id ~ '^[A-Za-z0-9._-]+$'),
    CHECK (registered_at_unix_ms >= 0),
    CHECK (last_seen_at_unix_ms >= registered_at_unix_ms)
);

CREATE TABLE IF NOT EXISTS prodex_governance_invalidation_acks (
    tenant_id UUID NOT NULL,
    replica_id TEXT NOT NULL,
    event_id BIGINT NOT NULL,
    delivered_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, replica_id, event_id),
    FOREIGN KEY (tenant_id, replica_id)
        REFERENCES prodex_governance_invalidation_replicas(tenant_id, replica_id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, event_id)
        REFERENCES prodex_governance_invalidation_outbox(tenant_id, event_id)
        ON DELETE CASCADE,
    CHECK (delivered_at_unix_ms >= 0)
);

CREATE INDEX IF NOT EXISTS prodex_governance_invalidation_outbox_order_idx
    ON prodex_governance_invalidation_outbox (tenant_id, event_id);
CREATE INDEX IF NOT EXISTS prodex_governance_invalidation_acks_event_idx
    ON prodex_governance_invalidation_acks (tenant_id, event_id, replica_id);

DO $migration$
DECLARE tenant_table TEXT;
BEGIN
    FOREACH tenant_table IN ARRAY ARRAY[
        'prodex_governance_invalidation_outbox',
        'prodex_governance_invalidation_replicas',
        'prodex_governance_invalidation_acks'
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
                'CREATE POLICY %I ON %I
                 USING (tenant_id = current_setting(''prodex.tenant_id'', true)::uuid)
                 WITH CHECK (tenant_id = current_setting(''prodex.tenant_id'', true)::uuid)',
                tenant_table || '_tenant_isolation',
                tenant_table
            );
        END IF;
    END LOOP;
END $migration$;

CREATE OR REPLACE FUNCTION prodex_notify_governance_invalidation()
RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE payload TEXT;
BEGIN
    INSERT INTO prodex_governance_invalidation_outbox (
        tenant_id, artifact_kind, created_at_unix_ms
    ) VALUES (
        NEW.tenant_id,
        TG_ARGV[0],
        (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
    );
    payload := json_build_object(
        'tenant_id', NEW.tenant_id::text,
        'kind', TG_ARGV[0]
    )::text;
    IF octet_length(payload) > 256 THEN
        RAISE EXCEPTION 'governance invalidation payload exceeds bound'
            USING ERRCODE = 'program_limit_exceeded';
    END IF;
    PERFORM pg_notify('prodex_governance_invalidation', payload);
    RETURN NEW;
END $function$;

DO $migration$
DECLARE pointer_spec RECORD;
BEGIN
    FOR pointer_spec IN
        SELECT * FROM (VALUES
            ('prodex_policy_pointers', 'policy'),
            ('prodex_classification_rule_pointers', 'classification_rules'),
            ('prodex_provider_registry_pointers', 'provider_registry'),
            ('prodex_routing_score_pointers', 'routing_scores')
        ) AS specs(table_name, artifact_kind)
    LOOP
        EXECUTE format(
            'DROP TRIGGER IF EXISTS prodex_governance_invalidation_notify ON %I',
            pointer_spec.table_name
        );
        EXECUTE format(
            'CREATE TRIGGER prodex_governance_invalidation_notify
             AFTER INSERT OR UPDATE ON %I
             FOR EACH ROW EXECUTE FUNCTION prodex_notify_governance_invalidation(%L)',
            pointer_spec.table_name,
            pointer_spec.artifact_kind
        );
    END LOOP;
END $migration$;
