use prodex_storage_postgres::RECONCILE_USAGE_STATEMENT;

#[test]
fn reconciliation_release_parameters_are_postgres_bigints() {
    assert_eq!(
        RECONCILE_USAGE_STATEMENT
            .sql
            .matches("::BIGINT > 0")
            .count(),
        4
    );
}
