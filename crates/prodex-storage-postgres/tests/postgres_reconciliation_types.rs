use prodex_storage_postgres::{RECONCILE_USAGE_STATEMENT, RECOVER_EXPIRED_RESERVATION_STATEMENT};

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

#[test]
fn reconciliation_binds_the_stored_reservation_identity() {
    let sql = RECONCILE_USAGE_STATEMENT.sql;

    for predicate in [
        "virtual_key_id IS NOT DISTINCT FROM $14",
        "storage_scope = $9",
        "reserved_tokens = $4",
        "reserved_cost_micros = $5",
        "created_at_unix_ms = $15",
        "expires_at_unix_ms = $16",
        "counter.storage_scope = reservation.storage_scope",
    ] {
        assert!(
            sql.contains(predicate),
            "missing reconciliation predicate: {predicate}"
        );
    }
}

#[test]
fn expired_recovery_binds_the_stored_reservation_identity() {
    let sql = RECOVER_EXPIRED_RESERVATION_STATEMENT.sql;

    for predicate in [
        "virtual_key_id IS NOT DISTINCT FROM $9",
        "storage_scope = $7",
        "reserved_tokens = $5::BIGINT",
        "reserved_cost_micros = $6::BIGINT",
        "created_at_unix_ms = $10",
        "expires_at_unix_ms = $11",
        "counter.storage_scope = reservation.storage_scope",
    ] {
        assert!(
            sql.contains(predicate),
            "missing recovery predicate: {predicate}"
        );
    }
}
