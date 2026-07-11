use crate::{
    SecretBackend, SecretError, SecretLocation, SecretRevision, SecretRevisionBackend, SecretValue,
};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct KeyringSecretBackend {
    service: String,
}

impl KeyringSecretBackend {
    pub fn new(service: impl Into<String>) -> Result<Self, SecretError> {
        let service = service.into();
        if service.is_empty() || service.chars().any(char::is_whitespace) {
            return Err(SecretError::invalid_location(
                "keyring service name must be non-empty without whitespace",
            ));
        }
        Ok(Self { service })
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn is_supported(&self) -> bool {
        false
    }

    pub fn unsupported_reason(&self) -> &'static str {
        "keyring secret backend is not implemented in this build; use backend=file"
    }
}

impl fmt::Debug for KeyringSecretBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyringSecretBackend")
            .field("service", &"<redacted>")
            .finish()
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
        SecretLocation::Keyring { service, account } => {
            if service != expected_service {
                return Err(SecretError::invalid_location(format!(
                    "expected keyring service '{}' but got '{}'",
                    expected_service, service
                )));
            }
            if account.is_empty() {
                return Err(SecretError::invalid_location(
                    "keyring account cannot be empty",
                ));
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
