use crate::{
    FileSecretBackend, KeyringSecretBackend, RefreshLeaseBypassReason, RefreshLeaseCoordinator,
    RefreshLeaseDecision, RefreshLeaseError, RefreshLeaseErrorStatus, RefreshLeaseRole,
    SecretBackendSelection, SecretError, SecretLocation, SecretManager, SecretRevision,
    SecretStoreErrorStatus, SecretValue, auth_json_location, auth_json_path,
    plan_refresh_lease_error_response, plan_secret_error_response, read_private_file_bounded,
    write_private_file_atomic,
};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod keyring;
mod refresh_lease;

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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    }
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
        Some(SecretValue::text("{\"access_token\":\"abc\"}"))
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
    let expected = vec![0xff, 0x00, 0x41];

    store
        .write(&location, SecretValue::bytes(expected.clone()))
        .unwrap();

    let payload = store.read(&location).unwrap().unwrap();
    payload.with_bytes(|bytes| assert_eq!(bytes, expected));
    assert!(store.read_text(&location).is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_rejects_oversized_secret_reads() {
    let root = temp_dir("oversized-read");
    let path = root.join("auth.json");
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);
    store.write(&location, SecretValue::bytes([0])).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();

    let err = store.read(&location).unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));
    assert_eq!(
        err.invalid_location_kind(),
        Some(crate::SecretInvalidLocationKind::SizeLimitExceeded)
    );
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
    assert_eq!(
        err.invalid_location_kind(),
        Some(crate::SecretInvalidLocationKind::SizeLimitExceeded)
    );
    assert!(err.to_string().contains("exceeds safe size limit"));
    assert!(!path.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_honors_caller_specific_bounds() {
    let root = temp_dir("caller-bound");
    let path = root.join("capability");
    let backend = FileSecretBackend::new();
    let location = SecretLocation::file(&path);

    assert!(
        backend
            .write_bounded(&location, SecretValue::bytes(b"12345".to_vec()), 4)
            .is_err()
    );
    assert!(!path.exists());
    backend
        .write_bounded(&location, SecretValue::bytes(b"1234".to_vec()), 4)
        .unwrap();
    assert!(backend.read_bounded(&location, 3).is_err());
    backend
        .read_bounded(&location, 4)
        .unwrap()
        .unwrap()
        .with_bytes(|bytes| assert_eq!(bytes, b"1234"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn private_file_facade_is_bounded_private_and_atomic() {
    let root = temp_dir("private-file-facade");
    let path = root.join("nested/profile-export.json");

    write_private_file_atomic(&path, b"first").unwrap();
    assert!(read_private_file_bounded(&path, 4).is_err());
    assert_eq!(
        read_private_file_bounded(&path, 5)
            .unwrap()
            .unwrap()
            .as_slice(),
        b"first"
    );

    #[cfg(unix)]
    let first_inode = {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(metadata.uid(), fs::metadata(&root).unwrap().uid());
        metadata.ino()
    };

    write_private_file_atomic(&path, b"second").unwrap();
    assert_eq!(
        read_private_file_bounded(&path, 6)
            .unwrap()
            .unwrap()
            .as_slice(),
        b"second"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        assert_ne!(first_inode, fs::metadata(&path).unwrap().ino());
    }
    assert!(fs::read_dir(path.parent().unwrap()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".prodex-secret.")
    }));

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn private_file_facade_rejects_symlinks_and_untrusted_parents() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let root = temp_dir("private-file-facade-symlink");
    let outside = root.with_extension("outside");
    fs::write(&outside, "outside").unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
    let path = root.join("profile-export.json");
    symlink(&outside, &path).unwrap();

    assert!(read_private_file_bounded(&path, 1024).is_err());
    write_private_file_atomic(&path, b"inside").unwrap();
    assert!(!path.is_symlink());
    assert_eq!(fs::read_to_string(&path).unwrap(), "inside");
    assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");

    fs::set_permissions(&root, fs::Permissions::from_mode(0o770)).unwrap();
    let rejected = root.join("rejected.json");
    assert!(write_private_file_atomic(&rejected, b"secret").is_err());
    assert!(!rejected.exists());

    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(outside);
}

#[cfg(windows)]
#[test]
fn private_file_facade_rejects_reparse_point_reads() {
    use std::os::windows::fs::symlink_file;

    let root = temp_dir("private-file-facade-reparse");
    let target = root.join("target.json");
    let path = root.join("profile-export.json");
    write_private_file_atomic(&target, b"outside").unwrap();
    if let Err(error) = symlink_file(&target, &path) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            let _ = fs::remove_dir_all(root);
            return;
        }
        panic!("failed to create test reparse point: {error}");
    }

    assert!(read_private_file_bounded(&path, 1024).is_err());
    assert_eq!(fs::read_to_string(target).unwrap(), "outside");
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

    let read_error = store.read_text(&location).unwrap_err();
    assert!(matches!(&read_error, SecretError::InvalidLocation { .. }));
    assert!(read_error.is_unsafe_file());
    assert!(matches!(
        store.probe_revision(&location).unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));
    assert!(matches!(
        store.delete(&location).unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "{\"access_token\":\"leaked\"}"
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn file_backend_rejects_symlinked_parent_components() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let root = temp_dir("symlink-parent");
    let outside = root.with_extension("outside-parent");
    fs::create_dir(&outside).unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o700)).unwrap();
    let outside_secret = outside.join("auth.json");
    fs::write(&outside_secret, "outside").unwrap();
    fs::set_permissions(&outside_secret, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&outside, root.join("linked")).unwrap();
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(root.join("linked/auth.json"));

    assert!(matches!(
        store.read(&location).unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));
    assert!(matches!(
        store.write_text(&location, "replacement").unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));
    assert_eq!(fs::read_to_string(outside_secret).unwrap(), "outside");

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn file_backend_rejects_non_private_file_and_parent_modes() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = temp_dir("mode-validation");
    let path = root.join("auth.json");
    fs::write(&path, "group-readable").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);
    assert!(matches!(
        store.read(&location).unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));

    fs::remove_file(&path).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o770)).unwrap();
    assert!(matches!(
        store.write_text(&location, "secret").unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));
    assert!(!path.exists());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn file_backend_atomically_replaces_symlink_without_touching_target() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let root = temp_dir("atomic-symlink-replace");
    let outside = root.with_extension("outside-secret");
    fs::write(&outside, "outside").unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
    let path = root.join("auth.json");
    symlink(&outside, &path).unwrap();
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    store.write_text(&location, "inside").unwrap();

    assert!(!path.is_symlink());
    assert_eq!(
        store.read_text(&location).unwrap().as_deref(),
        Some("inside")
    );
    assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(outside);
}

#[cfg(unix)]
#[test]
fn file_backend_writes_secret_files_with_private_permissions() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt};

    let root = temp_dir("private-permissions");
    let path = root.join("nested/auth.json");
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    store
        .write_text(&location, "{\"access_token\":\"abc\"}")
        .unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(
        fs::metadata(&path).unwrap().uid(),
        fs::metadata(&root).unwrap().uid()
    );

    let first_inode = fs::metadata(&path).unwrap().ino();
    store
        .write_text(&location, "{\"access_token\":\"rotated\"}")
        .unwrap();
    assert_ne!(first_inode, fs::metadata(&path).unwrap().ino());
    assert!(fs::read_dir(path.parent().unwrap()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".prodex-secret.")
    }));

    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn file_backend_rejects_reparse_point_secret_reads() {
    use std::os::windows::fs::symlink_file;

    let root = temp_dir("windows-reparse-read");
    let target = root.join("target.json");
    let path = root.join("auth.json");
    fs::write(&target, "outside").unwrap();
    if let Err(error) = symlink_file(&target, &path) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            let _ = fs::remove_dir_all(root);
            return;
        }
        panic!("failed to create test reparse point: {error}");
    }
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(&path);

    assert!(matches!(
        store.read(&location).unwrap_err(),
        SecretError::InvalidLocation { .. }
    ));
    assert_eq!(fs::read_to_string(target).unwrap(), "outside");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_rejects_empty_file_path() {
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::file(PathBuf::new());

    let err = store.read(&location).unwrap_err();

    assert!(matches!(err, SecretError::InvalidLocation { .. }));
}

#[test]
fn selectable_backend_file_round_trips_text_values() {
    let root = temp_dir("selection-text");
    let path = root.join("nested/auth.json");
    let store = SecretManager::new(SecretBackendSelection::file());
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
        Some(SecretValue::text("{\"access_token\":\"abc\"}"))
    );

    store.delete(&location).unwrap();
    assert_eq!(store.read(&location).unwrap(), None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn secret_error_response_is_stable_and_redacted() {
    let error = SecretError::io(
        "/tmp/prodex/auth.json",
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
    );
    assert!(!error.to_string().contains("/tmp"));
    assert!(!error.to_string().contains("auth.json"));
    assert!(!error.to_string().contains("permission"));

    let unsupported = SecretError::unsupported("keyring://prodex/auth-json:/tmp/codex-home");
    assert!(!unsupported.to_string().contains("keyring://"));
    assert!(!unsupported.to_string().contains("/tmp/codex-home"));

    let invalid = SecretError::invalid_location("unknown secret backend 'raw-prod-token'");
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
fn secret_store_debug_output_is_stable_and_redacted() {
    let location = SecretLocation::keyring("prodex-secret-service", "auth-json:/tmp/codex-home");
    let value = SecretValue::text("super-secret-token");
    let error = SecretError::io(
        "/tmp/prodex/auth.json",
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied for super-secret-token",
        ),
    );
    let keyring_backend = KeyringSecretBackend::new("prodex-secret-service").unwrap();
    let selection = SecretBackendSelection::Keyring(keyring_backend.clone());

    let rendered = format!("{location:?} {value:?} {error:?} {keyring_backend:?} {selection:?}");

    for sensitive in [
        "prodex-secret-service",
        "auth-json:/tmp/codex-home",
        "/tmp/prodex/auth.json",
        "super-secret-token",
        "permission denied",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "secret-store debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("Keyring"));
    assert!(rendered.contains("Text"));
    assert!(rendered.contains("Io"));
    assert!(rendered.contains("KeyringSecretBackend"));

    fn requires_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
    requires_zeroize_on_drop::<SecretValue>();
    requires_zeroize_on_drop::<SecretError>();
}

#[test]
fn refresh_lease_error_response_is_stable_and_redacted() {
    let error = RefreshLeaseError::io(
        "/tmp/prodex/refresh-token-secret.lock",
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
    );
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
fn refresh_lease_debug_output_is_stable_and_redacted() {
    let root = temp_dir("refresh-lease-debug-secret");
    let coordinator = RefreshLeaseCoordinator::new(&root)
        .with_namespace("secret-refresh-namespace")
        .with_lease_ttl(Duration::from_secs(42))
        .with_wait_timeout(Duration::from_secs(7))
        .with_result_ttl(Duration::from_secs(99))
        .with_poll_interval(Duration::from_millis(5));
    let sensitive_key = "super-refresh-secret";
    let paths = coordinator.paths_for_key(sensitive_key);
    let owner = match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };
    let follower = RefreshLeaseDecision::Follower {
        result_json: "{\"access_token\":\"super-refresh-secret\"}"
            .to_string()
            .into(),
    };
    let error = RefreshLeaseError::io(
        root.join("refresh-token-secret.lock"),
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied for super-refresh-secret",
        ),
    );

    let rendered = format!("{coordinator:?} {paths:?} {owner:?} {follower:?} {error:?}");

    for sensitive in [
        root.display().to_string(),
        paths.digest().to_string(),
        "refresh-lease-debug-secret".to_string(),
        "secret-refresh-namespace".to_string(),
        "super-refresh-secret".to_string(),
        "access_token".to_string(),
        "permission denied".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "refresh lease debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("RefreshLeaseCoordinator"));
    assert!(rendered.contains("RefreshLeasePaths"));
    assert!(rendered.contains("RefreshLeaseOwner"));
    assert!(rendered.contains("Follower"));
    assert!(rendered.contains("Io"));

    fn requires_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
    requires_zeroize_on_drop::<RefreshLeaseError>();

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_backend_probe_revision_tracks_metadata() {
    let root = temp_dir("revision");
    let path = root.join("secret.bin");
    let store = SecretManager::new(SecretBackendSelection::file());
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
    let rendered = format!("{:?}", revision.as_ref().unwrap());
    assert!(!rendered.contains("3"));
    assert!(!rendered.contains("SystemTime"));
    assert!(rendered.contains("SecretRevision"));
    assert!(rendered.contains("size_bytes: \"<redacted>\""));
    assert!(rendered.contains("modified_at: Some(\"<redacted>\")"));
    assert_eq!(
        revision.as_ref().unwrap().to_string(),
        "<redacted-secret-revision>"
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
