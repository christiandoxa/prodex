use crate::{
    FileSecretBackend, KeyringSecretBackend, SecretBackend, SecretBackendKind,
    SecretBackendSelection, SecretError, SecretLocation, SecretManager,
    auth_json_location_for_backend, describe_secret_location,
};

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
fn selectable_backend_from_kind_requires_keyring_service() {
    assert_eq!(
        SecretBackendSelection::from_kind(SecretBackendKind::File, None)
            .unwrap()
            .kind(),
        SecretBackendKind::File
    );

    let err = SecretBackendSelection::from_kind(SecretBackendKind::Keyring, None).unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));

    let selection =
        SecretBackendSelection::from_kind(SecretBackendKind::Keyring, Some("prodex".to_string()))
            .unwrap();
    let SecretBackendSelection::Keyring(backend) = selection else {
        panic!("expected keyring selection marker");
    };
    assert!(!backend.is_supported());
}

#[test]
fn secret_backend_kind_rejects_padded_values() {
    let err = " keyring ".parse::<SecretBackendKind>().unwrap_err();
    assert!(matches!(err, SecretError::InvalidLocation { .. }));
}

#[test]
fn auth_json_location_for_keyring_backend_uses_deterministic_account() {
    let selection = SecretBackendSelection::Keyring(KeyringSecretBackend::new("prodex").unwrap());
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
