use deadpool_postgres::Pool;
use prodex_domain::{
    BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey, ReservationId,
    ReservationReconciliationReason, ReservationRequest, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{
    AtomicReservationCommand, BudgetStorageScope, ExpiredReservationRecoveryCommand,
    TenantStorageKey, UsageReconciliationCommand,
};
use prodex_storage_postgres::{SET_TENANT_STATEMENT, UPSERT_TENANT_LIFECYCLE_STATEMENT};
use prodex_storage_postgres_runtime::{
    IdempotentWriteOutcome, PostgresRepository, PostgresRuntimeConfig, ReserveOutcome,
    ReserveRejection,
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

fn grouped_request_command(
    tenant_id: TenantId,
    scope: BudgetStorageScope,
) -> AtomicReservationCommand {
    let mut command = reservation_command(tenant_id);
    command.storage_key = TenantStorageKey::budget_group(tenant_id, VirtualKeyId::new(), scope);
    command.limit = BudgetLimit::new(1_000, 10_000).with_max_requests(1);
    command
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

    let abandoned = reservation_command(tenant_id);
    let abandoned_record = match repository_one.reserve(abandoned.clone()).await.unwrap() {
        ReserveOutcome::Reserved(record) => record,
        outcome => panic!("unexpected abandoned reservation outcome: {outcome:?}"),
    };
    let recovery = ExpiredReservationRecoveryCommand {
        storage_key: abandoned.storage_key,
        snapshot: BudgetSnapshot {
            reserved: abandoned.request.estimate,
            committed: UsageAmount::new(40, 400),
        },
        record: abandoned_record,
        now_unix_ms: abandoned_record.expires_at_unix_ms,
    };
    assert_eq!(
        repository_one
            .release_expired(recovery.clone())
            .await
            .unwrap(),
        IdempotentWriteOutcome::Applied
    );
    assert_eq!(
        repository_two.release_expired(recovery).await.unwrap(),
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
    assert_eq!(
        ledger_count, 5,
        "completed and abandoned reservations have one event per phase"
    );
    let counter = transaction
        .query_one(
            "SELECT COALESCE(SUM(reserved_tokens), 0)::BIGINT, \
                    COALESCE(SUM(reserved_cost_micros), 0)::BIGINT, \
                    COALESCE(SUM(committed_tokens), 0)::BIGINT, \
                    COALESCE(SUM(committed_cost_micros), 0)::BIGINT \
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_repositories_enforce_one_grouped_request_atomically() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!(
            "skipping two_repositories_enforce_one_grouped_request_atomically: \
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
    let scope = BudgetStorageScope::from_digest([7; 32]);
    let first = grouped_request_command(tenant_id, scope);
    let second = grouped_request_command(tenant_id, scope);

    let (first, second) = tokio::join!(
        repository_one.reserve(first),
        repository_two.reserve(second)
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
            .filter(|outcome| matches!(
                outcome,
                ReserveOutcome::Rejected(ReserveRejection::RequestBudgetExceeded)
            ))
            .count(),
        1
    );

    let mut client = pool_one
        .get()
        .await
        .expect("verification pool should connect");
    let transaction = client
        .transaction()
        .await
        .expect("transaction should start");
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .expect("tenant context should be set");
    let row = transaction
        .query_one(
            "SELECT request_count, \
                    (SELECT COUNT(*) FROM prodex_reservations WHERE tenant_id = $1), \
                    (SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = $1) \
             FROM prodex_budget_counters WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .expect("grouped request counter should exist");
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, i64>(1), 1);
    assert_eq!(row.get::<_, i64>(2), 1);
    transaction.commit().await.unwrap();
}
