use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn file_backend_rejects_keyring_locations() {
    let store = SecretManager::new(FileSecretBackend::new());
    let location = SecretLocation::keyring("prodex", "auth");
    let err = store.write_text(&location, "value").unwrap_err();
    assert!(matches!(err, SecretError::UnsupportedLocation { .. }));
}

#[test]
fn keyring_backend_validates_service_name() {
    let backend = KeyringSecretBackend::new("prodex").unwrap();
    assert_eq!(backend.service(), "prodex");

    let err = KeyringSecretBackend::new("   ").unwrap_err();
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
