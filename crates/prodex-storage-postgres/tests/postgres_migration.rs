use postgres::NoTls;
use prodex_storage_postgres::{INITIAL_TENANT_ACCOUNTING_MIGRATION, POSTGRES_MIGRATIONS};

#[test]
fn migration_creates_only_missing_rls_policies() {
    let sql = INITIAL_TENANT_ACCOUNTING_MIGRATION.sql;
    assert!(sql.contains("FROM pg_policies"));
    assert!(sql.contains("policyname = tenant_table || '_tenant_isolation'"));
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
    assert_eq!(policy_count, 12);
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
