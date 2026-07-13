use prodex_storage_sqlite::{
    INITIAL_LOCAL_ACCOUNTING_MIGRATION, LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION,
    LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION, LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION,
    LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION,
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
fn sqlite_governance_session_indexes_bound_background_refresh_and_admission() {
    let sql = LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION.sql;
    assert!(sql.contains("prodex_governance_sessions_principal_active_idx"));
    assert!(sql.contains("prodex_governance_sessions_refresh_idx"));
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
