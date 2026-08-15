use super::{
    acquire_json_file_lock, load_json_file_with_backup, parse_versioned_json_or_raw,
    read_versioned_json_file_with_backup, runtime_sidecar_generation_from_disk,
    write_private_file_atomic,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn last_good_recovery_repairs_missing_or_corrupt_primary() {
    for (name, primary) in [("missing", None), ("corrupt", Some("{broken"))] {
        let root = temp_root(name);
        let path = root.join("state.json");
        let backup_path = root.join("state.last-good.json");
        let backup = r#"{"source":"last-good"}"#;
        fs::write(&backup_path, backup).expect("backup should be writable");
        if let Some(primary) = primary {
            fs::write(&path, primary).expect("primary should be writable");
        }

        let loaded = load_json_file_with_backup::<serde_json::Value>(&path, &backup_path)
            .expect("valid backup should recover primary");

        assert!(loaded.recovered_from_backup);
        assert_eq!(loaded.value["source"], "last-good");
        assert_eq!(fs::read_to_string(&path).unwrap(), backup);
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn malformed_versioned_sidecar_does_not_fallback_to_raw_json() {
    let error = parse_versioned_json_or_raw::<serde_json::Value>(r#"{"generation":7}"#)
        .expect_err("missing versioned value must remain invalid");

    assert!(error.to_string().contains("versioned runtime sidecar"));
}

#[test]
fn malformed_versioned_sidecar_recovers_from_last_good() {
    let root = temp_root("malformed-versioned-recovery");
    let path = root.join("runtime.json");
    let backup_path = root.join("runtime.json.last-good");
    fs::write(&path, r#"{"generation":9}"#).expect("malformed primary should write");
    fs::write(
        &backup_path,
        r#"{"generation":8,"value":{"source":"last-good"}}"#,
    )
    .expect("valid backup should write");

    let loaded = read_versioned_json_file_with_backup::<serde_json::Value>(&path, &backup_path)
        .expect("valid backup should recover malformed primary");

    assert!(loaded.recovered_from_backup);
    assert_eq!(loaded.generation, 8);
    assert_eq!(loaded.value["source"], "last-good");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        fs::read_to_string(&backup_path).unwrap()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sidecar_generation_recovery_rejects_malformed_primary_envelope() {
    let root = temp_root("malformed-generation-recovery");
    let path = root.join("runtime.json");
    let backup_path = root.join("runtime.json.last-good");
    fs::write(&path, r#"{"generation":9}"#).expect("malformed primary should write");
    let backup = r#"{"generation":8,"value":{"source":"last-good"}}"#;
    fs::write(&backup_path, backup).expect("valid backup should write");

    assert_eq!(
        runtime_sidecar_generation_from_disk(&path, &backup_path)
            .expect("valid backup generation should recover"),
        8
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), backup);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn backup_recovery_waits_for_file_lock_before_repair() {
    let root = temp_root("recovery-lock");
    let path = root.join("state.json");
    let backup_path = root.join("state.last-good.json");
    fs::write(&path, "{broken").expect("primary should be writable");
    fs::write(&backup_path, r#"{"source":"old"}"#).expect("backup should be writable");

    let lock = acquire_json_file_lock(&path).expect("test should hold the file lock");
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (loaded_tx, loaded_rx) = std::sync::mpsc::channel();
    let reader_path = path.clone();
    let reader_backup_path = backup_path.clone();
    let reader = std::thread::spawn(move || {
        started_tx.send(()).expect("reader should start");
        let loaded =
            load_json_file_with_backup::<serde_json::Value>(&reader_path, &reader_backup_path)
                .expect("backup should recover after the lock is released");
        loaded_tx
            .send((loaded.recovered_from_backup, loaded.value["source"].clone()))
            .expect("reader result should be delivered");
    });
    started_rx.recv().expect("reader should be scheduled");
    assert!(
        loaded_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );

    let new = r#"{"source":"new"}"#;
    write_private_file_atomic(&path, new.as_bytes()).expect("new primary should be durable");
    write_private_file_atomic(&backup_path, new.as_bytes()).expect("new backup should be durable");
    drop(lock);

    let (recovered_from_backup, source) = loaded_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("reader should finish after the lock is released");
    reader.join().expect("reader should not panic");
    assert!(!recovered_from_backup);
    assert_eq!(source, "new");
    let _ = fs::remove_dir_all(root);
}
