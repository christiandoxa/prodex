use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_LEDGER_KEY, RUNTIME_GATEWAY_REDIS_LEDGER_LOCK,
    RuntimeGatewayBackgroundTaskGuard, RuntimeGatewayStateStore, RuntimeLocalRewriteProxyShared,
    runtime_gateway_generate_virtual_key_token,
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
pub(super) const RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT: usize = 4_096;
const RUNTIME_GATEWAY_USAGE_RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);

pub(super) struct RuntimeGatewayPendingUsageDelta {
    delta: RuntimeGatewayVirtualKeyUsageDelta,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

pub(super) struct RuntimeGatewayUsageRequestGuard {
    pub(super) request_ids: Arc<Mutex<BTreeSet<u64>>>,
    pub(super) reconciliation:
        super::local_rewrite_gateway_reconciliation_worker::RuntimeGatewayReconciliationQueue,
    pub(super) request_id: u64,
    pub(super) terminal: Option<(
        RuntimeLocalRewriteProxyShared,
        super::provider_bridge::RuntimeProviderGatewaySpendEvent,
    )>,
}

impl RuntimeGatewayUsageRequestGuard {
    pub(super) fn new(
        shared: &RuntimeLocalRewriteProxyShared,
        request_id: u64,
        captured: &RuntimeProxyRequest,
    ) -> Self {
        let model = super::provider_bridge::runtime_provider_model_from_body(&captured.body);
        Self {
            request_ids: Arc::clone(&shared.gateway_usage.request_ids),
            reconciliation: shared.gateway_usage.reconciliation.clone(),
            request_id,
            terminal: Some((
                shared.clone(),
                super::provider_bridge::runtime_provider_gateway_terminal_spend_event(
                    request_id,
                    shared.provider.bridge_kind(),
                    &captured.path_and_query,
                    model.as_deref(),
                    499,
                    prodex_domain::ReservationReconciliationReason::Cancelled,
                ),
            )),
        }
    }

    pub(super) fn mark_terminal(
        &mut self,
        status: u16,
        reason: prodex_domain::ReservationReconciliationReason,
    ) {
        if let Some((_, event)) = self.terminal.as_mut() {
            event.status = status;
            event.reconciliation_reason = Some(reason);
        }
    }

    pub(super) fn complete_realtime(
        &mut self,
        accounting: &super::local_rewrite_gateway_admission::RuntimeGatewayRealtimeAccountingPlan,
        usage: super::local_rewrite_gateway_admission::RuntimeGatewayRealtimeUsage,
        elapsed_ms: u128,
    ) {
        let Some((shared, terminal)) = self.terminal.take() else {
            return;
        };
        let mut event =
            super::provider_bridge::runtime_provider_gateway_response_spend_event_from_tokens(
                self.request_id,
                shared.provider.bridge_kind(),
                &terminal.path,
                Some(&accounting.model),
                101,
                elapsed_ms,
                &[],
                usage.output_bytes,
                Some(usage.input_tokens),
                Some(usage.output_tokens),
                accounting.cost,
            );
        event.request_bytes = usage.input_bytes;
        event.reconciliation_reason = Some(if usage.policy_interrupted {
            prodex_domain::ReservationReconciliationReason::StreamInterrupted
        } else {
            prodex_domain::ReservationReconciliationReason::Completed
        });
        super::local_rewrite_transport::emit_runtime_gateway_spend_event(&shared, event);
    }
}

impl Drop for RuntimeGatewayUsageRequestGuard {
    fn drop(&mut self) {
        if let Some((shared, event)) = self.terminal.take() {
            super::local_rewrite_transport::emit_runtime_gateway_terminal_spend_event(
                &shared, event,
            );
        }
        self.request_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.request_id);
        self.reconciliation.cancel(self.request_id);
    }
}

pub(super) fn runtime_gateway_try_reserve_usage_delta(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<tokio::sync::OwnedSemaphorePermit> {
    Arc::clone(&shared.gateway_usage.usage_slots)
        .try_acquire_owned()
        .ok()
}

fn runtime_gateway_enqueue_usage_delta(
    pending: &mut Vec<RuntimeGatewayPendingUsageDelta>,
    delta: RuntimeGatewayVirtualKeyUsageDelta,
    permit: tokio::sync::OwnedSemaphorePermit,
) {
    pending.push(RuntimeGatewayPendingUsageDelta {
        delta,
        _permit: permit,
    });
}

fn runtime_gateway_restore_pending_usage_batch(
    pending: &mut Vec<RuntimeGatewayPendingUsageDelta>,
    mut failed: Vec<RuntimeGatewayPendingUsageDelta>,
) {
    failed.append(pending);
    *pending = failed;
}

pub(super) fn runtime_gateway_virtual_key_usage_load_strict(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let path = state_store.usage_path();
    let usage = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => runtime_gateway_sqlite_usage_load(path),
        RuntimeGatewayStateStore::Postgres { url, tls, .. } => {
            runtime_gateway_postgres_usage_load(url, tls)
        }
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
    permit: tokio::sync::OwnedSemaphorePermit,
) {
    let state_store = shared.gateway_state_store.clone();
    runtime_gateway_enqueue_usage_delta(
        &mut shared
            .gateway_usage
            .pending_deltas
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        delta,
        permit,
    );
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
    let background_task = RuntimeGatewayBackgroundTaskGuard::new(shared);
    drop(shared.runtime_shared.async_runtime.spawn_blocking(move || {
        let _background_task = background_task;
        loop {
            dirty.store(false, Ordering::Release);
            let pending_batch = pending_deltas
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .drain(..)
                .collect::<Vec<_>>();
            if !pending_batch.is_empty() {
                let deltas = pending_batch
                    .iter()
                    .map(|pending| pending.delta.clone())
                    .collect::<Vec<_>>();
                if let Err(_err) =
                    runtime_gateway_virtual_key_usage_apply_deltas(&state_store, &deltas)
                {
                    let mut pending = pending_deltas
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    runtime_gateway_restore_pending_usage_batch(&mut pending, pending_batch);
                    drop(pending);
                    dirty.store(true, Ordering::Release);
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
                    std::thread::sleep(RUNTIME_GATEWAY_USAGE_RETRY_BACKOFF);
                }
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
        RuntimeGatewayStateStore::Postgres { url, tls, .. } => {
            return runtime_gateway_postgres_usage_apply_deltas(url, tls, deltas)
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
        prodex_gateway_core::apply_gateway_virtual_key_usage_update(
            entry,
            prodex_gateway_core::GatewayVirtualKeyUsageUpdate {
                minute_epoch: delta.minute_epoch,
                reserved_tokens: delta.reserved_tokens,
                estimated_cost_microusd: delta.estimated_cost_microusd,
            },
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
        let slots = Arc::new(tokio::sync::Semaphore::new(
            RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT,
        ));
        let mut permits = Vec::new();
        for _ in 0..RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT {
            permits.push(Arc::clone(&slots).try_acquire_owned().unwrap());
        }

        assert!(Arc::clone(&slots).try_acquire_owned().is_err());
        drop(permits.pop());
        assert!(Arc::clone(&slots).try_acquire_owned().is_ok());
    }

    #[test]
    fn failed_usage_batch_is_requeued_before_newer_deltas() {
        fn pending(
            request_id: u64,
            slots: &Arc<tokio::sync::Semaphore>,
        ) -> RuntimeGatewayPendingUsageDelta {
            RuntimeGatewayPendingUsageDelta {
                delta: RuntimeGatewayVirtualKeyUsageDelta {
                    request_id,
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
                },
                _permit: Arc::clone(slots).try_acquire_owned().unwrap(),
            }
        }

        let slots = Arc::new(tokio::sync::Semaphore::new(2));
        let failed = vec![pending(1, &slots)];
        let mut newer = vec![pending(2, &slots)];
        assert_eq!(slots.available_permits(), 0);

        runtime_gateway_restore_pending_usage_batch(&mut newer, failed);

        assert_eq!(
            newer
                .iter()
                .map(|pending| pending.delta.request_id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        drop(newer);
        assert_eq!(slots.available_permits(), 2);
    }
}
