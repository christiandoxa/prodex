use crate::secret_store::{
    SecretBackend, SecretError, SecretLocation, SecretRevision, SecretRevisionBackend, SecretValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyringSecretBackend {
    service: String,
}

impl KeyringSecretBackend {
    pub fn new(service: impl Into<String>) -> Result<Self, SecretError> {
        let service = service.into();
        if service.trim().is_empty() {
            return Err(SecretError::invalid_location(
                "keyring service name cannot be empty",
            ));
        }
        Ok(Self { service })
    }

    pub fn service(&self) -> &str {
        &self.service
    }
}

impl SecretBackend for KeyringSecretBackend {
    fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {
        validate_keyring_location(location, &self.service)?;
        unsupported_location(location)
    }

    fn write(&self, location: &SecretLocation, _value: SecretValue) -> Result<(), SecretError> {
        validate_keyring_location(location, &self.service)?;
        unsupported_location(location)
    }

    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        validate_keyring_location(location, &self.service)?;
        unsupported_location(location)
    }
}

impl SecretRevisionBackend for KeyringSecretBackend {
    fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError> {
        validate_keyring_location(location, &self.service)?;
        unsupported_location(location)
    }
}

fn validate_keyring_location(
    location: &SecretLocation,
    expected_service: &str,
) -> Result<(), SecretError> {
    match location {
        SecretLocation::Keyring { service, .. } => {
            if service != expected_service {
                return Err(SecretError::invalid_location(format!(
                    "expected keyring service '{}' but got '{}'",
                    expected_service, service
                )));
            }
            Ok(())
        }
        SecretLocation::File(path) => Err(SecretError::unsupported(format!(
            "file://{}",
            path.display()
        ))),
    }
}

fn unsupported_location<T>(location: &SecretLocation) -> Result<T, SecretError> {
    match location {
        SecretLocation::Keyring { service, account } => Err(SecretError::unsupported(format!(
            "keyring://{service}/{account}"
        ))),
        SecretLocation::File(path) => Err(SecretError::unsupported(format!(
            "file://{}",
            path.display()
        ))),
    }
}
