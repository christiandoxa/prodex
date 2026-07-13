use postgres::NoTls;
use prodex_storage_postgres::{
    APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT, ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION,
    ENTERPRISE_GOVERNANCE_MIGRATION, GOVERNANCE_LIFECYCLE_MIGRATION,
    GOVERNANCE_SESSION_INDEX_MIGRATION, INITIAL_TENANT_ACCOUNTING_MIGRATION, POSTGRES_MIGRATIONS,
    SIEM_OUTBOX_LEASING_MIGRATION, postgres_governance_pointer_statements,
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
    let forced_governance_table_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_class
             WHERE relnamespace = current_schema()::regnamespace
               AND relname = ANY($1)
               AND relforcerowsecurity",
            &[&&[
                "prodex_policy_revisions",
                "prodex_policy_pointers",
                "prodex_policy_activation_history",
                "prodex_approvals",
                "prodex_approval_votes",
                "prodex_classification_rule_revisions",
                "prodex_provider_registry_revisions",
                "prodex_pricing_revisions",
                "prodex_provider_descriptors",
                "prodex_routing_score_revisions",
                "prodex_governance_sessions",
                "prodex_session_revocations",
                "prodex_siem_outbox",
                "prodex_siem_dead_letters",
                "prodex_governance_revision_artifacts",
                "prodex_classification_rule_pointers",
                "prodex_provider_registry_pointers",
                "prodex_routing_score_pointers",
                "prodex_governance_activation_history",
                "prodex_governance_mutation_idempotency",
            ][..]],
        )
        .expect("forced RLS table count should load")
        .get(0);
    assert_eq!(forced_governance_table_count, 20);
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
