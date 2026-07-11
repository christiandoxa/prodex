use super::local_rewrite::RuntimeGatewayDurableReservationState;
use super::local_rewrite_gateway_ledger_types::runtime_gateway_usd_to_microusd;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use crate::RuntimeRotationProxyShared;
use prodex_domain::UsageAmount;

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

pub(super) fn runtime_gateway_durable_actual_usage(
    record: &prodex_domain::ReservationRecord,
    event: &RuntimeProviderGatewaySpendEvent,
) -> (UsageAmount, bool) {
    let actual_tokens = event
        .input_tokens
        .unwrap_or_default()
        .saturating_add(event.output_tokens.unwrap_or_default());
    let actual_cost_micros = runtime_gateway_usd_to_microusd(event.cost_usd).unwrap_or_default();
    let actual = UsageAmount::new(actual_tokens, actual_cost_micros);
    if actual.exceeds(record.reserved) {
        // ponytail: under-reserved requests still settle at the reserved estimate; once every
        // request reserves enough output-side headroom, remove this clamp and trust actual usage.
        (record.reserved, true)
    } else {
        (actual, false)
    }
}
