use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use fs2::FileExt;

use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_apply_response_to_ledger_entry,
    runtime_gateway_billing_ledger_entry_from_delta,
};
use super::local_rewrite_gateway_store_file::{
    runtime_gateway_read_regular_file, runtime_gateway_write_file_atomic,
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
    let mut file = open_gateway_ledger_append_file(path)?;
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

fn open_gateway_ledger_append_file(path: &Path) -> std::io::Result<File> {
    let existing_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "refusing to append gateway ledger through symlink {}",
                    path.display()
                ),
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("gateway ledger path {} is not a file", path.display()),
            ));
        }
        Ok(metadata) => Some(metadata),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };

    let mut options = OpenOptions::new();
    options.append(true);
    if existing_metadata.is_some() {
        options.create(false);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    if let Some(metadata) = existing_metadata
        && !runtime_gateway_same_file_metadata(&metadata, &file.metadata()?)
    {
        return Err(std::io::Error::other(format!(
            "gateway ledger path changed while opening {}",
            path.display()
        )));
    }
    Ok(file)
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
        runtime_gateway_write_file_atomic(path, "jsonl.tmp", |file| {
            for entry in &entries {
                serde_json::to_writer(&mut *file, entry).map_err(std::io::Error::other)?;
                file.write_all(b"\n")?;
            }
            Ok(())
        })?;
    }
    let _ = lock_file.unlock();
    Ok(changed)
}

pub(super) fn runtime_gateway_file_ledger_load(
    path: &Path,
    limit: usize,
) -> std::io::Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let bytes = match runtime_gateway_read_regular_file(path)? {
        Some(bytes) => bytes,
        None => return Ok(Vec::new()),
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

#[cfg(unix)]
fn runtime_gateway_same_file_metadata(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn runtime_gateway_same_file_metadata(
    _left: &std::fs::Metadata,
    _right: &std::fs::Metadata,
) -> bool {
    true
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

    #[cfg(unix)]
    #[test]
    fn append_rejects_symlink_ledger_without_touching_target() {
        let root = temp_dir("append-symlink");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let target = root.join("target.jsonl");
        std::fs::write(&target, "do not touch\n").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        let err = runtime_gateway_file_ledger_append_deltas(
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
        .expect_err("symlink ledger should be rejected");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "do not touch\n");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_symlink_ledger_without_reading_target() {
        let root = temp_dir("load-symlink");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let target = root.join("target.jsonl");
        std::fs::write(&target, "not a prodex ledger secret\n").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        let err = runtime_gateway_file_ledger_load(&path, usize::MAX)
            .expect_err("symlink ledger should be rejected");

        assert!(err.to_string().contains("symlink"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_rejects_oversized_ledger_file() {
        let root = temp_dir("load-large");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let file = File::create(&path).unwrap();
        file.set_len(64 * 1024 * 1024 + 1).unwrap();

        let err = runtime_gateway_file_ledger_load(&path, usize::MAX)
            .expect_err("oversized ledger should be rejected");

        assert!(err.to_string().contains("safe size limit"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
