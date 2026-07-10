use postgres::NoTls;
use prodex_storage_postgres::INITIAL_TENANT_ACCOUNTING_MIGRATION;

#[test]
fn migration_creates_only_missing_rls_policies() {
    let sql = INITIAL_TENANT_ACCOUNTING_MIGRATION.sql;
    assert!(sql.contains("FROM pg_policies"));
    assert!(sql.contains("policyname = tenant_table || '_tenant_isolation'"));
}

#[test]
fn postgres_migration_can_be_applied_twice_without_duplicate_rls_policies() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };
    let mut client = postgres::Client::connect(&url, NoTls).expect("postgres should connect");
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .expect("postgres schema should reset");
    client
        .batch_execute(INITIAL_TENANT_ACCOUNTING_MIGRATION.sql)
        .expect("first migration should apply");
    client
        .batch_execute(INITIAL_TENANT_ACCOUNTING_MIGRATION.sql)
        .expect("second migration should apply");

    let policy_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_policies
             WHERE schemaname = current_schema()
               AND policyname LIKE 'prodex_%_tenant_isolation'",
            &[],
        )
        .expect("RLS policy count should load")
        .get(0);
    assert_eq!(policy_count, 12);
}
