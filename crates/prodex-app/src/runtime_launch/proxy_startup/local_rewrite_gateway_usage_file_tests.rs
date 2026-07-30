use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-gateway-usage-file-{name}-{stamp}"))
}

#[test]
fn file_usage_rebuild_keeps_zero_state_unmaterialized() {
    let root = temp_dir("zero-state");
    std::fs::create_dir_all(&root).unwrap();
    let usage_path = root.join("usage.json");
    let ledger_path = root.join("ledger.jsonl");

    let usage =
        runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path).unwrap();

    assert!(usage.is_empty());
    assert!(!usage_path.exists());
    assert!(!ledger_path.exists());
    assert!(!usage_path.with_extension("ledger-baseline.json").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn file_usage_rebuild_rejects_directory_ledger_before_writes() {
    let root = temp_dir("directory-ledger");
    std::fs::create_dir_all(&root).unwrap();
    let usage_path = root.join("usage.json");
    let ledger_path = root.join("ledger.jsonl");
    std::fs::create_dir(&ledger_path).unwrap();

    let err = runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path)
        .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(!usage_path.exists());
    assert!(!usage_path.with_extension("ledger-baseline.json").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn file_usage_rebuild_rejects_symlink_ledger_before_writes() {
    let root = temp_dir("symlink-ledger");
    std::fs::create_dir_all(&root).unwrap();
    let usage_path = root.join("usage.json");
    let ledger_path = root.join("ledger.jsonl");
    let target = root.join("target.jsonl");
    std::fs::write(&target, "do not touch\n").unwrap();
    std::os::unix::fs::symlink(&target, &ledger_path).unwrap();

    let err = runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path)
        .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "do not touch\n");
    assert!(!usage_path.exists());
    assert!(!usage_path.with_extension("ledger-baseline.json").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn file_usage_rebuild_recovers_missing_snapshot_from_ledger() {
    let root = temp_dir("missing-snapshot");
    std::fs::create_dir_all(&root).unwrap();
    let usage_path = root.join("usage.json");
    let ledger_path = root.join("ledger.jsonl");
    let delta = RuntimeGatewayVirtualKeyUsageDelta {
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
    runtime_gateway_file_ledger_append_deltas(&ledger_path, &[delta]).unwrap();

    let usage =
        runtime_gateway_virtual_key_usage_file_rebuild_strict(&usage_path, &ledger_path).unwrap();

    assert_eq!(usage["team-a"].requests_total, 1);
    assert_eq!(usage["team-a"].tokens_this_minute, 7);
    assert!(usage_path.is_file());
    assert!(usage_path.with_extension("ledger-baseline.json").is_file());
    std::fs::remove_dir_all(root).unwrap();
}
