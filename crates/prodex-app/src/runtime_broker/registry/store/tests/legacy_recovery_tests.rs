use super::*;
use std::fs;

fn write_oversized_file(path: &std::path::Path) {
    fs::File::create(path)
        .unwrap()
        .set_len(crate::runtime_store::RUNTIME_STORE_JSON_MAX_BYTES + 1)
        .unwrap();
}

#[test]
fn truncated_primary_recovers_a_valid_last_good_registry() {
    let paths = test_paths("truncated-primary");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    let backup = serde_json::to_vec(&test_registry("backup-instance")).unwrap();
    fs::write(&registry_path, br#"{"admin_token":"truncated"#).unwrap();
    fs::write(&backup_path, &backup).unwrap();

    let loaded = load_runtime_broker_registry(&paths, broker_key)
        .unwrap()
        .unwrap();

    assert_eq!(loaded.instance_id, "backup-instance");
    assert_eq!(fs::read(&registry_path).unwrap(), backup);
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn malformed_backup_is_an_error_instead_of_empty_success() {
    let paths = test_paths("malformed-backup");
    let broker_key = "broker";
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    fs::write(&backup_path, br#"{"admin_token":"truncated"#).unwrap();

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("malformed backup must remain an error");

    assert!(format!("{error:#}").contains("last-good"));
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn malformed_primary_without_backup_is_an_error_instead_of_empty_success() {
    let paths = test_paths("malformed-primary");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    fs::write(&registry_path, br#"{"instance_token":"truncated"#).unwrap();

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("malformed primary without backup must remain an error");

    assert!(format!("{error:#}").contains("runtime-broker-broker.json"));
    assert!(registry_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn marker_only_legacy_data_is_an_error_instead_of_empty_success() {
    let paths = test_paths("marker-only");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    fs::write(&registry_path, br#"{"admin_token":"legacy-admin"}"#).unwrap();

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("marker-only legacy data must remain an error");

    assert!(format!("{error:#}").contains("runtime-broker-broker.json"));
    assert!(registry_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn marker_only_primary_recovers_a_valid_last_good_registry() {
    let paths = test_paths("marker-only-recovery");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    let backup = serde_json::to_vec(&test_registry("backup-instance")).unwrap();
    fs::write(&registry_path, br#"{"admin_token":"legacy-admin"}"#).unwrap();
    fs::write(&backup_path, &backup).unwrap();

    let loaded = load_runtime_broker_registry(&paths, broker_key)
        .unwrap()
        .unwrap();

    assert_eq!(loaded.instance_id, "backup-instance");
    assert_eq!(fs::read(&registry_path).unwrap(), backup);
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn malformed_marker_like_current_record_recovers_a_valid_last_good_registry() {
    let paths = test_paths("malformed-marker-like-recovery");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    let mut marker_like = serde_json::to_value(test_registry("marker-like")).unwrap();
    marker_like
        .as_object_mut()
        .unwrap()
        .insert("admin_token".to_string(), serde_json::json!("legacy-admin"));
    let backup = serde_json::to_vec(&test_registry("backup-instance")).unwrap();
    fs::write(&registry_path, serde_json::to_vec(&marker_like).unwrap()).unwrap();
    fs::write(&backup_path, &backup).unwrap();

    let loaded = load_runtime_broker_registry(&paths, broker_key)
        .unwrap()
        .unwrap();

    assert_eq!(loaded.instance_id, "backup-instance");
    assert_eq!(fs::read(&registry_path).unwrap(), backup);
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn malformed_marker_like_current_record_preserves_legacy_backup() {
    let paths = test_paths("malformed-marker-like-legacy-backup");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    let mut marker_like = serde_json::to_value(test_registry("marker-like")).unwrap();
    marker_like
        .as_object_mut()
        .unwrap()
        .insert("admin_token".to_string(), serde_json::json!("legacy-admin"));
    fs::write(&registry_path, serde_json::to_vec(&marker_like).unwrap()).unwrap();
    fs::write(
        &backup_path,
        br#"{"instance_token":"legacy-instance","admin_token":"legacy-admin"}"#,
    )
    .unwrap();

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("malformed marker-like current data must not be accepted");

    assert!(format!("{error:#}").contains("last-good"));
    assert!(registry_path.exists());
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn malformed_marker_like_current_record_preserves_invalid_backup_provenance() {
    let paths = test_paths("malformed-marker-like-invalid-backup");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    let mut marker_like = serde_json::to_value(test_registry("marker-like")).unwrap();
    marker_like
        .as_object_mut()
        .unwrap()
        .insert("admin_token".to_string(), serde_json::json!("legacy-admin"));
    fs::write(&registry_path, serde_json::to_vec(&marker_like).unwrap()).unwrap();
    fs::write(&backup_path, br#"{broken"#).unwrap();

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("malformed marker-like current data must be rejected");
    let rendered = format!("{error:#}");

    assert!(rendered.contains("legacy secret fields"));
    assert!(rendered.contains("last-good"));
    assert!(registry_path.exists());
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn oversized_registry_without_backup_is_an_error_instead_of_empty_success() {
    let paths = test_paths("oversized");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    write_oversized_file(&registry_path);

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("oversized primary must remain an error");

    assert!(format!("{error:#}").contains("safe size limit"));
    assert_eq!(
        fs::metadata(&registry_path).unwrap().len(),
        crate::runtime_store::RUNTIME_STORE_JSON_MAX_BYTES + 1
    );
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn oversized_primary_recovers_a_valid_last_good_registry() {
    let paths = test_paths("oversized-primary-recovery");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    let backup = serde_json::to_vec(&test_registry("backup-instance")).unwrap();
    write_oversized_file(&registry_path);
    fs::write(&backup_path, &backup).unwrap();

    let loaded = load_runtime_broker_registry(&paths, broker_key)
        .unwrap()
        .unwrap();

    assert_eq!(loaded.instance_id, "backup-instance");
    assert_eq!(fs::read(&registry_path).unwrap(), backup);
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn valid_primary_survives_an_oversized_last_good_registry() {
    let paths = test_paths("oversized-backup");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    let current = test_registry("current-instance");
    fs::write(&registry_path, serde_json::to_vec(&current).unwrap()).unwrap();
    write_oversized_file(&backup_path);

    let loaded = load_runtime_broker_registry(&paths, broker_key)
        .unwrap()
        .unwrap();

    assert_eq!(loaded, current);
    assert!(registry_path.exists());
    assert_eq!(
        fs::metadata(&backup_path).unwrap().len(),
        crate::runtime_store::RUNTIME_STORE_JSON_MAX_BYTES + 1
    );
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn oversized_primary_and_invalid_backup_preserve_error_provenance() {
    let paths = test_paths("oversized-primary-invalid-backup");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    write_oversized_file(&registry_path);
    fs::write(&backup_path, br#"{broken"#).unwrap();

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("oversized primary and invalid backup must fail closed");
    let rendered = format!("{error:#}");

    assert!(rendered.contains("safe size limit"));
    assert!(rendered.contains("runtime-broker-broker.json"));
    assert!(rendered.contains("last-good"));
    assert!(registry_path.exists());
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn oversized_registry_pair_is_not_treated_as_legacy() {
    let paths = test_paths("oversized-pair");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
    write_oversized_file(&registry_path);
    write_oversized_file(&backup_path);

    let error = load_runtime_broker_registry(&paths, broker_key)
        .expect_err("oversized registry pair must fail closed");

    assert!(format!("{error:#}").contains("safe size limit"));
    assert!(registry_path.exists());
    assert!(backup_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn legacy_cleanup_with_keyring_backend_has_no_capability_file() {
    let _env_lock = crate::TestEnvVarGuard::lock();
    let _backend = crate::TestEnvVarGuard::set(crate::PRODEX_SECRET_BACKEND_ENV, "keyring");
    let _service =
        crate::TestEnvVarGuard::set(crate::PRODEX_SECRET_KEYRING_SERVICE_ENV, "prodex-test");
    let paths = test_paths("legacy-keyring");
    let broker_key = "broker";
    let legacy = br#"{
        "instance_token":"legacy-instance",
        "admin_token":"legacy-admin"
    }"#;
    fs::write(
        runtime_broker_registry_file_path(&paths, broker_key),
        legacy,
    )
    .unwrap();
    fs::write(
        runtime_broker_registry_last_good_file_path(&paths, broker_key),
        legacy,
    )
    .unwrap();

    assert!(
        load_runtime_broker_registry(&paths, broker_key)
            .unwrap()
            .is_none()
    );
    assert!(!runtime_broker_capability_file_path(&paths, broker_key).exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn marker_like_text_inside_a_current_value_does_not_remove_registry() {
    let paths = test_paths("marker-value");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    let mut registry = test_registry("current-instance");
    registry.current_profile = r#"value "admin_token" and "instance_token""#.to_string();
    fs::write(&registry_path, serde_json::to_vec(&registry).unwrap()).unwrap();

    let loaded = load_runtime_broker_registry(&paths, broker_key)
        .unwrap()
        .unwrap();

    assert_eq!(loaded, registry);
    assert!(registry_path.exists());
    let _ = fs::remove_dir_all(paths.root);
}

#[cfg(unix)]
#[test]
fn broken_registry_symlink_is_not_empty_success() {
    use std::os::unix::fs::symlink;

    let paths = test_paths("broken-symlink");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    symlink(paths.root.join("missing-registry"), &registry_path).unwrap();

    assert!(load_runtime_broker_registry(&paths, broker_key).is_err());
    assert!(registry_path.is_symlink());
    let _ = fs::remove_dir_all(paths.root);
}

#[cfg(unix)]
#[test]
fn unreadable_registry_is_not_empty_success() {
    use std::os::unix::fs::PermissionsExt;

    let paths = test_paths("unreadable");
    let broker_key = "broker";
    let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
    fs::write(
        &registry_path,
        serde_json::to_vec(&test_registry("unreadable-instance")).unwrap(),
    )
    .unwrap();
    fs::set_permissions(&registry_path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::File::open(&registry_path).is_ok() {
        fs::set_permissions(&registry_path, fs::Permissions::from_mode(0o600)).unwrap();
        let _ = fs::remove_dir_all(paths.root);
        return;
    }

    assert!(load_runtime_broker_registry(&paths, broker_key).is_err());
    assert!(registry_path.exists());
    fs::set_permissions(&registry_path, fs::Permissions::from_mode(0o600)).unwrap();
    let _ = fs::remove_dir_all(paths.root);
}
