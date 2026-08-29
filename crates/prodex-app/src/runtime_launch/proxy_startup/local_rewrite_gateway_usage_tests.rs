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
        runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path).unwrap();
    assert_eq!(migrated["team-a"].requests_total, 20);

    runtime_gateway_file_ledger_append_deltas(&ledger_path, std::slice::from_ref(&new_delta))
        .unwrap();

    runtime_gateway_virtual_key_usage_apply_deltas(&state_store, std::slice::from_ref(&new_delta))
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
    runtime_gateway_write_file_atomic(&ledger_path, "jsonl.tmp", |file| file.write_all(&payload))
        .unwrap();

    let err = runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path)
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("prefix does not match"));
    assert_eq!(std::fs::read(&usage_path).unwrap(), original_usage);

    std::fs::remove_dir_all(root).unwrap();
}
