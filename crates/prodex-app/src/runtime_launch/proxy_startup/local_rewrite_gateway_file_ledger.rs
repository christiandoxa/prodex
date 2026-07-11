use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use fs2::FileExt;

use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_apply_response_to_ledger_entry,
    runtime_gateway_billing_ledger_entry_from_delta, runtime_gateway_billing_ledger_entry_identity,
};
use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;

const RUNTIME_GATEWAY_LEDGER_FILE_MAX_BYTES: u64 = 64 * 1024 * 1024;

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
    let mut seen = runtime_gateway_file_ledger_entry_ids(path)?;
    let mut file = open_gateway_ledger_append_file(path)?;
    for delta in deltas {
        let entry = runtime_gateway_billing_ledger_entry_from_delta(delta);
        if !seen.insert(runtime_gateway_file_ledger_entry_id(&entry)) {
            continue;
        }
        serde_json::to_writer(&mut file, &entry).map_err(std::io::Error::other)?;
        file.write_all(b"\n")?;
    }
    let _ = lock_file.unlock();
    Ok(())
}

fn runtime_gateway_file_ledger_entry_id(entry: &RuntimeGatewayBillingLedgerEntry) -> String {
    runtime_gateway_billing_ledger_entry_identity(entry)
}

fn runtime_gateway_file_ledger_entry_ids(path: &Path) -> std::io::Result<BTreeSet<String>> {
    let mut reader = match open_gateway_ledger_reader(path)? {
        Some(reader) => reader,
        None => return Ok(BTreeSet::new()),
    };
    let mut ids = BTreeSet::new();
    for line in reader.by_ref().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(trimmed) {
            ids.insert(runtime_gateway_file_ledger_entry_id(&entry));
        }
    }
    ensure_gateway_ledger_reader_within_limit(path, &reader)?;
    Ok(ids)
}

fn open_gateway_ledger_reader(
    path: &Path,
) -> std::io::Result<Option<BufReader<std::io::Take<File>>>> {
    let Some(metadata) = gateway_ledger_metadata(path)? else {
        return Ok(None);
    };
    let file = File::open(path)?;
    if !runtime_gateway_same_file_metadata(&metadata, &file.metadata()?) {
        return Err(std::io::Error::other(format!(
            "gateway ledger path changed while opening {}",
            path.display()
        )));
    }
    Ok(Some(BufReader::new(file.take(
        RUNTIME_GATEWAY_LEDGER_FILE_MAX_BYTES.saturating_add(1),
    ))))
}

fn open_gateway_ledger_append_file(path: &Path) -> std::io::Result<File> {
    let existing_metadata = gateway_ledger_metadata(path)?;
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

fn open_gateway_ledger_temp_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(file) => Ok(file),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(path)?;
            options.open(path)
        }
        Err(err) => Err(err),
    }
}

fn gateway_ledger_metadata(path: &Path) -> std::io::Result<Option<std::fs::Metadata>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "refusing to access gateway ledger through symlink {}",
                path.display()
            ),
        ));
    }
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("gateway ledger path {} is not a file", path.display()),
        ));
    }
    if metadata.len() > RUNTIME_GATEWAY_LEDGER_FILE_MAX_BYTES {
        return Err(gateway_ledger_size_error(path));
    }
    Ok(Some(metadata))
}

fn ensure_gateway_ledger_reader_within_limit(
    path: &Path,
    reader: &BufReader<std::io::Take<File>>,
) -> std::io::Result<()> {
    if reader.get_ref().limit() == 0 {
        Err(gateway_ledger_size_error(path))
    } else {
        Ok(())
    }
}

fn gateway_ledger_size_error(path: &Path) -> std::io::Error {
    std::io::Error::other(format!(
        "gateway ledger path {} exceeds safe size limit ({} bytes)",
        path.display(),
        RUNTIME_GATEWAY_LEDGER_FILE_MAX_BYTES
    ))
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
    let changed =
        runtime_gateway_file_ledger_reconcile_response_locked(path, event, reconciled_at_epoch)?;
    let _ = lock_file.unlock();
    Ok(changed)
}

fn runtime_gateway_file_ledger_reconcile_response_locked(
    path: &Path,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) -> std::io::Result<bool> {
    let mut reader = match open_gateway_ledger_reader(path)? {
        Some(reader) => reader,
        None => return Ok(false),
    };
    let tmp_path = path.with_extension("jsonl.tmp");
    let mut output = open_gateway_ledger_temp_file(&tmp_path)?;
    let mut changed = false;
    for line in reader.by_ref().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(mut entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(trimmed)
        else {
            continue;
        };
        if entry.call_id != event.call_id
            || entry.phase != "request"
            || event
                .key_name
                .as_deref()
                .is_some_and(|key_name| !entry.key_name.eq_ignore_ascii_case(key_name))
            || event.tenant_id.as_deref() != entry.tenant_id.as_deref()
        {
            serde_json::to_writer(&mut output, &entry).map_err(std::io::Error::other)?;
            output.write_all(b"\n")?;
            continue;
        }
        runtime_gateway_apply_response_to_ledger_entry(&mut entry, event, reconciled_at_epoch);
        serde_json::to_writer(&mut output, &entry).map_err(std::io::Error::other)?;
        output.write_all(b"\n")?;
        changed = true;
    }
    ensure_gateway_ledger_reader_within_limit(path, &reader)?;
    if changed {
        output.sync_all()?;
        drop(output);
        std::fs::rename(tmp_path, path)?;
    } else {
        drop(output);
        let _ = std::fs::remove_file(tmp_path);
    }
    Ok(changed)
}

pub(super) fn runtime_gateway_file_ledger_load(
    path: &Path,
    limit: usize,
) -> std::io::Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let mut reader = match open_gateway_ledger_reader(path)? {
        Some(reader) => reader,
        None => return Ok(Vec::new()),
    };
    let mut entries = Vec::new();
    for line in reader.by_ref().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(trimmed) {
            entries.push(entry);
            if entries.len() > limit {
                entries.remove(0);
            }
        }
    }
    ensure_gateway_ledger_reader_within_limit(path, &reader)?;
    Ok(entries)
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
                typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                call_id: format!("prodex-{}", prodex_domain::CallId::new()),
                key_name: "alpha".to_string(),
                tenant_id: Some("tenant-a".to_string()),
                team_id: Some("platform".to_string()),
                project_id: None,
                user_id: None,
                budget_id: Some("budget-a".to_string()),
                model: "gpt-5".to_string(),
                minute_epoch: 10,
                input_tokens: 100,
                reserved_tokens: 100,
                estimated_cost_microusd: Some(250_000),
                created_at_epoch: 20,
            }],
        )
        .unwrap();

        let entries = runtime_gateway_file_ledger_load(&path, usize::MAX).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key_name, "alpha");
        assert_eq!(entries[0].tenant_id.as_deref(), Some("tenant-a"));
        assert_eq!(entries[0].team_id.as_deref(), Some("platform"));
        assert_eq!(entries[0].budget_id.as_deref(), Some("budget-a"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn append_file_ledger_dedupes_call_key_phase() {
        let root = temp_dir("dedupe");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let call_id = format!("prodex-{}", prodex_domain::CallId::new());
        let delta = RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id,
            key_name: "Alpha".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5".to_string(),
            minute_epoch: 10,
            input_tokens: 100,
            reserved_tokens: 100,
            estimated_cost_microusd: Some(250_000),
            created_at_epoch: 20,
        };

        let mut duplicate = delta.clone();
        duplicate.input_tokens = 200;

        runtime_gateway_file_ledger_append_deltas(&path, &[delta]).unwrap();
        runtime_gateway_file_ledger_append_deltas(&path, &[duplicate]).unwrap();

        let entries = runtime_gateway_file_ledger_load(&path, usize::MAX).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key_name, "Alpha");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn append_file_ledger_keeps_case_distinct_key_names() {
        let root = temp_dir("case-distinct");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let call_id = format!("prodex-{}", prodex_domain::CallId::new());
        let delta = RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id,
            key_name: "Alpha".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5".to_string(),
            minute_epoch: 10,
            input_tokens: 100,
            reserved_tokens: 100,
            estimated_cost_microusd: Some(250_000),
            created_at_epoch: 20,
        };
        let mut case_distinct = delta.clone();
        case_distinct.key_name = "alpha".to_string();

        runtime_gateway_file_ledger_append_deltas(&path, &[delta, case_distinct]).unwrap();

        let entries = runtime_gateway_file_ledger_load(&path, usize::MAX).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key_name, "Alpha");
        assert_eq!(entries[1].key_name, "alpha");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_ledger_entry_ids_streams_existing_jsonl_ids() {
        let root = temp_dir("entry-ids");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let mut entry =
            runtime_gateway_billing_ledger_entry_from_delta(&RuntimeGatewayVirtualKeyUsageDelta {
                request_id: 1,
                typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                call_id: "prodex-call".to_string(),
                key_name: "Alpha".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                model: "gpt-5".to_string(),
                minute_epoch: 10,
                input_tokens: 100,
                reserved_tokens: 100,
                estimated_cost_microusd: Some(250_000),
                created_at_epoch: 20,
            });
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file, "not-json").unwrap();
        serde_json::to_writer(&mut file, &entry).unwrap();
        file.write_all(b"\n").unwrap();
        entry.key_name = "Beta".to_string();
        serde_json::to_writer(&mut file, &entry).unwrap();
        file.write_all(b"\n").unwrap();

        let ids = runtime_gateway_file_ledger_entry_ids(&path).unwrap();

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&runtime_gateway_file_ledger_entry_id(&entry)));
        entry.key_name = "Alpha".to_string();
        assert!(ids.contains(&runtime_gateway_file_ledger_entry_id(&entry)));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_ledger_load_streams_tail_entries_with_limit() {
        let root = temp_dir("load-limit");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        for key_name in ["alpha", "beta", "gamma"] {
            let entry = runtime_gateway_billing_ledger_entry_from_delta(
                &RuntimeGatewayVirtualKeyUsageDelta {
                    request_id: 1,
                    typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                    call_id: format!("prodex-{}", prodex_domain::CallId::new()),
                    key_name: key_name.to_string(),
                    tenant_id: None,
                    team_id: None,
                    project_id: None,
                    user_id: None,
                    budget_id: None,
                    model: "gpt-5".to_string(),
                    minute_epoch: 10,
                    input_tokens: 100,
                    reserved_tokens: 100,
                    estimated_cost_microusd: Some(250_000),
                    created_at_epoch: 20,
                },
            );
            serde_json::to_writer(&mut file, &entry).unwrap();
            file.write_all(b"\n").unwrap();
        }
        writeln!(file, "not-json").unwrap();

        let entries = runtime_gateway_file_ledger_load(&path, 2).unwrap();

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.key_name.as_str())
                .collect::<Vec<_>>(),
            vec!["beta", "gamma"]
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_ledger_reconcile_streams_and_updates_matching_call_id() {
        let root = temp_dir("reconcile");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        for (call_id, key_name) in [("call-a", "alpha"), ("call-b", "beta")] {
            let entry = runtime_gateway_billing_ledger_entry_from_delta(
                &RuntimeGatewayVirtualKeyUsageDelta {
                    request_id: 1,
                    typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                    call_id: call_id.to_string(),
                    key_name: key_name.to_string(),
                    tenant_id: None,
                    team_id: None,
                    project_id: None,
                    user_id: None,
                    budget_id: None,
                    model: "gpt-5".to_string(),
                    minute_epoch: 10,
                    input_tokens: 100,
                    reserved_tokens: 100,
                    estimated_cost_microusd: Some(250_000),
                    created_at_epoch: 20,
                },
            );
            serde_json::to_writer(&mut file, &entry).unwrap();
            file.write_all(b"\n").unwrap();
        }
        writeln!(file, "not-json").unwrap();
        let event = RuntimeProviderGatewaySpendEvent {
            event: "gateway_spend",
            phase: "response",
            request: 1,
            key_name: Some("beta".to_string()),
            tenant_id: None,
            request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            legacy_request_sequence: 1,
            call_id: "call-b".to_string(),
            provider: "openai".to_string(),
            path: "/v1/responses".to_string(),
            model: "gpt-5".to_string(),
            status: 200,
            elapsed_ms: 12,
            request_bytes: 2,
            response_bytes: Some(4),
            input_tokens: Some(100),
            output_tokens: Some(25),
            cost_usd: Some(0.0001),
            reconciliation_reason: Some(prodex_domain::ReservationReconciliationReason::Completed),
            sink: "runtime-log".to_string(),
        };

        assert!(runtime_gateway_file_ledger_reconcile_response(&path, &event, 30).unwrap());

        let entries = runtime_gateway_file_ledger_load(&path, usize::MAX).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].call_id, "call-a");
        assert_eq!(entries[0].response_status, None);
        assert_eq!(entries[1].call_id, "call-b");
        assert_eq!(entries[1].response_status, Some(200));
        assert_eq!(entries[1].response_bytes, Some(4));
        assert_eq!(entries[1].output_tokens, Some(25));
        assert_eq!(entries[1].reconciled_at_epoch, Some(30));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_ledger_reconcile_requires_matching_tenant_scope() {
        let root = temp_dir("reconcile-tenant-scope");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        for tenant_id in ["tenant-a", "tenant-b"] {
            let entry = runtime_gateway_billing_ledger_entry_from_delta(
                &RuntimeGatewayVirtualKeyUsageDelta {
                    request_id: 1,
                    typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                    call_id: "call-shared".to_string(),
                    key_name: "alpha".to_string(),
                    tenant_id: Some(tenant_id.to_string()),
                    team_id: None,
                    project_id: None,
                    user_id: None,
                    budget_id: None,
                    model: "gpt-5".to_string(),
                    minute_epoch: 10,
                    input_tokens: 100,
                    reserved_tokens: 100,
                    estimated_cost_microusd: Some(250_000),
                    created_at_epoch: 20,
                },
            );
            serde_json::to_writer(&mut file, &entry).unwrap();
            file.write_all(b"\n").unwrap();
        }

        assert!(
            runtime_gateway_file_ledger_reconcile_response(
                &path,
                &RuntimeProviderGatewaySpendEvent {
                    event: "gateway_spend",
                    phase: "response",
                    request: 1,
                    key_name: Some("alpha".to_string()),
                    tenant_id: Some("tenant-b".to_string()),
                    request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                    legacy_request_sequence: 1,
                    call_id: "call-shared".to_string(),
                    provider: "openai".to_string(),
                    path: "/v1/responses".to_string(),
                    model: "gpt-5".to_string(),
                    status: 200,
                    elapsed_ms: 12,
                    request_bytes: 2,
                    response_bytes: Some(4),
                    input_tokens: Some(100),
                    output_tokens: Some(25),
                    cost_usd: Some(0.0001),
                    reconciliation_reason: Some(
                        prodex_domain::ReservationReconciliationReason::Completed,
                    ),
                    sink: "runtime-log".to_string(),
                },
                30,
            )
            .unwrap()
        );

        let entries = runtime_gateway_file_ledger_load(&path, usize::MAX).unwrap();
        assert_eq!(entries[0].tenant_id.as_deref(), Some("tenant-a"));
        assert_eq!(entries[0].response_status, None);
        assert_eq!(entries[1].tenant_id.as_deref(), Some("tenant-b"));
        assert_eq!(entries[1].response_status, Some(200));

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
                typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                call_id: format!("prodex-{}", prodex_domain::CallId::new()),
                key_name: "alpha".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                model: "gpt-5".to_string(),
                minute_epoch: 10,
                input_tokens: 100,
                reserved_tokens: 100,
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
        file.set_len(RUNTIME_GATEWAY_LEDGER_FILE_MAX_BYTES + 1)
            .unwrap();

        let err = runtime_gateway_file_ledger_load(&path, usize::MAX)
            .expect_err("oversized ledger should be rejected");

        assert!(err.to_string().contains("safe size limit"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
