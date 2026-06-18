use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use fs2::FileExt;

use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_apply_response_to_ledger_entry,
    runtime_gateway_billing_ledger_entry_from_delta,
};
use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;

pub(super) fn runtime_gateway_file_ledger_append_deltas(
    path: &Path,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("jsonl.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    for delta in deltas {
        serde_json::to_writer(
            &mut file,
            &runtime_gateway_billing_ledger_entry_from_delta(delta),
        )
        .map_err(std::io::Error::other)?;
        file.write_all(b"\n")?;
    }
    let _ = lock_file.unlock();
    Ok(())
}

pub(super) fn runtime_gateway_file_ledger_reconcile_response(
    path: &Path,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) -> std::io::Result<bool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("jsonl.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let mut entries = runtime_gateway_file_ledger_load(path, usize::MAX)?;
    let mut changed = false;
    for entry in &mut entries {
        if entry.request != event.request || entry.phase != "request" {
            continue;
        }
        runtime_gateway_apply_response_to_ledger_entry(entry, event, reconciled_at_epoch);
        changed = true;
    }
    if changed {
        let tmp_path = path.with_extension("jsonl.tmp");
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)?;
            for entry in &entries {
                serde_json::to_writer(&mut file, entry).map_err(std::io::Error::other)?;
                file.write_all(b"\n")?;
            }
            file.sync_all()?;
        }
        std::fs::rename(tmp_path, path)?;
    }
    let _ = lock_file.unlock();
    Ok(changed)
}

pub(super) fn runtime_gateway_file_ledger_load(
    path: &Path,
    limit: usize,
) -> std::io::Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&bytes).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(trimmed) {
            entries.push(entry);
        }
    }
    if entries.len() > limit {
        Ok(entries.split_off(entries.len() - limit))
    } else {
        Ok(entries)
    }
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
        std::env::temp_dir().join(format!("prodex-gateway-file-ledger-{name}-{stamp}"))
    }

    #[test]
    fn append_and_load_file_ledger_entries() {
        let root = temp_dir("append");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        runtime_gateway_file_ledger_append_deltas(
            &path,
            &[RuntimeGatewayVirtualKeyUsageDelta {
                request_id: 1,
                key_name: "alpha".to_string(),
                model: "gpt-5".to_string(),
                minute_epoch: 10,
                input_tokens: 100,
                estimated_cost_microusd: Some(250_000),
                created_at_epoch: 20,
            }],
        )
        .unwrap();

        let entries = runtime_gateway_file_ledger_load(&path, usize::MAX).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key_name, "alpha");

        std::fs::remove_dir_all(root).unwrap();
    }
}
