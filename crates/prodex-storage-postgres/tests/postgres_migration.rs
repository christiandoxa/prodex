use postgres::NoTls;
use postgres::fallible_iterator::FallibleIterator;
use prodex_storage_postgres::{
    APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT, AUDIT_REASON_DETAIL_BYTE_LIMIT_MIGRATION,
    AUDIT_REASON_DETAIL_MIGRATION, ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION,
    ENTERPRISE_GOVERNANCE_MIGRATION, GOVERNANCE_INVALIDATION_OUTBOX_MIGRATION,
    GOVERNANCE_LIFECYCLE_MIGRATION, GOVERNANCE_REVOCATION_MIGRATION,
    GOVERNANCE_SESSION_INDEX_MIGRATION, GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION,
    INITIAL_TENANT_ACCOUNTING_MIGRATION, POSTGRES_MIGRATIONS, PostgresMigrationPhase,
    PostgresMigrationVersion, REQUIRED_POSTGRES_SCHEMA_VERSION,
    RESERVATION_STORAGE_SCOPE_MIGRATION, SIEM_OUTBOX_LEASING_MIGRATION,
    TENANT_RLS_AND_AUDIT_IMMUTABILITY_MIGRATION, VALIDATE_DEFERRED_CONSTRAINTS_MIGRATION,
    postgres_governance_pointer_statements,
};
use std::time::Duration;

#[test]
fn migration_creates_only_missing_rls_policies() {
    let sql = INITIAL_TENANT_ACCOUNTING_MIGRATION.sql;
    assert!(sql.contains("FROM pg_policies"));
    assert!(sql.contains("policyname = tenant_table || '_tenant_isolation'"));
}

#[test]
fn reservation_storage_scope_backfill_fails_closed_on_ambiguous_counters() {
    let sql = RESERVATION_STORAGE_SCOPE_MIGRATION.sql;
    assert!(sql.contains("multiple budget counters match"));
    assert!(sql.contains("SELECT COUNT(*)"));
    assert!(sql.contains("IS NOT DISTINCT FROM"));
    assert!(sql.contains("virtual_key:"));
    assert!(!sql.contains("ORDER BY counter.updated_at_unix_ms"));
    assert!(!sql.contains("LIMIT 1"));
}

#[test]
fn postgres_validation_migration_is_append_only_and_complete() {
    let migration = VALIDATE_DEFERRED_CONSTRAINTS_MIGRATION;
    assert_eq!(migration.version, PostgresMigrationVersion(18));
    assert_eq!(migration.phase, PostgresMigrationPhase::Validate);
    assert_eq!(POSTGRES_MIGRATIONS.get(17), Some(&migration));
    assert_eq!(
        POSTGRES_MIGRATIONS
            .iter()
            .map(|migration| migration.version.0)
            .collect::<Vec<_>>(),
        (1_u32..=REQUIRED_POSTGRES_SCHEMA_VERSION.0).collect::<Vec<_>>()
    );
    assert!(!migration.sql.contains("NOT VALID"));
    let sql = migration.sql.replace("\r\n", "\n");
    for (table, constraint) in [
        (
            "prodex_policy_pointers",
            "prodex_policy_pointers_active_revision_fk",
        ),
        (
            "prodex_policy_pointers",
            "prodex_policy_pointers_lkg_revision_fk",
        ),
        (
            "prodex_policy_activation_history",
            "prodex_policy_activation_revision_fk",
        ),
        (
            "prodex_policy_activation_history",
            "prodex_policy_activation_previous_revision_fk",
        ),
        (
            "prodex_provider_descriptors",
            "prodex_provider_descriptors_pricing_revision_fk",
        ),
        (
            "prodex_governance_sessions",
            "prodex_governance_sessions_policy_revision_fk",
        ),
        (
            "prodex_governance_sessions",
            "prodex_governance_sessions_registry_revision_fk",
        ),
        (
            "prodex_session_revocations",
            "prodex_session_revocations_session_fk",
        ),
        ("prodex_siem_outbox", "prodex_siem_outbox_audit_event_fk"),
        (
            "prodex_siem_dead_letters",
            "prodex_siem_dead_letters_audit_event_fk",
        ),
        ("prodex_policy_revisions", "prodex_policy_revisions_bounded"),
        ("prodex_policy_pointers", "prodex_policy_pointers_bounded"),
        (
            "prodex_policy_activation_history",
            "prodex_policy_activation_history_bounded",
        ),
        ("prodex_approvals", "prodex_approvals_bounded"),
        (
            "prodex_classification_rule_revisions",
            "prodex_classification_rule_revisions_bounded",
        ),
        (
            "prodex_provider_registry_revisions",
            "prodex_provider_registry_revisions_bounded",
        ),
        (
            "prodex_provider_descriptors",
            "prodex_provider_descriptors_bounded",
        ),
        (
            "prodex_routing_score_revisions",
            "prodex_routing_score_revisions_bounded",
        ),
        (
            "prodex_governance_sessions",
            "prodex_governance_sessions_bounded",
        ),
        (
            "prodex_session_revocations",
            "prodex_session_revocations_bounded",
        ),
        ("prodex_siem_outbox", "prodex_siem_outbox_bounded"),
        (
            "prodex_siem_dead_letters",
            "prodex_siem_dead_letters_bounded",
        ),
        ("prodex_siem_outbox", "prodex_siem_outbox_claim_pair"),
        (
            "prodex_governance_sessions",
            "prodex_governance_sessions_provider_descriptor_revision_check",
        ),
        (
            "prodex_approvals",
            "prodex_approvals_termination_reason_bounded",
        ),
        (
            "prodex_tenants",
            "prodex_tenants_session_revocation_epoch_nonnegative",
        ),
        (
            "prodex_governance_revision_artifacts",
            "prodex_governance_artifact_signature_pair",
        ),
        (
            "prodex_policy_revisions",
            "prodex_policy_revisions_lifecycle_state_check",
        ),
        (
            "prodex_policy_activation_history",
            "prodex_policy_activation_history_action_check",
        ),
        (
            "prodex_governance_activation_history",
            "prodex_governance_activation_history_action_check",
        ),
        (
            "prodex_governance_mutation_idempotency",
            "prodex_governance_mutation_idempotency_action_check",
        ),
        (
            "prodex_governance_activation_history",
            "prodex_governance_activation_history_result_ids_bounded",
        ),
        (
            "prodex_governance_mutation_idempotency",
            "prodex_governance_mutation_idempotency_result_ids_bounded",
        ),
    ] {
        assert!(
            sql.contains(&format!(
                "ALTER TABLE {table}\n    VALIDATE CONSTRAINT {constraint};"
            )),
            "missing validation for {table}.{constraint}"
        );
    }
}

#[test]
fn postgres_audit_reason_detail_migration_is_backward_compatible() {
    assert_eq!(
        AUDIT_REASON_DETAIL_MIGRATION.version,
        PostgresMigrationVersion(19)
    );
    assert_eq!(
        AUDIT_REASON_DETAIL_MIGRATION.phase,
        PostgresMigrationPhase::Expand
    );
    assert_eq!(
        AUDIT_REASON_DETAIL_BYTE_LIMIT_MIGRATION.version,
        PostgresMigrationVersion(20)
    );
    assert_eq!(
        AUDIT_REASON_DETAIL_BYTE_LIMIT_MIGRATION.version,
        REQUIRED_POSTGRES_SCHEMA_VERSION
    );
    assert_eq!(
        POSTGRES_MIGRATIONS.last(),
        Some(&AUDIT_REASON_DETAIL_BYTE_LIMIT_MIGRATION)
    );
    assert!(
        AUDIT_REASON_DETAIL_MIGRATION
            .sql
            .contains("ADD COLUMN IF NOT EXISTS reason_detail TEXT")
    );
    assert!(
        AUDIT_REASON_DETAIL_BYTE_LIMIT_MIGRATION
            .sql
            .contains("octet_length(reason_detail) <= 512")
    );
    assert!(
        AUDIT_REASON_DETAIL_BYTE_LIMIT_MIGRATION
            .sql
            .contains("prodex_audit_log_reason_detail_bounded")
    );
    assert!(AUDIT_REASON_DETAIL_BYTE_LIMIT_MIGRATION.sql.contains(
        "ALTER TABLE prodex_audit_log\n    VALIDATE CONSTRAINT prodex_audit_log_reason_detail_bounded;"
    ));
}

#[test]
fn governance_revocation_is_terminal_and_notifies_every_pointer_family() {
    let sql = GOVERNANCE_REVOCATION_MIGRATION.sql;
    assert_eq!(
        prodex_storage::GOVERNANCE_INVALIDATION_CHANNEL,
        "prodex_governance_invalidation"
    );
    assert_eq!(
        prodex_storage::MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES,
        256
    );
    assert!(sql.contains("prodex_reject_revoked_governance_revision_revival"));
    assert!(sql.contains("prodex_notify_governance_invalidation"));
    assert!(sql.contains("pg_notify('prodex_governance_invalidation', payload)"));
    assert!(sql.contains("octet_length(payload) > 256"));
    for (table, kind) in [
        ("prodex_policy_pointers", "policy"),
        (
            "prodex_classification_rule_pointers",
            "classification_rules",
        ),
        ("prodex_provider_registry_pointers", "provider_registry"),
        ("prodex_routing_score_pointers", "routing_scores"),
    ] {
        assert!(sql.contains(table), "missing notify trigger table {table}");
        assert!(sql.contains(kind), "missing notify payload kind {kind}");
    }
}

#[test]
fn governance_invalidation_outbox_is_bounded_tenant_scoped_and_transactional() {
    let sql = GOVERNANCE_INVALIDATION_OUTBOX_MIGRATION.sql;
    for table in [
        "prodex_governance_invalidation_outbox",
        "prodex_governance_invalidation_replicas",
        "prodex_governance_invalidation_acks",
    ] {
        assert!(sql.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")));
        assert!(sql.contains(table));
    }
    assert!(sql.contains("INSERT INTO prodex_governance_invalidation_outbox"));
    assert!(sql.contains("event_id BIGINT GENERATED ALWAYS AS IDENTITY"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, event_id)"));
    assert!(sql.contains("FOREIGN KEY (tenant_id, replica_id)"));
    assert!(sql.contains("FOREIGN KEY (tenant_id, event_id)"));
    assert!(sql.contains("ALTER TABLE %I FORCE ROW LEVEL SECURITY"));
    assert!(sql.contains("current_setting(''prodex.tenant_id'', true)::uuid"));
    assert!(sql.contains("WITH CHECK"));
    assert!(sql.contains("char_length(replica_id) BETWEEN 1 AND 128"));
    assert!(sql.contains("pg_notify('prodex_governance_invalidation', payload)"));
    assert!(sql.contains("octet_length(payload) > 256"));
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
        assert!(
            statements
                .compare_and_swap
                .sql
                .contains("VALUES ($1, $2, $3, $4, $5)")
        );
        assert!(
            !statements
                .compare_and_swap
                .sql
                .contains("WHERE $6::text IS NULL")
        );
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
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
fn postgres_migrations_can_be_applied_twice_without_duplicate_rls_policies() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let mut client = postgres::Client::connect(&url, NoTls).expect("postgres should connect");
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .expect("postgres schema should reset");
    for migration in POSTGRES_MIGRATIONS {
        client
            .batch_execute(migration.sql)
            .expect("migration should apply");
    }
    let first_policy_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_policies
             WHERE schemaname = current_schema()
               AND policyname LIKE 'prodex_%_tenant_isolation'",
            &[],
        )
        .expect("RLS policy count should load")
        .get(0);
    for migration in POSTGRES_MIGRATIONS {
        client
            .batch_execute(migration.sql)
            .expect("migration should apply idempotently");
    }
    let unvalidated_constraint_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_constraint
             WHERE connamespace = current_schema()::regnamespace
               AND NOT convalidated",
            &[],
        )
        .expect("constraint validation state should load")
        .get(0);
    assert_eq!(unvalidated_constraint_count, 0);
    let policy_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_policies
             WHERE schemaname = current_schema()
               AND policyname LIKE 'prodex_%_tenant_isolation'",
            &[],
        )
        .expect("RLS policy count should load")
        .get(0);
    assert!(first_policy_count > 0);
    assert_eq!(policy_count, first_policy_count);
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
    assert_eq!(forced_tenant_table_count, policy_count);
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

#[test]
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
fn governance_invalidation_notification_is_delivered_only_after_commit() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let mut writer = postgres::Client::connect(&url, NoTls).expect("postgres should connect");
    writer
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .expect("postgres schema should reset");
    for migration in POSTGRES_MIGRATIONS {
        writer
            .batch_execute(migration.sql)
            .expect("migration should apply");
    }
    let tenant_id: prodex_domain::TenantId =
        "00000000-0000-7000-8000-000000000051".parse().unwrap();
    writer
        .execute(
            "INSERT INTO prodex_tenants (
                tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms
             ) VALUES ($1::uuid, 'Notification Test', 1, 1)",
            &[&tenant_id.as_uuid()],
        )
        .expect("tenant should insert");
    writer
        .query_one(
            "SELECT set_config('prodex.tenant_id', $1, false)",
            &[&tenant_id.to_string()],
        )
        .expect("tenant context should set");

    let mut listener = postgres::Client::connect(&url, NoTls).expect("listener should connect");
    listener
        .batch_execute(&format!(
            "LISTEN {}",
            prodex_storage::GOVERNANCE_INVALIDATION_CHANNEL
        ))
        .expect("listener should subscribe");

    let mut rollback = writer
        .transaction()
        .expect("rollback transaction should open");
    rollback
        .execute(
            "INSERT INTO prodex_routing_score_pointers (
                tenant_id, active_revision_id, last_known_good_revision_id,
                etag, updated_at_unix_ms
             ) VALUES ($1::uuid, NULL, NULL, 'etag-rollback', 1)",
            &[&tenant_id.as_uuid()],
        )
        .expect("rollback pointer should insert");
    assert_eq!(
        rollback
            .query_one(
                "SELECT COUNT(*) FROM prodex_governance_invalidation_outbox WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .expect("rollback outbox event should be visible in transaction")
            .get::<_, i64>(0),
        1
    );
    rollback
        .rollback()
        .expect("pointer transaction should roll back");
    assert_eq!(
        writer
            .query_one(
                "SELECT COUNT(*) FROM prodex_governance_invalidation_outbox WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .expect("rolled back outbox should be queryable")
            .get::<_, i64>(0),
        0
    );
    assert!(
        listener
            .notifications()
            .timeout_iter(Duration::from_millis(100))
            .next()
            .expect("rollback notification wait should succeed")
            .is_none(),
        "rolled back pointer must not notify"
    );

    let mut transaction = writer.transaction().expect("transaction should open");
    transaction
        .execute(
            "INSERT INTO prodex_routing_score_pointers (
                tenant_id, active_revision_id, last_known_good_revision_id,
                etag, updated_at_unix_ms
             ) VALUES ($1::uuid, NULL, NULL, 'etag-1', 1)",
            &[&tenant_id.as_uuid()],
        )
        .expect("pointer should insert");
    let queued: i64 = transaction
        .query_one(
            "SELECT COUNT(*) FROM prodex_governance_invalidation_outbox WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .expect("outbox event should be in the pointer transaction")
        .get(0);
    assert_eq!(queued, 1);
    assert!(
        listener
            .notifications()
            .timeout_iter(Duration::from_millis(100))
            .next()
            .expect("notification wait should succeed")
            .is_none(),
        "notification escaped before transaction commit"
    );
    transaction.commit().expect("transaction should commit");

    let notification = listener
        .notifications()
        .timeout_iter(Duration::from_secs(2))
        .next()
        .expect("notification wait should succeed")
        .expect("committed pointer should notify");
    assert_eq!(
        notification.channel(),
        prodex_storage::GOVERNANCE_INVALIDATION_CHANNEL
    );
    assert!(
        notification.payload().len() <= prodex_storage::MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES
    );
    assert_eq!(
        notification
            .payload()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>(),
        format!(r#"{{"tenant_id":"{tenant_id}","kind":"routing_scores"}}"#)
    );
}
