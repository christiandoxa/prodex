use super::*;
use prodex_domain::{BudgetLimit, BudgetSnapshot, ReservationRequest};
use std::time::Duration;

#[test]
fn config_rejects_empty_url_and_zero_pool_size() {
    assert_eq!(
        PostgresRuntimeConfig::new("", 4),
        Err(PostgresRuntimeError::Configuration)
    );
    assert_eq!(
        PostgresRuntimeConfig::new("postgres://secret@example/db", 0),
        Err(PostgresRuntimeError::Configuration)
    );
    assert_eq!(
        PostgresRuntimeConfig::new("not a postgres connection string", 4),
        Err(PostgresRuntimeError::Configuration)
    );
    assert_eq!(
        PostgresRuntimeConfig::new("postgres://example/db", 4)
            .unwrap()
            .with_operation_timeout(Duration::ZERO),
        Err(PostgresRuntimeError::Configuration)
    );
}

#[test]
fn config_and_errors_are_redacted() {
    let password = "do-not-print-this-password";
    let config = PostgresRuntimeConfig::new(
        format!("postgres://prodex:{password}@db.internal/prodex"),
        8,
    )
    .unwrap();
    assert!(!format!("{config:?}").contains(password));
    assert!(!format!("{config:?}").contains("db.internal"));

    for error in [
        PostgresRuntimeError::Configuration,
        PostgresRuntimeError::PoolBuild,
        PostgresRuntimeError::PoolUnavailable,
        PostgresRuntimeError::Timeout,
        PostgresRuntimeError::Planning,
        PostgresRuntimeError::NumericOverflow,
        PostgresRuntimeError::Database,
        PostgresRuntimeError::InvalidDatabaseState,
        PostgresRuntimeError::StateConflict,
    ] {
        assert!(!format!("{error:?}").contains(password));
        assert!(!error.to_string().contains(password));
    }
}

#[test]
fn conversions_reject_values_outside_postgres_bigint() {
    assert_eq!(to_i64(i64::MAX as u64), Ok(i64::MAX));
    assert_eq!(
        to_i64(i64::MAX as u64 + 1),
        Err(PostgresRuntimeError::NumericOverflow)
    );
    assert_eq!(
        from_i64(-1),
        Err(PostgresRuntimeError::InvalidDatabaseState)
    );
}

#[test]
fn reserve_argument_conversion_checks_every_u64() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    let mut command = AtomicReservationCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(u64::MAX, 10),
        request: ReservationRequest {
            tenant_id,
            call_id,
            reservation_id,
            estimate: UsageAmount::new(1, 1),
        },
        created_at_unix_ms: 1,
        ttl_ms: 1,
    };
    assert!(matches!(
        ReserveArguments::try_from_command(&command),
        Err(PostgresRuntimeError::NumericOverflow)
    ));

    command.limit = BudgetLimit::new(10, 10);
    command.created_at_unix_ms = i64::MAX as u64;
    command.ttl_ms = 1;
    assert!(matches!(
        ReserveArguments::try_from_command(&command),
        Err(PostgresRuntimeError::NumericOverflow)
    ));
}
