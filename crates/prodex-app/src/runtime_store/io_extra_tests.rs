use super::*;
use crate::TestEnvVarGuard;

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-runtime-store-{name}-{}-{nanos}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("test root should be created");
    root
}

#[test]
fn sidecar_generation_recovery_repairs_corrupt_primary() {
    let root = temp_root("generation-recovery");
    let path = root.join("runtime.json");
    let backup_path = root.join("runtime.json.last-good");
    let backup = r#"{"generation":7,"value":{"source":"last-good"}}"#;
    fs::write(&path, "{broken").expect("primary should be writable");
    fs::write(&backup_path, backup).expect("backup should be writable");

    assert_eq!(
        runtime_sidecar_generation_from_disk(&path, &backup_path).unwrap(),
        7
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), backup);
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cached_generation_with_missing_sidecars_does_not_relock() {
    let root = temp_root("cached-generation-missing-sidecars");
    let path = root.join("runtime.json");
    let backup_path = root.join("runtime.json.last-good");
    let value = serde_json::json!({"source": "initial"});
    save_versioned_json_file_with_fence(&path, &backup_path, &value)
        .expect("initial sidecar should save");
    fs::remove_file(&path).expect("primary should be removed");
    fs::remove_file(&backup_path).expect("last-good should be removed");

    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let worker_path = path.clone();
    let worker_backup_path = backup_path.clone();
    let worker = std::thread::spawn(move || {
        let result = save_versioned_json_file_with_fence(
            &worker_path,
            &worker_backup_path,
            &serde_json::json!({"source": "recreated"}),
        )
        .map_err(|error| error.to_string());
        result_tx.send(result).expect("save result should be sent");
    });
    result_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("cached-generation recovery should not deadlock")
        .expect("sidecar should save after both files disappear");
    worker.join().expect("save worker should not panic");

    assert_eq!(
        runtime_sidecar_generation_from_disk(&path, &backup_path)
            .expect("recreated sidecar should be readable"),
        1
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn versioned_sidecar_cleanup_waits_for_primary_lock() {
    let root = temp_root("sidecar-cleanup-lock");
    let path = root.join("runtime.json");
    let backup_path = root.join("runtime.json.last-good");
    save_versioned_json_file_with_fence(&path, &backup_path, &serde_json::json!({"ok": true}))
        .expect("sidecar should save");
    let lock = acquire_json_file_lock(&path).expect("test should hold the sidecar lock");

    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let worker_root = root.clone();
    let worker_path = path.clone();
    let worker_backup_path = backup_path.clone();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).expect("cleanup should start");
        let result = remove_versioned_json_file_with_backup_under(
            &worker_root,
            &worker_path,
            &worker_backup_path,
        )
        .map_err(|error| error.to_string());
        result_tx
            .send(result)
            .expect("cleanup result should be sent");
    });
    started_rx
        .recv()
        .expect("cleanup worker should be scheduled");
    assert!(
        result_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "cleanup should wait for the primary sidecar lock"
    );

    drop(lock);
    let report = result_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("cleanup should finish after lock release")
        .expect("cleanup should remove sidecar pair");
    assert_eq!(report.removed, 2);
    assert!(report.failures.is_empty());
    worker.join().expect("cleanup worker should not panic");
    assert!(!path.exists());
    assert!(!backup_path.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn versioned_sidecar_cleanup_keeps_backup_when_primary_removal_fails() {
    let root = temp_root("sidecar-cleanup-primary-failure");
    let path = root.join("runtime.json");
    let backup_path = root.join("runtime.json.last-good");
    fs::create_dir(&path).expect("primary directory should be created");
    fs::write(&backup_path, "last-good").expect("backup should be writable");

    let report = remove_versioned_json_file_with_backup_under(&root, &path, &backup_path)
        .expect("cleanup should report deletion failures");

    assert_eq!(report.removed, 0);
    assert_eq!(report.missing, 0);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].path, path);
    assert!(path.is_dir());
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(root);
}

fn reset_runtime_store_fault_budget(env_key: &'static str) {
    let _env_lock = TestEnvVarGuard::lock();
    let _ = runtime_take_fault_injection_budget(env_key, 0);
}

fn assert_last_good_recovery_keeps_backup_on_fault(env_key: &'static str, primary: Option<&str>) {
    reset_runtime_store_fault_budget(env_key);
    let root = temp_root("recovery-fault");
    let path = root.join("state.json");
    let backup_path = root.join("state.last-good.json");
    let backup = r#"{"source":"last-good"}"#;
    if let Some(primary) = primary {
        fs::write(&path, primary).expect("primary should be writable");
    }
    fs::write(&backup_path, backup).expect("backup should be writable");

    let loaded = {
        let _fault = TestEnvVarGuard::set(env_key, "1");
        load_json_file_with_backup::<serde_json::Value>(&path, &backup_path)
            .expect("valid backup should remain usable")
    };

    assert!(loaded.recovered_from_backup);
    assert_eq!(loaded.value["source"], "last-good");
    match primary {
        Some(primary) => assert_eq!(fs::read_to_string(&path).unwrap(), primary),
        None => assert!(!path.exists()),
    }
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
    reset_runtime_store_fault_budget(env_key);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn last_good_recovery_keeps_backup_when_primary_write_fails() {
    assert_last_good_recovery_keeps_backup_on_fault(
        TEST_RUNTIME_STORE_WRITE_FAULT_ENV,
        Some("{broken"),
    );
}

#[test]
fn last_good_recovery_keeps_backup_when_primary_rename_fails() {
    assert_last_good_recovery_keeps_backup_on_fault(
        TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV,
        Some("{broken"),
    );
}

#[test]
fn last_good_recovery_keeps_backup_when_primary_is_missing_and_repair_fails() {
    for env_key in [
        TEST_RUNTIME_STORE_WRITE_FAULT_ENV,
        TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV,
    ] {
        assert_last_good_recovery_keeps_backup_on_fault(env_key, None);
    }
}

#[test]
fn sidecar_migration_keeps_last_good_when_missing_primary_repair_fails() {
    for env_key in [
        TEST_RUNTIME_STORE_WRITE_FAULT_ENV,
        TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV,
    ] {
        reset_runtime_store_fault_budget(env_key);
        let root = temp_root("generation-recovery-fault");
        let path = root.join("runtime.json");
        let backup_path = root.join("runtime.json.last-good");
        let backup = r#"{"generation":7,"value":{"source":"last-good"}}"#;
        fs::write(&backup_path, backup).expect("backup should be writable");
        let generation = {
            let _fault = TestEnvVarGuard::set(env_key, "1");
            runtime_sidecar_generation_from_disk(&path, &backup_path)
                .expect("valid last-good generation should remain usable")
        };

        assert_eq!(generation, 7);
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
        reset_runtime_store_fault_budget(env_key);
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn write_json_file_with_backup_keeps_backup_when_primary_write_fails() {
    reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_WRITE_FAULT_ENV);
    let root = temp_root("primary-write-fault");
    let path = root.join("state.json");
    let backup_path = root.join("state.last-good.json");
    let backup = r#"{"source":"last-good"}"#;
    fs::write(&backup_path, backup).expect("backup should be writable");
    let err = {
        let _fault = TestEnvVarGuard::set(TEST_RUNTIME_STORE_WRITE_FAULT_ENV, "1");
        write_json_file_with_backup(&path, &backup_path, r#"{"source":"new"}"#, |content| {
            let _: serde_json::Value = serde_json::from_str(content)?;
            Ok(())
        })
        .expect_err("injected primary write should fail")
    };

    assert!(err.to_string().contains("atomically write"));
    assert!(!path.exists());
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
    reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_WRITE_FAULT_ENV);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn write_json_file_with_backup_keeps_backup_when_primary_rename_fails() {
    reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV);
    let root = temp_root("primary-rename-fault");
    let path = root.join("state.json");
    let backup_path = root.join("state.last-good.json");
    let old = r#"{"source":"old"}"#;
    let new = r#"{"source":"new"}"#;
    fs::write(&path, old).expect("primary should be writable");
    fs::write(&backup_path, old).expect("backup should be writable");
    let err = {
        let _fault = TestEnvVarGuard::set(TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV, "1");
        write_json_file_with_backup(&path, &backup_path, new, |content| {
            let _: serde_json::Value = serde_json::from_str(content)?;
            Ok(())
        })
        .expect_err("injected primary rename should fail")
    };

    assert!(err.to_string().contains("atomically write"));
    assert_eq!(fs::read_to_string(&path).unwrap(), old);
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), old);
    reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn write_json_file_with_backup_updates_backup_only_after_primary_is_durable() {
    reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV);
    let root = temp_root("sidecar-rename-fault");
    let path = root.join("state.json");
    let backup_path = root.join("state.last-good.json");
    let old = r#"{"source":"old"}"#;
    let new = r#"{"source":"new"}"#;
    fs::write(&path, old).expect("primary should be writable");
    fs::write(&backup_path, old).expect("backup should be writable");
    let err = {
        let _fault = TestEnvVarGuard::set(TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV, "1");
        write_json_file_with_backup(&path, &backup_path, new, |content| {
            let _: serde_json::Value = serde_json::from_str(content)?;
            Ok(())
        })
        .expect_err("injected sidecar rename should fail")
    };

    assert!(err.to_string().contains("refresh"));
    assert_eq!(fs::read_to_string(&path).unwrap(), new);
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), old);
    reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn write_json_file_with_backup_restricts_primary_and_backup_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("permissions");
    let path = root.join("state.json");
    let backup_path = root.join("state.last-good.json");

    write_json_file_with_backup(&path, &backup_path, r#"{"ok":true}"#, |content| {
        let _: serde_json::Value = serde_json::from_str(content).context("json should parse")?;
        Ok(())
    })
    .expect("json should be written");

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&backup_path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn read_json_file_to_string_rejects_oversized_file_before_reading() {
    let root = temp_root("oversized-read");
    let path = root.join("state.json");
    fs::File::create(&path)
        .expect("state file should be created")
        .set_len(RUNTIME_STORE_JSON_MAX_BYTES + 1)
        .expect("state file size should be set");

    let err = read_json_file_to_string(&path).expect_err("oversized state should be rejected");

    assert!(err.to_string().contains("exceeds safe size limit"));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn read_json_file_to_string_rejects_symlink() {
    let root = temp_root("symlink-read");
    let target = root.join("target.json");
    let link = root.join("state.json");
    fs::write(&target, "{}").expect("target should write");
    std::os::unix::fs::symlink(&target, &link).expect("symlink should be created");

    let err = read_json_file_to_string(&link).expect_err("symlink should be rejected");

    assert!(
        err.to_string()
            .contains("refusing to read json through symlink")
    );
    let _ = fs::remove_dir_all(root);
}
