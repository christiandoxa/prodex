use deadpool_postgres::Pool;
use prodex_domain::{
    BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey, ReservationId,
    ReservationReconciliationReason, ReservationRequest, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{AtomicReservationCommand, TenantStorageKey, UsageReconciliationCommand};
use prodex_storage_postgres::{SET_TENANT_STATEMENT, UPSERT_TENANT_LIFECYCLE_STATEMENT};
use prodex_storage_postgres_runtime::{
    IdempotentWriteOutcome, PostgresRepository, PostgresRuntimeConfig, ReserveOutcome,
};

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
            estimate: UsageAmount::new(100, 1_000),
        },
        created_at_unix_ms: 1_800_000_000_000,
        ttl_ms: 60_000,
    }
}

async fn create_tenant(pool: &Pool, tenant_id: TenantId) {
    let mut client = pool.get().await.expect("postgres pool should connect");
    let transaction = client
        .transaction()
        .await
        .expect("tenant setup transaction should start");
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .expect("tenant context should be set");
    transaction
        .query_one(
            UPSERT_TENANT_LIFECYCLE_STATEMENT.sql,
            &[
                &tenant_id.as_uuid(),
                &"runtime integration tenant",
                &1_800_000_000_000_i64,
            ],
        )
        .await
        .expect("migrated database should accept tenant setup");
    transaction
        .commit()
        .await
        .expect("tenant setup should commit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_repositories_reserve_and_reconcile_idempotently() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!(
            "skipping two_repositories_reserve_and_reconcile_idempotently: \
             PRODEX_TEST_POSTGRES_URL is not set"
        );
        return;
    };
    let config = PostgresRuntimeConfig::new(url, 4).expect("test config should be valid");
    let pool_one = config
        .create_pool_explicit_no_tls()
        .expect("first test pool should build");
    let pool_two = config
        .create_pool_explicit_no_tls()
        .expect("second test pool should build");
    let repository_one = PostgresRepository::from_pool_with_config(pool_one.clone(), &config);
    let repository_two = PostgresRepository::from_pool_with_config(pool_two, &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool_one, tenant_id).await;

    let command = reservation_command(tenant_id);
    let (first, second) = tokio::join!(
        repository_one.reserve(command.clone()),
        repository_two.reserve(command.clone())
    );
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ReserveOutcome::Reserved(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ReserveOutcome::Replayed(_)))
            .count(),
        1
    );

    let loaded = repository_two
        .load_reservation(tenant_id, command.request.call_id)
        .await
        .unwrap()
        .expect("reservation should load across repositories");
    let reconcile = UsageReconciliationCommand {
        storage_key: command.storage_key,
        snapshot: BudgetSnapshot {
            reserved: command.request.estimate,
            committed: UsageAmount::ZERO,
        },
        record: loaded.record,
        actual: UsageAmount::new(40, 400),
        reason: ReservationReconciliationReason::Completed,
    };
    assert_eq!(
        repository_one
            .reconcile_usage(reconcile.clone(), 1_800_000_001_000)
            .await
            .unwrap(),
        IdempotentWriteOutcome::Applied
    );
    assert_eq!(
        repository_two
            .reconcile_usage(reconcile, 1_800_000_001_001)
            .await
            .unwrap(),
        IdempotentWriteOutcome::Replayed
    );

    let mut client = pool_one
        .get()
        .await
        .expect("verification pool should connect");
    let transaction = client
        .transaction()
        .await
        .expect("verification transaction should start");
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .expect("verification tenant context should be set");
    let ledger_count: i64 = transaction
        .query_one(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(ledger_count, 3, "reserved, committed, and released only");
    let counter = transaction
        .query_one(
            "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, \
                    committed_cost_micros \
             FROM prodex_budget_counters \
             WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(counter.get::<_, i64>(0), 0);
    assert_eq!(counter.get::<_, i64>(1), 0);
    assert_eq!(counter.get::<_, i64>(2), 40);
    assert_eq!(counter.get::<_, i64>(3), 400);
    transaction.commit().await.unwrap();
}
