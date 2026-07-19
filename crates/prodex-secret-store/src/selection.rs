use crate::{
    FileSecretBackend, KeyringSecretBackend, SecretBackend, SecretBackendKind, SecretError,
    SecretLocation, SecretRevision, SecretRevisionBackend, SecretValue,
};
use sha2::{Digest, Sha256};
use std::fmt::{self, Write as _};

#[derive(Clone, PartialEq, Eq)]
pub enum SecretBackendSelection {
    File(FileSecretBackend),
    Keyring(KeyringSecretBackend),
}

impl SecretBackendSelection {
    pub fn file() -> Self {
        Self::File(FileSecretBackend::new())
    }

    pub fn keyring(service: impl Into<String>) -> Result<Self, SecretError> {
        Ok(Self::Keyring(KeyringSecretBackend::new(service)?))
    }

    pub fn from_kind(
        kind: SecretBackendKind,
        keyring_service: Option<String>,
    ) -> Result<Self, SecretError> {
        match kind {
            SecretBackendKind::File => Ok(Self::file()),
            SecretBackendKind::Keyring => match keyring_service {
                Some(service) => Self::keyring(service),
                None => Err(SecretError::invalid_location(
                    "keyring backend requires a service name",
                )),
            },
        }
    }

    pub fn kind(&self) -> SecretBackendKind {
        match self {
            Self::File(_) => SecretBackendKind::File,
            Self::Keyring(_) => SecretBackendKind::Keyring,
        }
    }

    pub fn keyring_service(&self) -> Option<&str> {
        match self {
            Self::File(_) => None,
            Self::Keyring(backend) => Some(backend.service()),
        }
    }

    pub fn read_bounded(
        &self,
        location: &SecretLocation,
        max_bytes: u64,
    ) -> Result<Option<SecretValue>, SecretError> {
        match self {
            Self::File(backend) => backend.read_bounded(location, max_bytes),
            Self::Keyring(backend) => {
                let location = projected_keyring_location(backend, location);
                let value = backend.read(&location)?;
                if value
                    .as_ref()
                    .is_some_and(|value| value.with_bytes(|bytes| bytes.len() as u64 > max_bytes))
                {
                    return Err(SecretError::size_limit_exceeded(
                        "keyring secret exceeds requested safe size limit",
                    ));
                }
                Ok(value)
            }
        }
    }

    pub fn write_bounded(
        &self,
        location: &SecretLocation,
        value: SecretValue,
        max_bytes: u64,
    ) -> Result<(), SecretError> {
        if value.with_bytes(|bytes| bytes.len() as u64 > max_bytes) {
            return Err(SecretError::size_limit_exceeded(
                "secret exceeds requested safe size limit",
            ));
        }
        match self {
            Self::File(backend) => backend.write_bounded(location, value, max_bytes),
            Self::Keyring(backend) => {
                let location = projected_keyring_location(backend, location);
                backend.write(&location, value)
            }
        }
    }
}

impl fmt::Debug for SecretBackendSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(_) => f
                .debug_tuple("File")
                .field(&FileSecretBackend::new())
                .finish(),
            Self::Keyring(_) => f
                .debug_tuple("Keyring")
                .field(&"<redacted-keyring-backend>")
                .finish(),
        }
    }
}

impl Default for SecretBackendSelection {
    fn default() -> Self {
        Self::file()
    }
}

impl SecretBackend for SecretBackendSelection {
    fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {
        match self {
            Self::File(backend) => backend.read(location),
            Self::Keyring(backend) => backend.read(&projected_keyring_location(backend, location)),
        }
    }

    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        match self {
            Self::File(backend) => backend.write(location, value),
            Self::Keyring(backend) => {
                backend.write(&projected_keyring_location(backend, location), value)
            }
        }
    }

    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        match self {
            Self::File(backend) => backend.delete(location),
            Self::Keyring(backend) => {
                backend.delete(&projected_keyring_location(backend, location))
            }
        }
    }
}

impl SecretRevisionBackend for SecretBackendSelection {
    fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError> {
        match self {
            Self::File(backend) => backend.probe_revision(location),
            Self::Keyring(backend) => {
                backend.probe_revision(&projected_keyring_location(backend, location))
            }
        }
    }
}

fn projected_keyring_location(
    backend: &KeyringSecretBackend,
    location: &SecretLocation,
) -> SecretLocation {
    match location {
        SecretLocation::File(path) => {
            let digest = Sha256::digest(path.as_os_str().as_encoded_bytes());
            let mut account = String::with_capacity(5 + digest.len() * 2);
            account.push_str("file:");
            for byte in digest {
                write!(account, "{byte:02x}").expect("writing to String cannot fail");
            }
            SecretLocation::keyring(backend.service(), account)
        }
        SecretLocation::Keyring { .. } => location.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_selection_projects_file_paths_to_stable_opaque_accounts() {
        let backend = KeyringSecretBackend::new("prodex-test").unwrap();
        let first =
            projected_keyring_location(&backend, &SecretLocation::file("/home/test-user/a"));
        let repeated =
            projected_keyring_location(&backend, &SecretLocation::file("/home/test-user/a"));
        let second =
            projected_keyring_location(&backend, &SecretLocation::file("/home/test-user/b"));

        assert_eq!(first, repeated);
        assert_ne!(first, second);
        let SecretLocation::Keyring { service, account } = first else {
            panic!("file location should project to keyring");
        };
        assert_eq!(service, "prodex-test");
        assert!(account.starts_with("file:"));
        assert!(!account.contains("test-user"));
    }
}
