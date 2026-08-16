use super::{PostgresRuntimeError, StoredReservation};
use super::{StoredReservationState, from_i64};
use deadpool_postgres::Transaction;
use prodex_domain::{
    CallId, IdempotencyKey, ReservationId, ReservationRecord, TenantId, UsageAmount, VirtualKeyId,
};
use tokio_postgres::Row;
use uuid::Uuid;

const LOAD_RESERVATION_BY_CALL_FOR_STORAGE_KEY_SQL: &str = r#"
SELECT
    reservation.tenant_id,
    reservation.reservation_id,
    reservation.call_id,
    reservation.virtual_key_id,
    reservation.idempotency_key,
    reservation.reserved_tokens,
    reservation.reserved_cost_micros,
    reservation.created_at_unix_ms,
    reservation.expires_at_unix_ms,
    reservation.committed_at_unix_ms,
    reservation.released_at_unix_ms,
    committed.tokens AS committed_tokens,
    committed.cost_micros AS committed_cost_micros,
    released.tokens AS released_tokens,
    released.cost_micros AS released_cost_micros
FROM prodex_reservations AS reservation
LEFT JOIN prodex_usage_ledger AS committed
    ON committed.tenant_id = reservation.tenant_id
   AND committed.reservation_id = reservation.reservation_id
   AND committed.event_kind = 'committed'
LEFT JOIN prodex_usage_ledger AS released
    ON released.tenant_id = reservation.tenant_id
   AND released.reservation_id = reservation.reservation_id
   AND released.event_kind = 'released'
WHERE reservation.tenant_id = $1
  AND reservation.call_id = $2
  AND reservation.storage_scope = $3
  AND reservation.virtual_key_id IS NOT DISTINCT FROM $4
"#;

pub(super) async fn load_by_call_for_storage_key_in_transaction(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    call_id: CallId,
    storage_scope: &str,
    virtual_key_id: Option<Uuid>,
) -> Result<Option<StoredReservation>, PostgresRuntimeError> {
    let statement = transaction
        .prepare_cached(LOAD_RESERVATION_BY_CALL_FOR_STORAGE_KEY_SQL)
        .await
        .map_err(|_| PostgresRuntimeError::Database)?;
    let row = transaction
        .query_opt(
            &statement,
            &[
                &tenant_id.as_uuid(),
                &call_id.as_uuid(),
                &storage_scope,
                &virtual_key_id,
            ],
        )
        .await
        .map_err(|_| PostgresRuntimeError::Database)?;
    row.map(stored_reservation_from_row).transpose()
}

pub(super) fn stored_reservation_from_row(
    row: Row,
) -> Result<StoredReservation, PostgresRuntimeError> {
    let tenant_id = TenantId::from_uuid(row.get::<_, Uuid>("tenant_id"));
    let reservation_id = ReservationId::from_uuid(row.get::<_, Uuid>("reservation_id"));
    let call_id = CallId::from_uuid(row.get::<_, Uuid>("call_id"));
    let virtual_key_id = row
        .get::<_, Option<Uuid>>("virtual_key_id")
        .map(VirtualKeyId::from_uuid);
    let idempotency_key = IdempotencyKey::new(row.get::<_, String>("idempotency_key"))
        .map_err(|_| PostgresRuntimeError::InvalidDatabaseState)?;
    let committed_at = row.get::<_, Option<i64>>("committed_at_unix_ms");
    let released_at = row.get::<_, Option<i64>>("released_at_unix_ms");
    let committed_usage =
        optional_usage_from_row(&row, "committed_tokens", "committed_cost_micros")?;
    let released_usage = optional_usage_from_row(&row, "released_tokens", "released_cost_micros")?;
    let state = if committed_at.is_some() {
        StoredReservationState::Committed
    } else if released_at.is_some() {
        StoredReservationState::Released
    } else {
        StoredReservationState::Active
    };
    Ok(StoredReservation {
        record: ReservationRecord {
            tenant_id,
            call_id,
            reservation_id,
            reserved: UsageAmount::new(
                from_i64(row.get("reserved_tokens"))?,
                from_i64(row.get("reserved_cost_micros"))?,
            ),
            created_at_unix_ms: from_i64(row.get("created_at_unix_ms"))?,
            expires_at_unix_ms: from_i64(row.get("expires_at_unix_ms"))?,
        },
        virtual_key_id,
        idempotency_key,
        state,
        committed_usage,
        released_usage,
    })
}

fn optional_usage_from_row(
    row: &Row,
    tokens_column: &str,
    cost_column: &str,
) -> Result<Option<UsageAmount>, PostgresRuntimeError> {
    match (
        row.get::<_, Option<i64>>(tokens_column),
        row.get::<_, Option<i64>>(cost_column),
    ) {
        (Some(tokens), Some(cost_micros)) => Ok(Some(UsageAmount::new(
            from_i64(tokens)?,
            from_i64(cost_micros)?,
        ))),
        (None, None) => Ok(None),
        _ => Err(PostgresRuntimeError::InvalidDatabaseState),
    }
}
