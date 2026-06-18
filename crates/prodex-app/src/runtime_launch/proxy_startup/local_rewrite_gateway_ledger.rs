use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_LEDGER_KEY, RUNTIME_GATEWAY_REDIS_LEDGER_LOCK,
    RuntimeLocalRewriteProxyShared,
};
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_file_ledger::{
    runtime_gateway_file_ledger_load, runtime_gateway_file_ledger_reconcile_response,
};
use super::local_rewrite_gateway_ledger_types::RuntimeGatewayBillingLedgerEntry;
use super::local_rewrite_gateway_redis_ledger::{
    runtime_gateway_redis_ledger_load, runtime_gateway_redis_ledger_reconcile_response,
};
use super::local_rewrite_gateway_sql_ledger::{
    runtime_gateway_postgres_ledger_load, runtime_gateway_postgres_ledger_reconcile_response,
    runtime_gateway_sqlite_ledger_load, runtime_gateway_sqlite_ledger_reconcile_response,
};
use super::local_rewrite_gateway_util::{
    runtime_gateway_generate_virtual_key_token, runtime_gateway_unix_epoch_seconds,
};
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use super::*;
use std::sync::Arc;

pub(super) fn runtime_gateway_billing_ledger_load(
    state_store: &RuntimeGatewayStateStore,
    limit: usize,
) -> std::io::Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    match state_store {
        RuntimeGatewayStateStore::File { ledger_path, .. } => {
            runtime_gateway_file_ledger_load(ledger_path, limit)
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_sqlite_ledger_load(path, limit).map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_postgres_ledger_load(url, limit).map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_ledger_load(url, RUNTIME_GATEWAY_REDIS_LEDGER_KEY, limit)
                .map_err(std::io::Error::other)
        }
    }
}

pub(super) fn schedule_runtime_gateway_billing_ledger_reconcile(
    shared: &RuntimeLocalRewriteProxyShared,
    event: RuntimeProviderGatewaySpendEvent,
) {
    if event.phase != "response" {
        return;
    }
    let should_reconcile = shared
        .gateway_usage
        .request_ids
        .lock()
        .map(|request_ids| request_ids.contains(&event.request))
        .unwrap_or(false);
    if !should_reconcile {
        return;
    }
    let state_store = shared.gateway_state_store.clone();
    let runtime_shared = shared.runtime_shared.clone();
    let request_ids = Arc::clone(&shared.gateway_usage.request_ids);
    shared.runtime_shared.async_runtime.spawn_blocking(move || {
        let mut last_error = None;
        for attempt in 0..25 {
            match runtime_gateway_billing_ledger_reconcile_response(&state_store, &event) {
                Ok(true) => {
                    if let Ok(mut request_ids) = request_ids.lock() {
                        request_ids.remove(&event.request);
                    }
                    return;
                }
                Ok(false) => {
                    if attempt < 24 {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
                Err(err) => {
                    last_error = Some(err);
                    break;
                }
            }
        }
        if let Ok(mut request_ids) = request_ids.lock() {
            request_ids.remove(&event.request);
        }
        if let Some(err) = last_error {
            crate::runtime_proxy_log(
                &runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_billing_ledger_reconcile_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("backend", state_store.label()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
        }
    });
}

fn runtime_gateway_billing_ledger_reconcile_response(
    state_store: &RuntimeGatewayStateStore,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<bool> {
    match state_store {
        RuntimeGatewayStateStore::File { ledger_path, .. } => {
            runtime_gateway_file_ledger_reconcile_response(
                ledger_path,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_sqlite_ledger_reconcile_response(
                path,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
            .map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_postgres_ledger_reconcile_response(
                url,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
            .map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_ledger_reconcile_response(
                url,
                RUNTIME_GATEWAY_REDIS_LEDGER_KEY,
                RUNTIME_GATEWAY_REDIS_LEDGER_LOCK,
                runtime_gateway_generate_virtual_key_token,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
            .map_err(std::io::Error::other)
        }
    }
}
