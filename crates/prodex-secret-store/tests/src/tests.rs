use crate::{
    FileSecretBackend, KeyringSecretBackend, RefreshLeaseBypassReason, RefreshLeaseCoordinator,
    RefreshLeaseDecision, RefreshLeaseError, RefreshLeaseErrorStatus, RefreshLeaseRole,
    SecretBackend, SecretBackendKind, SecretBackendSelection, SecretError, SecretLocation,
    SecretManager, SecretRevision, SecretStoreErrorStatus, SecretValue, auth_json_location,
    auth_json_location_for_backend, auth_json_path, describe_secret_location,
    plan_refresh_lease_error_response, plan_secret_error_response,
};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "prodex-secret-store-{name}-{}-{nanos:x}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn auth_json_location_maps_to_expected_path() {
    let home = PathBuf::from("/tmp/codex-home");
    assert_eq!(auth_json_path(&home), home.join("auth.json"));
    assert_eq!(
        auth_json_location(&home),
        SecretLocation::File(home.join("auth.json"))
    );
}

#[test]
fn file_backend_round_trips_text_values() {
    let root = temp_dir("text");
    let path = root.join("nested/auth.json");
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    store
        .write_text(&location, "{\"access_token\":\"abc\"}")
        .unwrap();

    assert_eq!(
        store.read_text(&location).unwrap().as_deref(),
        Some("{\"access_token\":\"abc\"}")
    );
    assert_eq!(
        store.read(&location).unwrap(),
        Some(SecretValue::Text("{\"access_token\":\"abc\"}".to_string()))
    );

    store.delete(&location).unwrap();
    assert_eq!(store.read(&location).unwrap(), None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_preserves_binary_values() {
    let root = temp_dir("binary");
    let path = root.join("secret.bin");
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);
    let payload = SecretValue::bytes(vec![0xff, 0x00, 0x41]);

    store.write(&location, payload.clone()).unwrap();

    assert_eq!(store.read(&location).unwrap(), Some(payload));
    assert!(store.read_text(&location).is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_rejects_oversized_secret_reads() {
    let root = temp_dir("oversized-read");
    let path = root.join("auth.json");
    let file = fs::File::create(&path).unwrap();
    file.set_len(1024 * 1024 + 1).unwrap();
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    let err = store.read(&location).unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));
    assert!(err.to_string().contains("exceeds safe size limit"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_rejects_oversized_secret_writes() {
    let root = temp_dir("oversized-write");
    let path = root.join("auth.json");
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    let err = store
        .write(&location, SecretValue::bytes(vec![b'x'; 1024 * 1024 + 1]))
        .unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));
    assert!(err.to_string().contains("exceeds safe size limit"));
    assert!(!path.exists());

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn file_backend_rejects_symlink_secret_reads() {
    let root = temp_dir("symlink-read");
    let target = root.join("target.json");
    let path = root.join("auth.json");
    fs::write(&target, "{\"access_token\":\"leaked\"}").unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    assert!(matches!(
        store.read_text(&location).unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));
    assert!(matches!(
        store.probe_revision(&location).unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn file_backend_writes_secret_files_with_private_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("private-permissions");
    let path = root.join("nested/auth.json");
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    store
        .write_text(&location, "{\"access_token\":\"abc\"}")
        .unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_rejects_keyring_locations() {
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::keyring("prodex", "auth");
    let err = store.write_text(&location, "value").unwrap_err();
    assert!(matches!(err, SecretError::UnsupportedLocation { .. }));
}

#[test]
fn file_backend_rejects_empty_file_path() {
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(PathBuf::new());

    let err = store.read(&location).unwrap_err();

    assert!(matches!(err, SecretError::InvalidLocation { .. }));
}

#[test]
fn keyring_backend_validates_service_name() {
    let backend = KeyringSecretBackend::new("prodex").unwrap();
    assert_eq!(backend.service(), "prodex");
    assert!(!backend.is_supported());
    assert!(backend.unsupported_reason().contains("not implemented"));

    let err = KeyringSecretBackend::new("   ").unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));

    let err = KeyringSecretBackend::new(" prodex ").unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));
}

#[test]
fn keyring_backend_rejects_empty_location_account() {
    let backend = KeyringSecretBackend::new("prodex").unwrap();
    let location = SecretLocation::keyring("prodex", "");

    let err = backend.read(&location).unwrap_err();

    assert!(matches!(err, SecretError::InvalidLocation { .. }));
}

#[test]
fn selectable_backend_file_round_trips_text_values() {
    let root = temp_dir("selection-text");
    let path = root.join("nested/auth.json");
    let store = SecretBackendSelection::file().into_manager();
    let location = SecretLocation::file(&path);

    store
        .write_text(&location, "{\"access_token\":\"abc\"}")
        .unwrap();

    assert_eq!(
        store.read_text(&location).unwrap().as_deref(),
        Some("{\"access_token\":\"abc\"}")
    );
    assert_eq!(
        store.read(&location).unwrap(),
        Some(SecretValue::Text("{\"access_token\":\"abc\"}".to_string()))
    );

    store.delete(&location).unwrap();
    assert_eq!(store.read(&location).unwrap(), None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn selectable_backend_from_kind_requires_keyring_service() {
    assert_eq!(
        SecretBackendSelection::from_kind(SecretBackendKind::File, None)
            .unwrap()
            .kind(),
        SecretBackendKind::File
    );

    let err = SecretBackendSelection::from_kind(SecretBackendKind::Keyring, None).unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));
}

#[test]
fn secret_backend_kind_rejects_padded_values() {
    let err = " keyring ".parse::<SecretBackendKind>().unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));
}

#[test]
fn secret_error_response_is_stable_and_redacted() {
    let error = SecretError::Io {
        path: PathBuf::from("/tmp/prodex/auth.json"),
        reason: "permission denied".to_string(),
    };
    assert!(!error.to_string().contains("/tmp"));
    assert!(!error.to_string().contains("auth.json"));
    assert!(!error.to_string().contains("permission"));

    let unsupported = SecretError::UnsupportedLocation {
        location: "keyring://prodex/auth-json:/tmp/codex-home".to_string(),
    };
    assert!(!unsupported.to_string().contains("keyring://"));
    assert!(!unsupported.to_string().contains("/tmp/codex-home"));

    let invalid = SecretError::InvalidLocation {
        reason: "unknown secret backend 'raw-prod-token'".to_string(),
    };
    assert!(!invalid.to_string().contains("raw-prod-token"));

    let response = plan_secret_error_response(&error);

    assert_eq!(response.status, SecretStoreErrorStatus::ServiceUnavailable);
    assert_eq!(response.code, "secret_store_unavailable");
    assert_eq!(
        response.message,
        "secret storage is temporarily unavailable"
    );
    assert!(!response.message.contains("/tmp"));
    assert!(!response.message.contains("auth.json"));
    assert!(!response.message.contains("permission"));
}

#[test]
fn refresh_lease_error_response_is_stable_and_redacted() {
    let error = RefreshLeaseError::Io {
        path: PathBuf::from("/tmp/prodex/refresh-token-secret.lock"),
        reason: "permission denied".to_string(),
    };
    assert!(!error.to_string().contains("/tmp"));
    assert!(!error.to_string().contains("refresh-token-secret"));
    assert!(!error.to_string().contains("permission"));

    let response = plan_refresh_lease_error_response(&error);

    assert_eq!(response.status, RefreshLeaseErrorStatus::ServiceUnavailable);
    assert_eq!(response.code, "refresh_lease_unavailable");
    assert_eq!(
        response.message,
        "refresh lease coordination is temporarily unavailable"
    );
    assert!(!response.message.contains("/tmp"));
    assert!(!response.message.contains("refresh-token-secret"));
    assert!(!response.message.contains("permission"));
}

#[test]
fn file_backend_probe_revision_tracks_metadata() {
    let root = temp_dir("revision");
    let path = root.join("secret.bin");
    let store = SecretBackendSelection::file().into_manager();
    let location = SecretLocation::file(&path);

    store
        .write(&location, SecretValue::bytes(vec![0xff, 0x00, 0x41]))
        .unwrap();

    let metadata = fs::metadata(&path).unwrap();
    let revision = store.probe_revision(&location).unwrap();
    assert_eq!(revision, Some(SecretRevision::from_metadata(&metadata)));
    assert_eq!(revision.as_ref().map(SecretRevision::size_bytes), Some(3));
    assert_eq!(
        revision.as_ref().and_then(SecretRevision::modified_at),
        metadata.modified().ok()
    );

    store
        .write(&location, SecretValue::bytes(vec![0xff, 0x00, 0x41, 0x42]))
        .unwrap();
    let updated_revision = store.probe_revision(&location).unwrap();
    assert_ne!(revision, updated_revision);

    store.delete(&location).unwrap();
    assert_eq!(store.probe_revision(&location).unwrap(), None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn auth_json_location_for_keyring_backend_uses_deterministic_account() {
    let selection = SecretBackendSelection::keyring("prodex").unwrap();
    let location = auth_json_location_for_backend("/tmp/codex-home", &selection);
    assert_eq!(
        location,
        SecretLocation::Keyring {
            service: "prodex".to_string(),
            account: "auth-json:/tmp/codex-home".to_string(),
        }
    );
    assert_eq!(
        describe_secret_location(&location),
        "keyring://prodex/auth-json:/tmp/codex-home"
    );
}

#[test]
fn refresh_lease_acquires_owner_and_creates_hashed_lock() {
    let root = temp_dir("refresh-lease-owner");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "refresh-token-secret";
    let paths = coordinator.paths_for_key(sensitive_key);

    let decision = coordinator.acquire(sensitive_key).unwrap();
    assert_eq!(decision.role(), RefreshLeaseRole::Owner);

    match decision {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(owner.lock_path().exists());
            assert!(
                !owner
                    .lock_path()
                    .display()
                    .to_string()
                    .contains(sensitive_key)
            );
            assert!(
                !owner
                    .result_path()
                    .display()
                    .to_string()
                    .contains(sensitive_key)
            );
        }
        other => panic!("expected owner, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_follower_reads_committed_result() {
    let root = temp_dir("refresh-lease-follower");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "access-token-secret";

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner
            .commit_result("{\"access_token\":\"redacted-result\"}")
            .unwrap(),
        other => panic!("expected owner, got {other:?}"),
    }

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Follower { result_json } => {
            assert_eq!(result_json, "{\"access_token\":\"redacted-result\"}");
        }
        other => panic!("expected follower, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_ignores_oversized_result_file() {
    let root = temp_dir("refresh-lease-oversized-result");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "shared-refresh-token-secret";
    let paths = coordinator.paths_for_key(sensitive_key);
    let file = fs::File::create(paths.result_path()).unwrap();
    file.set_len(1024 * 1024 + 1).unwrap();

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(!paths.result_path().exists());
        }
        other => panic!("expected owner after oversized result cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_rejects_oversized_committed_result() {
    let root = temp_dir("refresh-lease-oversized-commit");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "shared-refresh-token-secret";
    let paths = coordinator.paths_for_key(sensitive_key);
    let owner = match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };

    let err = owner
        .commit_result("x".repeat(1024 * 1024 + 1))
        .unwrap_err();
    assert!(err.to_string().contains("exceeds safe size limit"));
    assert!(!paths.result_path().exists());

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn refresh_lease_ignores_symlinked_result() {
    let root = temp_dir("refresh-lease-symlink-result");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let paths = coordinator.paths_for_key("shared-refresh-token-secret");
    let outside = root.join("outside-result.json");
    fs::write(&outside, "{\"access_token\":\"attacker\"}").unwrap();
    std::os::unix::fs::symlink(&outside, paths.result_path()).unwrap();

    match coordinator.acquire("shared-refresh-token-secret").unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(!paths.result_path().is_symlink());
            assert_eq!(
                fs::read_to_string(&outside).unwrap(),
                "{\"access_token\":\"attacker\"}"
            );
        }
        other => panic!("expected owner after unsafe result cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn refresh_lease_removes_symlinked_lock_before_acquiring() {
    let root = temp_dir("refresh-lease-symlink-lock");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let paths = coordinator.paths_for_key("shared-refresh-token-secret");
    let outside = root.join("outside-lock");
    fs::write(&outside, "pid=attacker\n").unwrap();
    std::os::unix::fs::symlink(&outside, paths.lock_path()).unwrap();

    match coordinator.acquire("shared-refresh-token-secret").unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(!owner.lock_path().is_symlink());
            assert_eq!(fs::read_to_string(&outside).unwrap(), "pid=attacker\n");
        }
        other => panic!("expected owner after unsafe lock cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_waiting_follower_reads_committed_result() {
    let root = temp_dir("refresh-lease-waiting-follower");
    let owner_coordinator = RefreshLeaseCoordinator::new(&root);
    let follower_coordinator = RefreshLeaseCoordinator::new(&root)
        .with_wait_timeout(Duration::from_secs(2))
        .with_poll_interval(Duration::from_millis(1));
    let sensitive_key = "shared-refresh-token-secret";
    let owner = match owner_coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };

    let follower = std::thread::spawn(move || follower_coordinator.acquire(sensitive_key).unwrap());
    std::thread::sleep(Duration::from_millis(20));
    owner
        .commit_result("{\"access_token\":\"shared-result\"}")
        .unwrap();

    match follower.join().unwrap() {
        RefreshLeaseDecision::Follower { result_json } => {
            assert_eq!(result_json, "{\"access_token\":\"shared-result\"}");
        }
        other => panic!("expected follower, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_recovers_stale_lock() {
    let root = temp_dir("refresh-lease-stale");
    let coordinator = RefreshLeaseCoordinator::new(&root)
        .with_lease_ttl(Duration::ZERO)
        .with_wait_timeout(Duration::from_millis(20))
        .with_poll_interval(Duration::from_millis(1));
    let paths = coordinator.paths_for_key("stale-token-secret");
    fs::write(paths.lock_path(), "pid=old\n").unwrap();
    std::thread::sleep(Duration::from_millis(2));

    match coordinator.acquire("stale-token-secret").unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(owner.lock_path().exists());
        }
        other => panic!("expected owner after stale cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_times_out_to_bypass_when_lock_is_held() {
    let root = temp_dir("refresh-lease-bypass");
    let owner_coordinator = RefreshLeaseCoordinator::new(&root);
    let follower_coordinator = RefreshLeaseCoordinator::new(&root)
        .with_wait_timeout(Duration::ZERO)
        .with_poll_interval(Duration::from_millis(1));
    let sensitive_key = "held-token-secret";
    let owner = match owner_coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };

    match follower_coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Bypass { reason } => {
            assert_eq!(reason, RefreshLeaseBypassReason::WaitTimeout);
        }
        other => panic!("expected bypass, got {other:?}"),
    }

    drop(owner);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_file_names_use_hash_not_sensitive_material() {
    let root = temp_dir("refresh-lease-hash");
    let coordinator = RefreshLeaseCoordinator::new(&root).with_namespace("quota-refresh");
    let sensitive_key = "sk-prodex-super-secret-token";
    let paths = coordinator.paths_for_key(sensitive_key);
    let lock_name = paths.lock_path().file_name().unwrap().to_string_lossy();
    let result_name = paths.result_path().file_name().unwrap().to_string_lossy();

    assert_eq!(paths.digest().len(), 64);
    assert!(paths.digest().chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(lock_name, format!("{}.lock", paths.digest()));
    assert_eq!(result_name, format!("{}.result.json", paths.digest()));
    assert!(!lock_name.contains(sensitive_key));
    assert!(!result_name.contains(sensitive_key));

    let _ = fs::remove_dir_all(root);
}
