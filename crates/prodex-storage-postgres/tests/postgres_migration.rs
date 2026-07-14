use postgres::NoTls;
use prodex_storage_postgres::{
    APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT, ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION,
    ENTERPRISE_GOVERNANCE_MIGRATION, GOVERNANCE_LIFECYCLE_MIGRATION,
    GOVERNANCE_SESSION_INDEX_MIGRATION, GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION,
    INITIAL_TENANT_ACCOUNTING_MIGRATION, POSTGRES_MIGRATIONS, SIEM_OUTBOX_LEASING_MIGRATION,
    TENANT_RLS_AND_AUDIT_IMMUTABILITY_MIGRATION, postgres_governance_pointer_statements,
};

#[test]
fn migration_creates_only_missing_rls_policies() {
    let sql = INITIAL_TENANT_ACCOUNTING_MIGRATION.sql;
    assert!(sql.contains("FROM pg_policies"));
    assert!(sql.contains("policyname = tenant_table || '_tenant_isolation'"));
}

#[test]
fn governance_hardening_forces_rls_and_adds_bounded_pricing_revisions() {
    let sql = ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION.sql;
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_pricing_revisions"));
    assert!(sql.contains("ALTER TABLE %I FORCE ROW LEVEL SECURITY"));
    assert!(sql.contains("prodex_provider_descriptors_pricing_revision_fk"));
    assert!(sql.contains("prodex_governance_sessions_policy_revision_fk"));
    assert!(sql.contains("prodex_session_revocations_session_fk"));
    assert!(sql.contains("prodex_siem_outbox_audit_event_fk"));
    assert!(sql.contains("octet_length(event_envelope::text) <= 1048576"));
}

#[test]
fn governance_lifecycle_adds_immutable_artifacts_pointers_and_idempotency() {
    let sql = GOVERNANCE_LIFECYCLE_MIGRATION.sql;
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_governance_revision_artifacts"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_classification_rule_pointers"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_governance_mutation_idempotency"));
    assert!(sql.contains("prodex_reject_governance_revision_mutation"));
    assert!(sql.contains("ALTER TABLE %I FORCE ROW LEVEL SECURITY"));
}

#[test]
fn siem_outbox_leasing_is_bounded_and_reclaimable() {
    let sql = SIEM_OUTBOX_LEASING_MIGRATION.sql;
    assert!(sql.contains("claim_token UUID"));
    assert!(sql.contains("claim_expires_at_unix_ms BIGINT"));
    assert!(sql.contains("prodex_siem_outbox_claim_pair"));
    assert!(sql.contains("prodex_siem_outbox_due_claim_idx"));
}

#[test]
fn governance_session_indexes_bound_background_refresh_and_admission() {
    let sql = GOVERNANCE_SESSION_INDEX_MIGRATION.sql;
    assert!(sql.contains("prodex_governance_sessions_principal_active_idx"));
    assert!(sql.contains("prodex_governance_sessions_refresh_idx"));
}

#[test]
fn governance_session_provider_revisions_are_separate_and_backfilled() {
    let sql = GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION.sql;
    assert!(sql.contains("RENAME COLUMN registry_revision_id TO provider_registry_revision"));
    assert!(sql.contains("provider_descriptor_revision BIGINT"));
    assert!(sql.contains("split_part(registry_revision_id, ':', 2)"));
    assert!(sql.contains("prodex_governance_sessions_provider_descriptor_revision_check"));
}

#[test]
fn tenant_rls_and_audit_immutability_hardening_is_idempotent() {
    let sql = TENANT_RLS_AND_AUDIT_IMMUTABILITY_MIGRATION.sql;
    assert!(sql.contains("policyname = tablename || '_tenant_isolation'"));
    assert!(sql.contains("ALTER TABLE %I FORCE ROW LEVEL SECURITY"));
    assert!(sql.contains("BEFORE UPDATE OR DELETE ON prodex_audit_log"));
    assert!(sql.contains("BEFORE TRUNCATE ON prodex_audit_log"));
    assert!(sql.contains("REVOKE UPDATE, DELETE, TRUNCATE ON prodex_audit_log FROM PUBLIC"));
}

#[test]
fn governance_postgres_port_uses_atomic_audit_outbox_and_pointer_cas() {
    let audit = APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT.sql;
    assert!(audit.contains("WITH audit_insert AS"));
    assert!(audit.contains("INSERT INTO prodex_siem_outbox"));
    for kind in [
        prodex_storage::GovernanceArtifactKind::Policy,
        prodex_storage::GovernanceArtifactKind::ClassificationRules,
        prodex_storage::GovernanceArtifactKind::ProviderRegistry,
        prodex_storage::GovernanceArtifactKind::RoutingScores,
    ] {
        let statements = postgres_governance_pointer_statements(kind);
        assert!(statements.load.sql.contains("FOR UPDATE"));
        assert!(statements.compare_and_swap.sql.contains(".etag = $6"));
        assert!(statements.compare_and_swap.sql.contains("RETURNING etag"));
    }
}

#[test]
fn governance_migration_is_tenant_scoped_and_content_minimized() {
    let sql = ENTERPRISE_GOVERNANCE_MIGRATION.sql;
    for table in [
        "prodex_policy_revisions",
        "prodex_approvals",
        "prodex_provider_descriptors",
        "prodex_governance_sessions",
        "prodex_siem_outbox",
    ] {
        assert!(sql.contains(table), "missing governance table {table}");
    }
    assert!(sql.contains("ENABLE ROW LEVEL SECURITY"));
    assert!(sql.contains("current_setting(''prodex.tenant_id''"));
    for forbidden in [
        "raw_prompt",
        "raw_response",
        "provider_secret",
        "access_token",
    ] {
        assert!(!sql.contains(forbidden), "migration contains {forbidden}");
    }
}

#[test]
fn postgres_migrations_can_be_applied_twice_without_duplicate_rls_policies() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };
    let mut client = postgres::Client::connect(&url, NoTls).expect("postgres should connect");
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .expect("postgres schema should reset");
    for _ in 0..2 {
        for migration in POSTGRES_MIGRATIONS {
            client
                .batch_execute(migration.sql)
                .expect("migration should apply idempotently");
        }
    }

    let policy_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_policies
             WHERE schemaname = current_schema()
               AND policyname LIKE 'prodex_%_tenant_isolation'",
            &[],
        )
        .expect("RLS policy count should load")
        .get(0);
    assert_eq!(policy_count, 32);
    let forced_tenant_table_count: i64 = client
        .query_one(
            "SELECT COUNT(DISTINCT class.oid)
             FROM pg_class class
             JOIN pg_policies policy
               ON policy.schemaname = current_schema()
              AND policy.tablename = class.relname
              AND policy.policyname = class.relname || '_tenant_isolation'
             WHERE class.relnamespace = current_schema()::regnamespace
               AND class.relforcerowsecurity",
            &[],
        )
        .expect("forced RLS table count should load")
        .get(0);
    assert_eq!(forced_tenant_table_count, 32);
    let immutable_audit_trigger_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_trigger
             WHERE tgrelid = 'prodex_audit_log'::regclass
               AND tgname IN ('prodex_audit_log_immutable', 'prodex_audit_log_no_truncate')
               AND NOT tgisinternal",
            &[],
        )
        .expect("audit immutability triggers should load")
        .get(0);
    assert_eq!(immutable_audit_trigger_count, 2);

    client
        .batch_execute(
            "INSERT INTO prodex_tenants (
                 tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms
             ) VALUES ('00000000-0000-7000-8000-000000000001', 'Audit Test', 1, 1);
             INSERT INTO prodex_audit_log (
                 tenant_id, audit_event_id, previous_digest, event_digest,
                 occurred_at_unix_ms, principal_id, action, resource_kind,
                 resource_id, outcome, reason_code
             ) VALUES (
                 '00000000-0000-7000-8000-000000000001',
                 '00000000-0000-7000-8000-000000000002', NULL, 'sha256:audit-test', 1,
                 '00000000-0000-7000-8000-000000000003', 'test', 'test', NULL, 'success', NULL
             );",
        )
        .expect("audit fixture should insert");
    for error in [
        client
            .execute(
                "UPDATE prodex_audit_log SET outcome = 'changed'
                 WHERE audit_event_id = '00000000-0000-7000-8000-000000000002'",
                &[],
            )
            .unwrap_err(),
        client
            .execute(
                "DELETE FROM prodex_audit_log
                 WHERE audit_event_id = '00000000-0000-7000-8000-000000000002'",
                &[],
            )
            .unwrap_err(),
        client
            .batch_execute("TRUNCATE prodex_audit_log CASCADE")
            .unwrap_err(),
    ] {
        assert_eq!(
            error.as_db_error().map(postgres::error::DbError::message),
            Some("audit events are immutable")
        );
    }
    client
        .batch_execute(
            "INSERT INTO prodex_tenants VALUES
                 ('00000000-0000-7000-8000-000000000011', 'Session A', 1, 1),
                 ('00000000-0000-7000-8000-000000000012', 'Session B', 1, 1);
             INSERT INTO prodex_policy_revisions VALUES
                 ('00000000-0000-7000-8000-000000000011',
                  '00000000-0000-7000-8000-000000000021', 'policy-a', '{}', 'active',
                  '00000000-0000-7000-8000-000000000031', 1),
                 ('00000000-0000-7000-8000-000000000012',
                  '00000000-0000-7000-8000-000000000022', 'policy-b', '{}', 'active',
                  '00000000-0000-7000-8000-000000000032', 1);
             INSERT INTO prodex_provider_registry_revisions VALUES
                 ('00000000-0000-7000-8000-000000000011', '7', 'registry-a', 'active', 1);
             INSERT INTO prodex_governance_sessions (
                 tenant_id, session_id_hash, principal_id, channel, credential_scope,
                 classification, policy_revision_id, provider_registry_revision,
                 provider_descriptor_revision, provider_affinity, created_at_unix_ms,
                 last_seen_at_unix_ms, absolute_expires_at_unix_ms, idle_expires_at_unix_ms
             ) VALUES (
                 '00000000-0000-7000-8000-000000000011', repeat('a', 64),
                 '00000000-0000-7000-8000-000000000041', 'api', 'data_plane', 'internal',
                 '00000000-0000-7000-8000-000000000021', '7', 9, 'openai', 1, 1, 100, 100
             );",
        )
        .expect("separate provider revisions should satisfy tenant-scoped FKs");
    let revisions = client
        .query_one(
            "SELECT provider_registry_revision, provider_descriptor_revision
             FROM prodex_governance_sessions
             WHERE tenant_id = '00000000-0000-7000-8000-000000000011'",
            &[],
        )
        .expect("session provider revisions should round trip");
    assert_eq!(revisions.get::<_, String>(0), "7");
    assert_eq!(revisions.get::<_, i64>(1), 9);
    let cross_tenant = client
        .execute(
            "INSERT INTO prodex_governance_sessions (
                 tenant_id, session_id_hash, principal_id, channel, credential_scope,
                 classification, policy_revision_id, provider_registry_revision,
                 provider_descriptor_revision, provider_affinity, created_at_unix_ms,
                 last_seen_at_unix_ms, absolute_expires_at_unix_ms, idle_expires_at_unix_ms
             ) VALUES (
                 '00000000-0000-7000-8000-000000000012', repeat('b', 64),
                 '00000000-0000-7000-8000-000000000042', 'api', 'data_plane', 'internal',
                 '00000000-0000-7000-8000-000000000022', '7', 9, 'openai', 1, 1, 100, 100
             )",
            &[],
        )
        .unwrap_err();
    assert_eq!(
        cross_tenant.code(),
        Some(&postgres::error::SqlState::FOREIGN_KEY_VIOLATION)
    );
    let request_count_column: bool = client
        .query_one(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.columns
                WHERE table_schema = current_schema()
                  AND table_name = 'prodex_budget_counters'
                  AND column_name = 'request_count'
            )",
            &[],
        )
        .expect("request counter column should be inspectable")
        .get(0);
    assert!(request_count_column);
}
