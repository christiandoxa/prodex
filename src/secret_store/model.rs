use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretLocation {
    File(PathBuf),
    Keyring { service: String, account: String },
}

impl SecretLocation {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }

    pub fn auth_json(codex_home: impl AsRef<Path>) -> Self {
        Self::File(codex_home.as_ref().join("auth.json"))
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretValue {
    Text(String),
    Bytes(Vec<u8>),
}

impl SecretValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn bytes(value: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(value.into())
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value.as_str()),
            Self::Bytes(_) => None,
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::Text(value) => value.into_bytes(),
            Self::Bytes(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    UnsupportedLocation { location: String },
    InvalidLocation { reason: String },
    Io { path: PathBuf, reason: String },
}

impl SecretError {
    pub fn unsupported(location: impl Into<String>) -> Self {
        Self::UnsupportedLocation {
            location: location.into(),
        }
    }

    pub fn invalid_location(reason: impl Into<String>) -> Self {
        Self::InvalidLocation {
            reason: reason.into(),
        }
    }

    pub fn io(path: impl Into<PathBuf>, error: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            reason: error.to_string(),
        }
    }
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLocation { location } => {
                write!(f, "unsupported secret location: {location}")
            }
            Self::InvalidLocation { reason } => write!(f, "invalid secret location: {reason}"),
            Self::Io { path, reason } => write!(f, "I/O error for {}: {reason}", path.display()),
        }
    }
}

impl StdError for SecretError {}

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
        match value.trim().to_ascii_lowercase().as_str() {
            "file" => Ok(Self::File),
            "keyring" => Ok(Self::Keyring),
            _ => Err(SecretError::invalid_location(format!(
                "unknown secret backend '{value}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn modified_at(&self) -> Option<SystemTime> {
        self.modified_at
    }
}

impl fmt::Display for SecretRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.modified_at.as_ref() {
            Some(modified_at) => write!(
                f,
                "size_bytes={} modified_at={modified_at:?}",
                self.size_bytes
            ),
            None => write!(f, "size_bytes={} modified_at=none", self.size_bytes),
        }
    }
}

pub trait SecretBackend {
    fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError>;
    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError>;
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
    pub fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {
        self.backend.read(location)
    }

    pub fn read_text(&self, location: &SecretLocation) -> Result<Option<String>, SecretError> {
        match self.backend.read(location)? {
            Some(SecretValue::Text(text)) => Ok(Some(text)),
            Some(SecretValue::Bytes(bytes)) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| SecretError::invalid_location("secret payload is not valid UTF-8")),
            None => Ok(None),
        }
    }

    pub fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        self.backend.write(location, value)
    }

    pub fn write_text(
        &self,
        location: &SecretLocation,
        value: impl Into<String>,
    ) -> Result<(), SecretError> {
        self.backend
            .write(location, SecretValue::Text(value.into()))
    }

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
