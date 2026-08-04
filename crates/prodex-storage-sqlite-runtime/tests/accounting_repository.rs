use std::path::Path;
use std::sync::{Arc, Barrier};

use prodex_domain::{
    BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey, ReservationId,
    ReservationReconciliationReason, ReservationRequest, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{AtomicReservationCommand, TenantStorageKey, UsageReconciliationCommand};
use prodex_storage_sqlite::SQLITE_MIGRATIONS;
use prodex_storage_sqlite_runtime::{
    SqliteAccountingRepository, SqliteAccountingRepositoryError, SqliteIdempotentWriteOutcome,
    SqliteReserveOutcome,
};
use rusqlite::{Connection, params};

fn current_schema(path: &Path, tenant_id: TenantId) {
    let connection = Connection::open(path).expect("SQLite database should open");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("foreign keys should enable");
    for migration in SQLITE_MIGRATIONS {
        connection
            .execute_batch(migration.sql)
            .expect("SQLite migration should apply");
    }
    connection
        .execute(
            "INSERT INTO prodex_tenants
             (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'tenant', 1, 1)",
            params![tenant_id.to_string()],
        )
        .expect("tenant should insert");
}

fn reservation_command(tenant_id: TenantId) -> AtomicReservationCommand {
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    AtomicReservationCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
        idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(1_000, 10_000),
        request: ReservationRequest {
            tenant_id,
            call_id,
            reservation_id,
            estimate: UsageAmount::new(25, 250),
        },
        created_at_unix_ms: 1_000,
        ttl_ms: 60_000,
    }
}

#[test]
fn reusable_sqlite_reservation_and_reconciliation_are_exact_and_idempotent() {
    let root = std::env::temp_dir().join(format!("prodex-sqlite-accounting-{}", TenantId::new()));
    std::fs::create_dir_all(&root).expect("test root should be created");
    let path = root.join("state.sqlite");
    let tenant_id = TenantId::new();
    current_schema(&path, tenant_id);
    let command = reservation_command(tenant_id);
    let mut repository = SqliteAccountingRepository::open(&path).unwrap();
    let record = match repository.reserve(command.clone()).unwrap() {
        SqliteReserveOutcome::Reserved(record) => record,
        outcome => panic!("expected initial reservation, got {outcome:?}"),
    };
    assert!(matches!(
        repository.reserve(command.clone()).unwrap(),
        SqliteReserveOutcome::Replayed(_)
    ));

    let reconciliation = UsageReconciliationCommand {
        storage_key: command.storage_key,
        snapshot: BudgetSnapshot {
            reserved: record.reserved,
            committed: UsageAmount::ZERO,
        },
        record,
        actual: UsageAmount::new(20, 200),
        reason: ReservationReconciliationReason::Completed,
    };
    assert_eq!(
        repository.reconcile_usage(reconciliation.clone(), 2_000),
        Ok(SqliteIdempotentWriteOutcome::Applied)
    );
    assert_eq!(
        repository.reconcile_usage(reconciliation.clone(), 2_000),
        Ok(SqliteIdempotentWriteOutcome::Replayed)
    );
    let mut mismatch = reconciliation;
    mismatch.actual = UsageAmount::new(19, 190);
    assert_eq!(
        repository.reconcile_usage(mismatch, 2_000),
        Err(SqliteAccountingRepositoryError::StateConflict)
    );

    let connection = Connection::open(&path).unwrap();
    let state: (i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT reserved_tokens, reserved_cost_micros, committed_tokens,
                    committed_cost_micros,
                    (SELECT COUNT(*) FROM prodex_usage_ledger)
             FROM prodex_budget_counters",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(state, (0, 0, 20, 200, 3));
    drop(connection);
    drop(repository);
    std::fs::remove_dir_all(root).expect("test root should clean up");
}

#[test]
fn reusable_sqlite_exact_replay_reserves_once_concurrently() {
    let root =
        std::env::temp_dir().join(format!("prodex-sqlite-accounting-race-{}", TenantId::new()));
    std::fs::create_dir_all(&root).expect("test root should be created");
    let path = root.join("state.sqlite");
    let tenant_id = TenantId::new();
    current_schema(&path, tenant_id);
    let command = reservation_command(tenant_id);
    let barrier = Arc::new(Barrier::new(8));
    let results = std::thread::scope(|scope| {
        let handles = (0..8)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let path = path.clone();
                let command = command.clone();
                scope.spawn(move || {
                    let mut repository = SqliteAccountingRepository::open(&path).unwrap();
                    barrier.wait();
                    repository.reserve(command)
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        results
            .iter()
            .filter(|outcome| matches!(outcome, SqliteReserveOutcome::Reserved(_)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|outcome| matches!(outcome, SqliteReserveOutcome::Replayed(_)))
            .count(),
        7
    );
    let connection = Connection::open(&path).unwrap();
    let counts: (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM prodex_reservations),
                    (SELECT COUNT(*) FROM prodex_usage_ledger)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 1));
    drop(connection);
    std::fs::remove_dir_all(root).expect("test root should clean up");
}
