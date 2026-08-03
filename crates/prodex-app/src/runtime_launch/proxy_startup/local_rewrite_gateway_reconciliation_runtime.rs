use super::local_rewrite::RuntimeGatewayDurableReservationState;
use super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_open;
use super::local_rewrite_gateway_ledger_types::runtime_gateway_usd_to_microusd;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use crate::RuntimeRotationProxyShared;
use prodex_domain::UsageAmount;
use rusqlite::OptionalExtension;

pub(super) fn runtime_gateway_postgres_reconcile_usage(
    runtime_shared: &RuntimeRotationProxyShared,
    repository: Option<&prodex_storage_postgres_runtime::PostgresRepository>,
    storage: &prodex_storage_postgres::PostgresUsageReconciliationSqlPlan,
    command: prodex_storage::UsageReconciliationCommand,
) -> std::io::Result<()> {
    if storage.tenant_id != command.record.tenant_id || storage.storage_key != command.storage_key {
        return Err(std::io::Error::other(
            "application reconciliation storage mismatch",
        ));
    }
    runtime_shared
        .async_runtime
        .handle()
        .block_on(
            repository
                .ok_or_else(|| {
                    std::io::Error::other("PostgreSQL accounting repository unavailable")
                })?
                .reconcile_usage(command, runtime_gateway_unix_epoch_millis()),
        )
        .map(|_| ())
        .map_err(std::io::Error::other)
}

pub(super) fn runtime_gateway_postgres_load_durable_reservation_state(
    runtime_shared: &RuntimeRotationProxyShared,
    repository: Option<&prodex_storage_postgres_runtime::PostgresRepository>,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<Option<RuntimeGatewayDurableReservationState>> {
    let Some(tenant_id) = event
        .tenant_id
        .as_deref()
        .and_then(|tenant_id| tenant_id.parse::<prodex_domain::TenantId>().ok())
    else {
        return Ok(None);
    };
    let call_id_text = event
        .call_id
        .strip_prefix("prodex-")
        .unwrap_or(&event.call_id);
    let Ok(call_id) = call_id_text.parse::<prodex_domain::CallId>() else {
        return Ok(None);
    };
    let stored = runtime_shared
        .async_runtime
        .handle()
        .block_on(
            repository
                .ok_or_else(|| {
                    std::io::Error::other("PostgreSQL accounting repository unavailable")
                })?
                .load_reservation(tenant_id, call_id),
        )
        .map_err(std::io::Error::other)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let storage_key = match stored.virtual_key_id {
        Some(virtual_key_id) => {
            prodex_storage::TenantStorageKey::virtual_key(tenant_id, virtual_key_id)
        }
        None => prodex_storage::TenantStorageKey::tenant(tenant_id),
    };
    Ok(Some(RuntimeGatewayDurableReservationState {
        storage_key,
        record: stored.record,
    }))
}

pub(super) fn runtime_gateway_sqlite_load_durable_reservation_state(
    path: &std::path::Path,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<Option<RuntimeGatewayDurableReservationState>> {
    let Some(tenant_id) = event
        .tenant_id
        .as_deref()
        .and_then(|tenant_id| tenant_id.parse::<prodex_domain::TenantId>().ok())
    else {
        return Ok(None);
    };
    let call_id_text = event
        .call_id
        .strip_prefix("prodex-")
        .unwrap_or(&event.call_id);
    let Ok(call_id) = call_id_text.parse::<prodex_domain::CallId>() else {
        return Ok(None);
    };
    let conn = runtime_gateway_sqlite_open(path).map_err(std::io::Error::other)?;
    let stored = conn
        .query_row(
            "SELECT reservation_id, virtual_key_id, storage_scope, reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms FROM prodex_reservations WHERE tenant_id = ?1 AND call_id = ?2",
            rusqlite::params![tenant_id.to_string(), call_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(std::io::Error::other)?;
    let Some((
        reservation_id,
        virtual_key_id,
        storage_scope,
        reserved_tokens,
        reserved_cost_micros,
        created_at_unix_ms,
        expires_at_unix_ms,
    )) = stored
    else {
        return Ok(None);
    };
    let virtual_key_id = virtual_key_id
        .map(|value| value.parse::<prodex_domain::VirtualKeyId>())
        .transpose()
        .map_err(|_| std::io::Error::other("invalid SQLite durable reservation key"))?;
    let storage_key = prodex_storage::TenantStorageKey::from_storage_scope(
        tenant_id,
        virtual_key_id,
        &storage_scope,
    )
    .ok_or_else(|| std::io::Error::other("invalid SQLite durable reservation scope"))?;
    let reservation_id = reservation_id
        .parse::<prodex_domain::ReservationId>()
        .map_err(|_| std::io::Error::other("invalid SQLite durable reservation id"))?;
    let to_u64 = |value: i64| {
        u64::try_from(value).map_err(|_| std::io::Error::other("invalid SQLite durable usage"))
    };
    Ok(Some(RuntimeGatewayDurableReservationState {
        storage_key,
        record: prodex_domain::ReservationRecord {
            tenant_id,
            call_id,
            reservation_id,
            reserved: UsageAmount::new(to_u64(reserved_tokens)?, to_u64(reserved_cost_micros)?),
            created_at_unix_ms: to_u64(created_at_unix_ms)?,
            expires_at_unix_ms: to_u64(expires_at_unix_ms)?,
        },
    }))
}

pub(super) fn runtime_gateway_durable_actual_usage(
    event: &RuntimeProviderGatewaySpendEvent,
) -> UsageAmount {
    let actual_tokens = event
        .input_tokens
        .unwrap_or_default()
        .saturating_add(event.output_tokens.unwrap_or_default());
    let actual_cost_micros = runtime_gateway_usd_to_microusd(event.cost_usd).unwrap_or_default();
    UsageAmount::new(actual_tokens, actual_cost_micros)
}
