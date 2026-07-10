use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_LEDGER_KEY, RUNTIME_GATEWAY_REDIS_LEDGER_LOCK, RuntimeGatewayStateStore,
    RuntimeLocalRewriteProxyShared, runtime_gateway_generate_virtual_key_token,
};
use super::local_rewrite_gateway_file_ledger::{
    runtime_gateway_file_ledger_append_deltas, runtime_gateway_file_ledger_load,
};
use super::local_rewrite_gateway_store_file::{
    runtime_gateway_read_regular_file, runtime_gateway_write_file_atomic,
};
use super::local_rewrite_gateway_usage_backend::{
    RuntimeGatewayVirtualKeyUsageDelta, runtime_gateway_postgres_usage_apply_deltas,
    runtime_gateway_postgres_usage_load, runtime_gateway_redis_usage_apply_deltas,
    runtime_gateway_redis_usage_load, runtime_gateway_sqlite_usage_apply_deltas,
    runtime_gateway_sqlite_usage_load,
};
use super::*;
use anyhow::Result;
use fs2::FileExt;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

const RUNTIME_GATEWAY_REDIS_USAGE_KEY: &str = "prodex:gateway:virtual_key_usage";
const RUNTIME_GATEWAY_REDIS_USAGE_LOCK: &str = "prodex:gateway:virtual_key_usage:lock";
const RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT: usize = 4_096;

fn runtime_gateway_enqueue_usage_delta(
    pending: &mut Vec<RuntimeGatewayVirtualKeyUsageDelta>,
    delta: RuntimeGatewayVirtualKeyUsageDelta,
) -> bool {
    if pending.len() >= RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT {
        return false;
    }
    pending.push(delta);
    true
}

pub(super) fn runtime_gateway_virtual_key_usage_load_strict(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let path = state_store.usage_path();
    let usage = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => runtime_gateway_sqlite_usage_load(path),
        RuntimeGatewayStateStore::Postgres { url, .. } => runtime_gateway_postgres_usage_load(url),
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_usage_load(url, RUNTIME_GATEWAY_REDIS_USAGE_KEY)
        }
        RuntimeGatewayStateStore::File { .. } => {
            runtime_gateway_virtual_key_usage_file_load_strict(path)
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        }
    }
    .inspect_err(|_err| {
        runtime_proxy_log_to_path(
            log_path,
            &runtime_proxy_structured_log_message(
                "gateway_virtual_key_usage_load_failed",
                [
                    runtime_proxy_log_field("backend", state_store.label()),
                    runtime_proxy_log_field("error_kind", "gateway_usage_persistence_failed"),
                ],
            ),
        );
    })?;
    Ok(usage)
}

pub(super) fn schedule_runtime_gateway_virtual_key_usage_save(
    shared: &RuntimeLocalRewriteProxyShared,
    delta: RuntimeGatewayVirtualKeyUsageDelta,
) {
    let state_store = shared.gateway_state_store.clone();
    if let Ok(mut pending) = shared.gateway_usage.pending_deltas.lock() {
        if !runtime_gateway_enqueue_usage_delta(&mut pending, delta.clone()) {
            // ponytail: persistence is best effort; use synchronous admission if durable backpressure is required.
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_virtual_key_usage_delta_dropped",
                    [
                        runtime_proxy_log_field("request", delta.request_id.to_string()),
                        runtime_proxy_log_field("key", delta.key_name.as_str()),
                        runtime_proxy_log_field("reason", "queue_limit"),
                    ],
                ),
            );
            return;
        }
    } else {
        return;
    }
    shared
        .gateway_usage
        .save_dirty
        .store(true, Ordering::Release);
    if shared
        .gateway_usage
        .save_in_flight
        .swap(true, Ordering::AcqRel)
    {
        return;
    }

    let pending_deltas = Arc::clone(&shared.gateway_usage.pending_deltas);
    let dirty = Arc::clone(&shared.gateway_usage.save_dirty);
    let in_flight = Arc::clone(&shared.gateway_usage.save_in_flight);
    let log_path = shared.runtime_shared.log_path.clone();
    drop(shared.runtime_shared.async_runtime.spawn_blocking(move || {
        loop {
            dirty.store(false, Ordering::Release);
            let deltas = pending_deltas
                .lock()
                .map(|mut pending| pending.drain(..).collect::<Vec<_>>())
                .unwrap_or_default();
            if !deltas.is_empty()
                && let Err(_err) =
                    runtime_gateway_virtual_key_usage_apply_deltas(&state_store, &deltas)
            {
                runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "gateway_virtual_key_usage_save_failed",
                        [
                            runtime_proxy_log_field("backend", state_store.label()),
                            runtime_proxy_log_field(
                                "error_kind",
                                "gateway_usage_persistence_failed",
                            ),
                        ],
                    ),
                );
            }
            if !dirty.load(Ordering::Acquire) {
                in_flight.store(false, Ordering::Release);
                if dirty.load(Ordering::Acquire) && !in_flight.swap(true, Ordering::AcqRel) {
                    continue;
                }
                break;
            }
        }
    }));
}

pub(super) fn runtime_gateway_virtual_key_usage_apply_deltas(
    state_store: &RuntimeGatewayStateStore,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> std::io::Result<()> {
    let path = state_store.usage_path();
    match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            return runtime_gateway_sqlite_usage_apply_deltas(path, deltas)
                .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            return runtime_gateway_postgres_usage_apply_deltas(url, deltas)
                .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            return runtime_gateway_redis_usage_apply_deltas(
                url,
                RUNTIME_GATEWAY_REDIS_USAGE_KEY,
                RUNTIME_GATEWAY_REDIS_USAGE_LOCK,
                RUNTIME_GATEWAY_REDIS_LEDGER_KEY,
                RUNTIME_GATEWAY_REDIS_LEDGER_LOCK,
                runtime_gateway_generate_virtual_key_token,
                deltas,
            )
            .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::File { .. } => {}
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("json.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let ledger_path = state_store.ledger_path();
    let mut seen_requests = runtime_gateway_file_ledger_load(ledger_path, usize::MAX)?
        .into_iter()
        .filter(|entry| entry.phase == "request")
        .map(|entry| (entry.request, entry.key_name.to_ascii_lowercase()))
        .collect::<std::collections::BTreeSet<_>>();
    let unique_deltas = deltas
        .iter()
        .filter(|&delta| {
            seen_requests.insert((delta.request_id, delta.key_name.to_ascii_lowercase()))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut usage = runtime_gateway_virtual_key_usage_file_load_strict(path)?;
    for delta in &unique_deltas {
        let entry = usage.entry(delta.key_name.clone()).or_default();
        let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
            key_name: delta.key_name.clone(),
            model: None,
            input_tokens: delta.input_tokens,
            reserved_tokens: delta.reserved_tokens,
            estimated_cost_microusd: delta.estimated_cost_microusd,
        };
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            entry,
            &admission,
            delta.minute_epoch,
        );
    }
    let payload = serde_json::to_vec_pretty(&usage).map_err(std::io::Error::other)?;
    runtime_gateway_write_file_atomic(path, "json.tmp", |file| file.write_all(&payload))?;
    runtime_gateway_file_ledger_append_deltas(ledger_path, &unique_deltas)?;
    let _ = lock_file.unlock();
    Ok(())
}

fn runtime_gateway_virtual_key_usage_file_load_strict(
    path: &Path,
) -> std::io::Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    match runtime_gateway_read_regular_file(path)? {
        Some(bytes) => serde_json::from_slice::<
            BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
        >(&bytes)
        .map_err(std::io::Error::other),
        None => Ok(BTreeMap::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_usage_delta_queue_is_bounded() {
        let delta = RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: format!("prodex-{}", prodex_domain::CallId::new()),
            key_name: "team-a".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            minute_epoch: 1,
            input_tokens: 1,
            reserved_tokens: 1,
            estimated_cost_microusd: Some(1),
            created_at_epoch: 1,
        };
        let mut pending = Vec::new();
        for _ in 0..RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT {
            assert!(runtime_gateway_enqueue_usage_delta(
                &mut pending,
                delta.clone()
            ));
        }

        assert!(!runtime_gateway_enqueue_usage_delta(&mut pending, delta));
    }
}
