use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_LEDGER_KEY, RuntimeGatewayBackgroundTaskGuard, RuntimeGatewayStateStore,
    RuntimeLocalRewriteProxyShared,
};
#[cfg(test)]
use super::local_rewrite_gateway_file_ledger::runtime_gateway_file_ledger_append_deltas;
use super::local_rewrite_gateway_file_ledger::{
    runtime_gateway_file_ledger_append_deltas_after_load, runtime_gateway_file_ledger_load,
};
use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_billing_ledger_entry_identity,
};
use super::local_rewrite_gateway_store_file::{
    runtime_gateway_read_regular_file, runtime_gateway_state_path_is_absent,
    runtime_gateway_write_file_atomic,
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
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

const RUNTIME_GATEWAY_REDIS_USAGE_KEY: &str = "prodex:gateway:virtual_key_usage";
const RUNTIME_GATEWAY_FILE_USAGE_BASELINE_VERSION: u32 = 1;
pub(super) const RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT: usize = 4_096;
const RUNTIME_GATEWAY_USAGE_RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);

pub(super) struct RuntimeGatewayPendingUsageDelta {
    delta: RuntimeGatewayVirtualKeyUsageDelta,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeGatewayFileUsageBaseline {
    version: u32,
    ledger_entries: usize,
    ledger_prefix_sha256: String,
    usage: BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
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
        RuntimeGatewayStateStore::File { ledger_path, .. } => {
            runtime_gateway_virtual_key_usage_file_rebuild_strict(path, ledger_path)
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
        runtime_gateway_usage_save_loop(
            &state_store,
            &pending_deltas,
            &dirty,
            &in_flight,
            &log_path,
        );
    }));
}

fn runtime_gateway_usage_save_loop(
    state_store: &RuntimeGatewayStateStore,
    pending_deltas: &Arc<Mutex<Vec<RuntimeGatewayPendingUsageDelta>>>,
    dirty: &Arc<std::sync::atomic::AtomicBool>,
    in_flight: &Arc<std::sync::atomic::AtomicBool>,
    log_path: &Path,
) {
    loop {
        dirty.store(false, Ordering::Release);
        runtime_gateway_save_usage_batch(state_store, pending_deltas, dirty, log_path);
        if !runtime_gateway_usage_save_should_continue(dirty, in_flight) {
            return;
        }
    }
}

fn runtime_gateway_save_usage_batch(
    state_store: &RuntimeGatewayStateStore,
    pending_deltas: &Arc<Mutex<Vec<RuntimeGatewayPendingUsageDelta>>>,
    dirty: &Arc<std::sync::atomic::AtomicBool>,
    log_path: &Path,
) {
    let pending_batch = pending_deltas
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .drain(..)
        .collect::<Vec<_>>();
    if pending_batch.is_empty() {
        return;
    }
    let deltas = pending_batch
        .iter()
        .map(|pending| pending.delta.clone())
        .collect::<Vec<_>>();
    if runtime_gateway_virtual_key_usage_apply_deltas(state_store, &deltas).is_ok() {
        return;
    }
    let mut pending = pending_deltas
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    runtime_gateway_restore_pending_usage_batch(&mut pending, pending_batch);
    drop(pending);
    dirty.store(true, Ordering::Release);
    runtime_proxy_log_to_path(
        log_path,
        &runtime_proxy_structured_log_message(
            "gateway_virtual_key_usage_save_failed",
            [
                runtime_proxy_log_field("backend", state_store.label()),
                runtime_proxy_log_field("error_kind", "gateway_usage_persistence_failed"),
            ],
        ),
    );
    std::thread::sleep(RUNTIME_GATEWAY_USAGE_RETRY_BACKOFF);
}

fn runtime_gateway_usage_save_should_continue(
    dirty: &Arc<std::sync::atomic::AtomicBool>,
    in_flight: &Arc<std::sync::atomic::AtomicBool>,
) -> bool {
    if dirty.load(Ordering::Acquire) {
        return true;
    }
    in_flight.store(false, Ordering::Release);
    dirty.load(Ordering::Acquire) && !in_flight.swap(true, Ordering::AcqRel)
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
                RUNTIME_GATEWAY_REDIS_LEDGER_KEY,
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
    let mut baseline = None;
    let entries = runtime_gateway_file_ledger_append_deltas_after_load(
        ledger_path,
        deltas,
        |existing_entries| {
            baseline = Some(runtime_gateway_file_usage_baseline_load_or_create(
                path,
                existing_entries,
            )?);
            Ok(())
        },
    )?;
    let baseline =
        baseline.ok_or_else(|| std::io::Error::other("gateway usage baseline missing"))?;
    let usage = runtime_gateway_file_usage_rebuild(&baseline, &entries)?;
    let payload = serde_json::to_vec_pretty(&usage).map_err(std::io::Error::other)?;
    runtime_gateway_write_file_atomic(path, "json.tmp", |file| file.write_all(&payload))?;
    let _ = lock_file.unlock();
    Ok(())
}

#[cfg(test)]
fn runtime_gateway_virtual_key_usage_file_load_strict(
    path: &Path,
) -> std::io::Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    Ok(runtime_gateway_virtual_key_usage_file_load_optional_strict(path)?.unwrap_or_default())
}

fn runtime_gateway_virtual_key_usage_file_load_optional_strict(
    path: &Path,
) -> std::io::Result<Option<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>> {
    match runtime_gateway_read_regular_file(path)? {
        Some(bytes) => serde_json::from_slice::<
            BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
        >(&bytes)
        .map(Some)
        .map_err(std::io::Error::other),
        None => Ok(None),
    }
}

fn runtime_gateway_virtual_key_usage_file_rebuild_strict(
    usage_path: &Path,
    ledger_path: &Path,
) -> std::io::Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    if let Some(parent) = usage_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = usage_path.with_extension("json.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let entries = runtime_gateway_file_ledger_load(ledger_path, usize::MAX)?;
    let baseline_path = usage_path.with_extension("ledger-baseline.json");
    if entries.is_empty()
        && runtime_gateway_state_path_is_absent(usage_path)?
        && runtime_gateway_state_path_is_absent(&baseline_path)?
    {
        let _ = lock_file.unlock();
        return Ok(BTreeMap::new());
    }
    let baseline = runtime_gateway_file_usage_baseline_load_or_create(usage_path, &entries)?;
    let usage = runtime_gateway_file_usage_rebuild(&baseline, &entries)?;
    let payload = serde_json::to_vec_pretty(&usage).map_err(std::io::Error::other)?;
    runtime_gateway_write_file_atomic(usage_path, "json.tmp", |file| file.write_all(&payload))?;
    let _ = lock_file.unlock();
    Ok(usage)
}

fn runtime_gateway_file_usage_baseline_load_or_create(
    usage_path: &Path,
    entries: &[RuntimeGatewayBillingLedgerEntry],
) -> std::io::Result<RuntimeGatewayFileUsageBaseline> {
    let path = usage_path.with_extension("ledger-baseline.json");
    if let Some(bytes) = runtime_gateway_read_regular_file(&path)? {
        let baseline = serde_json::from_slice::<RuntimeGatewayFileUsageBaseline>(&bytes)
            .map_err(std::io::Error::other)?;
        if baseline.version != RUNTIME_GATEWAY_FILE_USAGE_BASELINE_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "unsupported gateway usage baseline version {}",
                    baseline.version
                ),
            ));
        }
        runtime_gateway_file_usage_validate_baseline(&baseline, entries)?;
        return Ok(baseline);
    }

    let existing = runtime_gateway_virtual_key_usage_file_load_optional_strict(usage_path)?;
    let baseline = RuntimeGatewayFileUsageBaseline {
        version: RUNTIME_GATEWAY_FILE_USAGE_BASELINE_VERSION,
        ledger_entries: existing.as_ref().map_or(0, |_| entries.len()),
        ledger_prefix_sha256: runtime_gateway_file_usage_ledger_prefix_sha256(
            if existing.is_some() { entries } else { &[] },
        ),
        usage: existing.unwrap_or_default(),
    };
    let payload = serde_json::to_vec_pretty(&baseline).map_err(std::io::Error::other)?;
    runtime_gateway_write_file_atomic(&path, "json.tmp", |file| file.write_all(&payload))?;
    Ok(baseline)
}

fn runtime_gateway_file_usage_rebuild(
    baseline: &RuntimeGatewayFileUsageBaseline,
    entries: &[RuntimeGatewayBillingLedgerEntry],
) -> std::io::Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    runtime_gateway_file_usage_validate_baseline(baseline, entries)?;
    let mut usage = baseline.usage.clone();
    let mut seen = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.phase != "request"
            || !seen.insert(runtime_gateway_billing_ledger_entry_identity(entry))
            || index < baseline.ledger_entries
        {
            continue;
        }
        prodex_gateway_core::apply_gateway_virtual_key_usage_update(
            usage.entry(entry.key_name.clone()).or_default(),
            prodex_gateway_core::GatewayVirtualKeyUsageUpdate {
                minute_epoch: entry.minute_epoch,
                reserved_tokens: entry.reserved_tokens.unwrap_or(entry.input_tokens),
                estimated_cost_microusd: entry.estimated_cost_microusd,
            },
        );
    }
    Ok(usage)
}

fn runtime_gateway_file_usage_validate_baseline(
    baseline: &RuntimeGatewayFileUsageBaseline,
    entries: &[RuntimeGatewayBillingLedgerEntry],
) -> std::io::Result<()> {
    if baseline.ledger_entries > entries.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "gateway billing ledger is shorter than its usage baseline",
        ));
    }
    let prefix_sha256 =
        runtime_gateway_file_usage_ledger_prefix_sha256(&entries[..baseline.ledger_entries]);
    if prefix_sha256 != baseline.ledger_prefix_sha256 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "gateway billing ledger prefix does not match its usage baseline",
        ));
    }
    Ok(())
}

fn runtime_gateway_file_usage_ledger_prefix_sha256(
    entries: &[RuntimeGatewayBillingLedgerEntry],
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"prodex-gateway-ledger-prefix-v1\0");
    digest.update((entries.len() as u64).to_be_bytes());
    for entry in entries {
        let identity = runtime_gateway_billing_ledger_entry_identity(entry);
        digest.update((identity.len() as u64).to_be_bytes());
        digest.update(identity.as_bytes());
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = digest.finalize();
    let mut fingerprint = String::with_capacity(bytes.len() * 2);
    for &byte in bytes.iter() {
        fingerprint.push(HEX[usize::from(byte >> 4)] as char);
        fingerprint.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    fingerprint
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-gateway-usage-{name}-{stamp}"))
    }

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

    #[test]
    fn file_usage_rebuild_recovers_partial_commit_without_resetting_legacy_counters() {
        let root = temp_dir("partial-commit");
        std::fs::create_dir_all(&root).unwrap();
        let usage_path = root.join("usage.json");
        let ledger_path = root.join("ledger.jsonl");
        let state_store = RuntimeGatewayStateStore::File {
            key_store_path: root.join("keys.json"),
            usage_path: usage_path.clone(),
            ledger_path: ledger_path.clone(),
        };
        let old_delta = RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: format!("prodex-{}", prodex_domain::CallId::new()),
            key_name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 3,
            reserved_tokens: 7,
            estimated_cost_microusd: Some(11),
            created_at_epoch: 1_700_000_000,
        };
        runtime_gateway_file_ledger_append_deltas(&ledger_path, &[old_delta]).unwrap();
        let legacy_usage = BTreeMap::from([(
            "team-a".to_string(),
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                minute_epoch: 100,
                requests_this_minute: 5,
                tokens_this_minute: 50,
                requests_total: 20,
                spend_microusd: 200,
            },
        )]);
        std::fs::write(
            &usage_path,
            serde_json::to_vec_pretty(&legacy_usage).unwrap(),
        )
        .unwrap();

        let new_delta = RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 2,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: format!("prodex-{}", prodex_domain::CallId::new()),
            key_name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 3,
            reserved_tokens: 13,
            estimated_cost_microusd: Some(17),
            created_at_epoch: 1_700_000_001,
        };

        let err = runtime_gateway_file_ledger_append_deltas_after_load(
            &ledger_path,
            std::slice::from_ref(&new_delta),
            |entries| {
                runtime_gateway_file_usage_baseline_load_or_create(&usage_path, entries)?;
                Err(std::io::Error::other(
                    "injected failure after baseline creation",
                ))
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("injected failure"));
        assert!(usage_path.with_extension("ledger-baseline.json").is_file());
        assert_eq!(
            runtime_gateway_file_ledger_load(&ledger_path, usize::MAX)
                .unwrap()
                .len(),
            1
        );

        let migrated =
            runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path)
                .unwrap();
        assert_eq!(migrated["team-a"].requests_total, 20);

        runtime_gateway_file_ledger_append_deltas(&ledger_path, std::slice::from_ref(&new_delta))
            .unwrap();

        runtime_gateway_virtual_key_usage_apply_deltas(
            &state_store,
            std::slice::from_ref(&new_delta),
        )
        .unwrap();
        runtime_gateway_virtual_key_usage_apply_deltas(&state_store, &[new_delta]).unwrap();

        let usage = runtime_gateway_virtual_key_usage_file_load_strict(&usage_path).unwrap();
        assert_eq!(usage["team-a"].requests_total, 21);
        assert_eq!(usage["team-a"].requests_this_minute, 6);
        assert_eq!(usage["team-a"].tokens_this_minute, 63);
        assert_eq!(usage["team-a"].spend_microusd, 217);
        let entries = runtime_gateway_file_ledger_load(&ledger_path, usize::MAX).unwrap();
        assert_eq!(entries.len(), 2);
        let mut legacy_entry = entries[1].clone();
        legacy_entry.reserved_tokens = None;
        let legacy_rebuild = runtime_gateway_file_usage_rebuild(
            &RuntimeGatewayFileUsageBaseline {
                version: RUNTIME_GATEWAY_FILE_USAGE_BASELINE_VERSION,
                ledger_entries: 0,
                ledger_prefix_sha256: runtime_gateway_file_usage_ledger_prefix_sha256(&[]),
                usage: BTreeMap::new(),
            },
            &[legacy_entry],
        )
        .unwrap();
        assert_eq!(legacy_rebuild["team-a"].tokens_this_minute, 3);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_usage_rebuild_rejects_same_length_reordered_ledger_prefix() {
        let root = temp_dir("reordered-prefix");
        std::fs::create_dir_all(&root).unwrap();
        let usage_path = root.join("usage.json");
        let ledger_path = root.join("ledger.jsonl");
        let first = RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: format!("prodex-{}", prodex_domain::CallId::new()),
            key_name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 3,
            reserved_tokens: 7,
            estimated_cost_microusd: Some(11),
            created_at_epoch: 1_700_000_000,
        };
        let mut second = first.clone();
        second.request_id = 2;
        second.typed_request_id = format!("prodex-{}", prodex_domain::RequestId::new());
        second.call_id = format!("prodex-{}", prodex_domain::CallId::new());
        second.key_name = "team-b".to_string();
        runtime_gateway_file_ledger_append_deltas(&ledger_path, &[first, second]).unwrap();
        std::fs::write(&usage_path, b"{}").unwrap();

        runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path).unwrap();
        let original_usage = std::fs::read(&usage_path).unwrap();
        let mut entries = runtime_gateway_file_ledger_load(&ledger_path, usize::MAX).unwrap();
        entries.swap(0, 1);
        let mut payload = Vec::new();
        for entry in &entries {
            serde_json::to_writer(&mut payload, entry).unwrap();
            payload.push(b'\n');
        }
        runtime_gateway_write_file_atomic(&ledger_path, "jsonl.tmp", |file| {
            file.write_all(&payload)
        })
        .unwrap();

        let err = runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path)
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("prefix does not match"));
        assert_eq!(std::fs::read(&usage_path).unwrap(), original_usage);

        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "local_rewrite_gateway_usage_file_tests.rs"]
mod file_tests;
