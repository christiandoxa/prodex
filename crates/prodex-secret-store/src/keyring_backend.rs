use crate::{
    SecretBackend, SecretError, SecretLocation, SecretRevision, SecretRevisionBackend, SecretValue,
};
use std::fmt;
use std::io;
use zeroize::Zeroizing;

const KEYRING_SECRET_MAX_BYTES: usize = 1024 * 1024;
const KEYRING_TEXT_TAG: u8 = 0;
const KEYRING_BYTES_TAG: u8 = 1;

/// OS-native keyring backend backed by Keychain, Credential Manager, or Secret Service.
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
        true
    }

    pub fn unsupported_reason(&self) -> &'static str {
        "OS keyring backend is unavailable"
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
        let entry = self.entry(location)?;
        match entry.get_secret() {
            Ok(secret) => decode_keyring_secret(Zeroizing::new(secret)).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_error(error)),
        }
    }

    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        let entry = self.entry(location)?;
        let secret = encode_keyring_secret(value)?;
        entry.set_secret(&secret).map_err(keyring_error)
    }

    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        let entry = self.entry(location)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_error(error)),
        }
    }
}

impl SecretRevisionBackend for KeyringSecretBackend {
    fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError> {
        Ok(self.read(location)?.map(|value| {
            let size = value.with_bytes(|bytes| bytes.len() as u64);
            SecretRevision::new(size, None)
        }))
    }
}

impl KeyringSecretBackend {
    fn entry(&self, location: &SecretLocation) -> Result<keyring::Entry, SecretError> {
        let account = validate_keyring_location(location, &self.service)?;
        keyring::Entry::new(&self.service, account).map_err(keyring_error)
    }
}

fn validate_keyring_location<'a>(
    location: &'a SecretLocation,
    expected_service: &str,
) -> Result<&'a str, SecretError> {
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
            Ok(account)
        }
        SecretLocation::File(path) => Err(SecretError::unsupported(format!(
            "file://{}",
            path.display()
        ))),
    }
}

fn encode_keyring_secret(value: SecretValue) -> Result<Zeroizing<Vec<u8>>, SecretError> {
    let (tag, mut bytes) = match value {
        SecretValue::Text(mut text) => (KEYRING_TEXT_TAG, std::mem::take(&mut *text).into_bytes()),
        SecretValue::Bytes(mut bytes) => (KEYRING_BYTES_TAG, std::mem::take(&mut *bytes)),
    };
    if bytes.len() > KEYRING_SECRET_MAX_BYTES {
        return Err(SecretError::size_limit_exceeded(
            "keyring secret exceeds safe size limit",
        ));
    }
    bytes.insert(0, tag);
    Ok(Zeroizing::new(bytes))
}

fn decode_keyring_secret(mut secret: Zeroizing<Vec<u8>>) -> Result<SecretValue, SecretError> {
    if secret.is_empty() || secret.len() - 1 > KEYRING_SECRET_MAX_BYTES {
        return Err(SecretError::invalid_location(
            "invalid keyring secret envelope",
        ));
    }
    let tag = secret.remove(0);
    match tag {
        KEYRING_TEXT_TAG => String::from_utf8(std::mem::take(&mut *secret))
            .map(SecretValue::text)
            .map_err(|error| {
                drop(Zeroizing::new(error.into_bytes()));
                SecretError::invalid_location("keyring text secret is not valid UTF-8")
            }),
        KEYRING_BYTES_TAG => Ok(SecretValue::bytes(std::mem::take(&mut *secret))),
        _ => Err(SecretError::invalid_location(
            "unknown keyring secret envelope",
        )),
    }
}

fn keyring_error(_error: keyring::Error) -> SecretError {
    SecretError::io("<keyring>", io::Error::other("OS keyring operation failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_envelope_round_trips_text_and_bytes() {
        for value in [
            SecretValue::text("secret"),
            SecretValue::bytes(vec![0xff, 0x00, 0x41]),
        ] {
            let expected = value.with_bytes(|bytes| bytes.to_vec());
            let decoded = decode_keyring_secret(encode_keyring_secret(value).unwrap()).unwrap();
            assert_eq!(decoded.with_bytes(|bytes| bytes.to_vec()), expected);
        }
    }

    #[test]
    fn keyring_envelope_rejects_oversized_values() {
        let error =
            encode_keyring_secret(SecretValue::bytes(vec![0; KEYRING_SECRET_MAX_BYTES + 1]))
                .unwrap_err();
        assert_eq!(
            error.invalid_location_kind(),
            Some(crate::SecretInvalidLocationKind::SizeLimitExceeded)
        );
    }

    #[test]
    fn keyring_errors_do_not_expose_backend_details() {
        let error = keyring_error(keyring::Error::Invalid(
            "secret-token".to_string(),
            "/home/test-user/private".to_string(),
        ));
        let rendered = error.to_string();
        assert!(!rendered.contains("secret-token"));
        assert!(!rendered.contains("/home/test-user"));
    }
}
