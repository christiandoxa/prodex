use prodex_storage_sqlite::{
    INITIAL_LOCAL_ACCOUNTING_MIGRATION, LOCAL_APPROVAL_TERMINATION_REASON_MIGRATION,
    LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION, LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION,
    LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION, LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION,
    LOCAL_GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION, LOCAL_SIEM_OUTBOX_LEASING_MIGRATION,
};

#[test]
fn sqlite_governance_migration_is_content_minimized_and_executable() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(INITIAL_LOCAL_ACCOUNTING_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_APPROVAL_TERMINATION_REASON_MIGRATION.sql)
        .unwrap();
    let sql = LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION.sql;
    for table in [
        "prodex_policy_revisions",
        "prodex_approvals",
        "prodex_provider_descriptors",
        "prodex_governance_sessions",
        "prodex_siem_outbox",
    ] {
        assert!(sql.contains(table), "missing governance table {table}");
    }
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
fn sqlite_siem_outbox_leasing_migration_preserves_existing_rows() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(INITIAL_LOCAL_ACCOUNTING_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION.sql)
        .unwrap();
    connection
        .execute(
            "INSERT INTO prodex_tenants VALUES ('tenant-a', 'A', 1, 1)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO prodex_siem_outbox (
                tenant_id, event_id, audit_event_id, event_envelope,
                attempt_count, next_attempt_at_unix_ms, created_at_unix_ms,
                delivered_at_unix_ms
             ) VALUES ('tenant-a', 'event-a', 'audit-a', '{}', 2, 10, 10, NULL)",
            [],
        )
        .unwrap();

    connection
        .execute_batch(LOCAL_SIEM_OUTBOX_LEASING_MIGRATION.sql)
        .unwrap();

    let existing = connection
        .query_row(
            "SELECT attempt_count, next_attempt_at_unix_ms, claim_token,
                    claim_expires_at_unix_ms
             FROM prodex_siem_outbox WHERE tenant_id = 'tenant-a' AND event_id = 'event-a'",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(existing, (2, 10, None, None));
    assert!(
        connection
            .execute(
                "UPDATE prodex_siem_outbox SET claim_token = 'token' \
             WHERE tenant_id = 'tenant-a' AND event_id = 'event-a'",
                [],
            )
            .is_err()
    );
    assert!(
        LOCAL_SIEM_OUTBOX_LEASING_MIGRATION
            .sql
            .contains("prodex_siem_outbox_due_claim_idx")
    );
    let indexed_columns = connection
        .prepare(
            "SELECT name FROM pragma_index_info('prodex_siem_outbox_due_claim_idx') ORDER BY seqno",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        indexed_columns,
        [
            "delivered_at_unix_ms",
            "next_attempt_at_unix_ms",
            "event_id",
            "claim_expires_at_unix_ms",
        ]
    );
}

#[test]
fn sqlite_governance_session_indexes_bound_background_refresh_and_admission() {
    let sql = LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION.sql;
    assert!(sql.contains("prodex_governance_sessions_principal_active_idx"));
    assert!(sql.contains("prodex_governance_sessions_refresh_idx"));
}

#[test]
fn sqlite_session_provider_revisions_backfill_and_reject_cross_tenant_registry() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    for migration in [
        INITIAL_LOCAL_ACCOUNTING_MIGRATION,
        LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION,
        LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION,
        LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION,
        LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION,
    ] {
        connection.execute_batch(migration.sql).unwrap();
    }
    connection
        .execute_batch(
            "INSERT INTO prodex_tenants VALUES ('tenant-a', 'A', 1, 1);
             INSERT INTO prodex_tenants VALUES ('tenant-b', 'B', 1, 1);
             INSERT INTO prodex_provider_registry_revisions VALUES
                 ('tenant-a', '7', 'checksum-7', 'active', 1);
             INSERT INTO prodex_governance_sessions VALUES (
                 'tenant-a', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                 'principal-a', 'api', 'data_plane', 'internal', 'policy-a', '7:9',
                 'openai', 1, 1, 100, 100
             );",
        )
        .unwrap();

    connection
        .execute_batch(LOCAL_GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION.sql)
        .unwrap();
    let revisions = connection
        .query_row(
            "SELECT provider_registry_revision, provider_descriptor_revision
             FROM prodex_governance_sessions WHERE tenant_id = 'tenant-a'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap();
    assert_eq!(revisions, ("7".to_string(), 9));

    let cross_tenant = connection.execute(
        "INSERT INTO prodex_governance_sessions (
             tenant_id, session_id_hash, principal_id, channel, credential_scope,
             classification, policy_revision_id, provider_registry_revision,
             provider_descriptor_revision, provider_affinity, created_at_unix_ms,
             last_seen_at_unix_ms, absolute_expires_at_unix_ms, idle_expires_at_unix_ms
         ) VALUES ('tenant-b', ?1, 'principal-b', 'api', 'data_plane', 'internal',
                   'policy-b', '7', 9, 'openai', 1, 1, 100, 100)",
        ["b".repeat(64)],
    );
    assert!(cross_tenant.is_err());
}

#[test]
fn sqlite_governance_lifecycle_has_immutable_artifact_authority() {
    let sql = LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION.sql;
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_governance_revision_artifacts"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_classification_rule_pointers"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_governance_mutation_idempotency"));
    assert!(sql.contains("prodex_governance_revision_artifacts_immutable_update"));
}

#[test]
fn sqlite_governance_hardening_requires_a_versioned_pricing_revision() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(INITIAL_LOCAL_ACCOUNTING_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION.sql)
        .unwrap();
    connection
        .execute_batch(LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION.sql)
        .unwrap();
    connection
        .execute(
            "INSERT INTO prodex_tenants VALUES (?1, 'tenant', 1, 1)",
            ["tenant-a"],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO prodex_provider_registry_revisions VALUES (
                'tenant-a', 'registry-1', 'registry-checksum', 'active', 1
            )",
            [],
        )
        .unwrap();
    let descriptor_sql = "INSERT INTO prodex_provider_descriptors VALUES (
        'tenant-a', 'registry-1', 'provider-1', 'adapter', 'active', 'trusted', 'hosted',
        '[\"region\"]', '{}', '{}', 0, 'pricing-1', 'projected', 'credential', NULL
    )";
    assert!(connection.execute(descriptor_sql, []).is_err());
    connection
        .execute(
            "INSERT INTO prodex_pricing_revisions VALUES (
                'tenant-a', 'pricing-1', 'checksum', '{}', 'active', 1
            )",
            [],
        )
        .unwrap();
    connection.execute(descriptor_sql, []).unwrap();
}
