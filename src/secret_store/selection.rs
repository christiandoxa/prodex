use crate::secret_store::{
    FileSecretBackend, KeyringSecretBackend, SecretBackend, SecretBackendKind, SecretError,
    SecretLocation, SecretManager, SecretRevision, SecretRevisionBackend, SecretValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn into_manager(self) -> SecretManager<Self> {
        SecretManager::new(self)
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
            Self::Keyring(backend) => backend.read(location),
        }
    }

    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        match self {
            Self::File(backend) => backend.write(location, value),
            Self::Keyring(backend) => backend.write(location, value),
        }
    }

    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        match self {
            Self::File(backend) => backend.delete(location),
            Self::Keyring(backend) => backend.delete(location),
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
            Self::Keyring(backend) => backend.probe_revision(location),
        }
    }
}
