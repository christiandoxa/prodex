use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use zeroize::{ZeroizeOnDrop, Zeroizing};

#[derive(Clone, PartialEq, Eq)]
pub enum SecretLocation {
    File(PathBuf),
    #[cfg_attr(not(test), allow(dead_code))]
    Keyring {
        service: String,
        account: String,
    },
}

impl SecretLocation {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }

    pub fn auth_json(codex_home: impl AsRef<Path>) -> Self {
        Self::File(codex_home.as_ref().join("auth.json"))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn keyring(service: impl Into<String>, account: impl Into<String>) -> Self {
        Self::Keyring {
            service: service.into(),
            account: account.into(),
        }
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }
}

impl fmt::Debug for SecretLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(_) => f.debug_tuple("File").field(&"<redacted>").finish(),
            Self::Keyring { .. } => f
                .debug_struct("Keyring")
                .field("service", &"<redacted>")
                .field("account", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum SecretValue {
    Text(Zeroizing<String>),
    Bytes(Zeroizing<Vec<u8>>),
}

impl ZeroizeOnDrop for SecretValue {}

impl SecretValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(Zeroizing::new(value.into()))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn bytes(value: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(Zeroizing::new(value.into()))
    }

    pub fn with_bytes<T>(&self, expose: impl FnOnce(&[u8]) -> T) -> T {
        match self {
            Self::Text(value) => expose(value.as_bytes()),
            Self::Bytes(value) => expose(value.as_slice()),
        }
    }

    pub fn with_text<T>(&self, expose: impl FnOnce(Option<&str>) -> T) -> T {
        match self {
            Self::Text(value) => expose(Some(value.as_str())),
            Self::Bytes(_) => expose(None),
        }
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(_) => f.debug_tuple("Text").field(&"<redacted>").finish(),
            Self::Bytes(_) => f.debug_tuple("Bytes").field(&"<redacted>").finish(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretInvalidLocationKind {
    Generic,
    UnsafeFile,
    SizeLimitExceeded,
}

#[derive(Clone, PartialEq, Eq)]
pub enum SecretError {
    UnsupportedLocation {
        location: Zeroizing<String>,
    },
    InvalidLocation {
        kind: SecretInvalidLocationKind,
        reason: Zeroizing<String>,
    },
    Io {
        reason: Zeroizing<String>,
    },
}

impl ZeroizeOnDrop for SecretError {}

impl SecretError {
    pub fn unsupported(location: impl Into<String>) -> Self {
        Self::UnsupportedLocation {
            location: Zeroizing::new(location.into()),
        }
    }

    pub fn invalid_location(reason: impl Into<String>) -> Self {
        Self::invalid_location_with_kind(SecretInvalidLocationKind::Generic, reason)
    }

    pub(crate) fn unsafe_file(reason: impl Into<String>) -> Self {
        Self::invalid_location_with_kind(SecretInvalidLocationKind::UnsafeFile, reason)
    }

    pub(crate) fn size_limit_exceeded(reason: impl Into<String>) -> Self {
        Self::invalid_location_with_kind(SecretInvalidLocationKind::SizeLimitExceeded, reason)
    }

    fn invalid_location_with_kind(
        kind: SecretInvalidLocationKind,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidLocation {
            kind,
            reason: Zeroizing::new(reason.into()),
        }
    }

    pub fn io(_path: impl Into<PathBuf>, error: io::Error) -> Self {
        Self::Io {
            reason: Zeroizing::new(error.to_string()),
        }
    }

    pub fn invalid_location_kind(&self) -> Option<SecretInvalidLocationKind> {
        match self {
            Self::InvalidLocation { kind, .. } => Some(*kind),
            Self::UnsupportedLocation { .. } | Self::Io { .. } => None,
        }
    }

    pub fn is_unsafe_file(&self) -> bool {
        self.invalid_location_kind() == Some(SecretInvalidLocationKind::UnsafeFile)
    }
}

impl fmt::Debug for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLocation { .. } => f
                .debug_struct("UnsupportedLocation")
                .field("location", &"<redacted>")
                .finish(),
            Self::InvalidLocation { .. } => f
                .debug_struct("InvalidLocation")
                .field("reason", &"<redacted>")
                .finish(),
            Self::Io { .. } => f.debug_struct("Io").field("reason", &"<redacted>").finish(),
        }
    }
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLocation { .. } => write!(f, "unsupported secret location"),
            Self::InvalidLocation {
                kind: SecretInvalidLocationKind::SizeLimitExceeded,
                ..
            } => {
                write!(f, "secret exceeds safe size limit")
            }
            Self::InvalidLocation { .. } => write!(f, "invalid secret location"),
            Self::Io { .. } => write!(f, "secret storage I/O error"),
        }
    }
}

impl StdError for SecretError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStoreErrorStatus {
    ServiceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretStoreErrorResponsePlan {
    pub status: SecretStoreErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_secret_error_response(_error: &SecretError) -> SecretStoreErrorResponsePlan {
    SecretStoreErrorResponsePlan {
        status: SecretStoreErrorStatus::ServiceUnavailable,
        code: "secret_store_unavailable",
        message: "secret storage is temporarily unavailable",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretBackendKind {
    File,
    Keyring,
}

impl SecretBackendKind {
    pub fn file() -> Self {
        Self::File
    }

    pub fn keyring() -> Self {
        Self::Keyring
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Keyring => "keyring",
        }
    }
}

impl fmt::Display for SecretBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SecretBackendKind {
    type Err = SecretError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value != value.trim() {
            return Err(SecretError::invalid_location(format!(
                "unknown secret backend '{value}'"
            )));
        }
        match value.to_ascii_lowercase().as_str() {
            "file" => Ok(Self::File),
            "keyring" => Ok(Self::Keyring),
            _ => Err(SecretError::invalid_location(format!(
                "unknown secret backend '{value}'"
            ))),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretRevision {
    size_bytes: u64,
    modified_at: Option<SystemTime>,
}

impl SecretRevision {
    pub fn new(size_bytes: u64, modified_at: Option<SystemTime>) -> Self {
        Self {
            size_bytes,
            modified_at,
        }
    }

    pub fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self::new(metadata.len(), metadata.modified().ok())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn modified_at(&self) -> Option<SystemTime> {
        self.modified_at
    }
}

impl fmt::Debug for SecretRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretRevision")
            .field("size_bytes", &"<redacted>")
            .field("modified_at", &self.modified_at.map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Display for SecretRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted-secret-revision>")
    }
}

pub trait SecretBackend {
    fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError>;
    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError>;
    #[cfg_attr(not(test), allow(dead_code))]
    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError>;
}

pub trait SecretRevisionBackend: SecretBackend {
    fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError>;
}

#[derive(Debug, Clone)]
pub struct SecretManager<B> {
    backend: B,
}

impl<B> SecretManager<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: SecretBackend> SecretManager<B> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {
        self.backend.read(location)
    }

    pub fn read_text(&self, location: &SecretLocation) -> Result<Option<String>, SecretError> {
        match self.backend.read(location)? {
            Some(SecretValue::Text(mut text)) => Ok(Some(std::mem::take(&mut *text))),
            Some(SecretValue::Bytes(mut bytes)) => String::from_utf8(std::mem::take(&mut *bytes))
                .map(Some)
                .map_err(|error| {
                    drop(Zeroizing::new(error.into_bytes()));
                    SecretError::invalid_location("secret payload is not valid UTF-8")
                }),
            None => Ok(None),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        self.backend.write(location, value)
    }

    pub fn write_text(
        &self,
        location: &SecretLocation,
        value: impl Into<String>,
    ) -> Result<(), SecretError> {
        self.backend.write(location, SecretValue::text(value))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        self.backend.delete(location)
    }
}

impl<B: SecretRevisionBackend> SecretManager<B> {
    pub fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError> {
        self.backend.probe_revision(location)
    }
}
