use prodex_storage_postgres::RECONCILE_USAGE_STATEMENT;

#[test]
fn reconciliation_release_parameters_are_postgres_bigints() {
    for parameter in ["$4::BIGINT", "$5::BIGINT"] {
        assert_eq!(
            RECONCILE_USAGE_STATEMENT.sql.matches(parameter).count(),
            2,
            "{parameter} must stay typed across released-reservation CASE expressions"
        );
    }
    assert_eq!(
        RECONCILE_USAGE_STATEMENT
            .sql
            .matches("::BIGINT > 0")
            .count(),
        4
    );
}
