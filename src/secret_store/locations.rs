use crate::secret_store::{SecretBackendSelection, SecretLocation};
use std::path::{Path, PathBuf};

pub fn auth_json_path(codex_home: impl AsRef<Path>) -> PathBuf {
    codex_home.as_ref().join("auth.json")
}

pub fn auth_json_location(codex_home: impl AsRef<Path>) -> SecretLocation {
    SecretLocation::File(auth_json_path(codex_home))
}

pub fn auth_json_location_for_backend(
    codex_home: impl AsRef<Path>,
    selection: &SecretBackendSelection,
) -> SecretLocation {
    match selection {
        SecretBackendSelection::File(_) => auth_json_location(codex_home),
        SecretBackendSelection::Keyring(backend) => SecretLocation::keyring(
            backend.service().to_string(),
            auth_json_keyring_account(codex_home),
        ),
    }
}

pub fn auth_json_keyring_account(codex_home: impl AsRef<Path>) -> String {
    format!("auth-json:{}", codex_home.as_ref().display())
}

pub fn describe_secret_location(location: &SecretLocation) -> String {
    match location {
        SecretLocation::File(path) => path.display().to_string(),
        SecretLocation::Keyring { service, account } => format!("keyring://{service}/{account}"),
    }
}
