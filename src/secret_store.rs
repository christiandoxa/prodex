#![allow(dead_code)]

use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FileSecretBackend;

impl FileSecretBackend {
    pub fn new() -> Self {
        Self
    }
}

impl SecretBackend for FileSecretBackend {
    fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {
        let path = match location {
            SecretLocation::File(path) => path,
            SecretLocation::Keyring { service, account } => {
                return Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )));
            }
        };

        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(SecretError::io(path, err)),
        };

        match String::from_utf8(bytes.clone()) {
            Ok(text) => Ok(Some(SecretValue::Text(text))),
            Err(_) => Ok(Some(SecretValue::Bytes(bytes))),
        }
    }

    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        let path = match location {
            SecretLocation::File(path) => path,
            SecretLocation::Keyring { service, account } => {
                return Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )));
            }
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| SecretError::io(parent, err))?;
        }

        let bytes = value.into_bytes();
        let temp_path = unique_temp_path(path);
        fs::write(&temp_path, bytes).map_err(|err| SecretError::io(&temp_path, err))?;
        replace_file(&temp_path, path)?;
        secure_file(path)?;
        Ok(())
    }

    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        let path = match location {
            SecretLocation::File(path) => path,
            SecretLocation::Keyring { service, account } => {
                return Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )));
            }
        };

        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(SecretError::io(path, err)),
        }
    }
}

impl SecretRevisionBackend for FileSecretBackend {
    fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError> {
        let path = match location {
            SecretLocation::File(path) => path,
            SecretLocation::Keyring { service, account } => {
                return Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )));
            }
        };

        match fs::metadata(path) {
            Ok(metadata) => Ok(Some(SecretRevision::from_metadata(&metadata))),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(SecretError::io(path, err)),
        }
    }
}

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
        match location {
            SecretLocation::Keyring { service, account } => {
                if service != &self.service {
                    return Err(SecretError::invalid_location(format!(
                        "expected keyring service '{}' but got '{}'",
                        self.service, service
                    )));
                }
                Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )))
            }
            SecretLocation::File(path) => Err(SecretError::unsupported(format!(
                "file://{}",
                path.display()
            ))),
        }
    }

    fn write(&self, location: &SecretLocation, _value: SecretValue) -> Result<(), SecretError> {
        match location {
            SecretLocation::Keyring { service, account } => {
                if service != &self.service {
                    return Err(SecretError::invalid_location(format!(
                        "expected keyring service '{}' but got '{}'",
                        self.service, service
                    )));
                }
                Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )))
            }
            SecretLocation::File(path) => Err(SecretError::unsupported(format!(
                "file://{}",
                path.display()
            ))),
        }
    }

    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        match location {
            SecretLocation::Keyring { service, account } => {
                if service != &self.service {
                    return Err(SecretError::invalid_location(format!(
                        "expected keyring service '{}' but got '{}'",
                        self.service, service
                    )));
                }
                Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )))
            }
            SecretLocation::File(path) => Err(SecretError::unsupported(format!(
                "file://{}",
                path.display()
            ))),
        }
    }
}

impl SecretRevisionBackend for KeyringSecretBackend {
    fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError> {
        match location {
            SecretLocation::Keyring { service, account } => {
                if service != &self.service {
                    return Err(SecretError::invalid_location(format!(
                        "expected keyring service '{}' but got '{}'",
                        self.service, service
                    )));
                }
                Err(SecretError::unsupported(format!(
                    "keyring://{service}/{account}"
                )))
            }
            SecretLocation::File(path) => Err(SecretError::unsupported(format!(
                "file://{}",
                path.display()
            ))),
        }
    }
}

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

fn unique_temp_path(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let temp_name = format!(
        "{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("secret"),
        pid,
        nanos
    );
    path.with_file_name(temp_name)
}

fn replace_file(temp_path: &Path, path: &Path) -> Result<(), SecretError> {
    match fs::rename(temp_path, path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(path).map_err(|err| SecretError::io(path, err))?;
            fs::rename(temp_path, path).map_err(|err| SecretError::io(path, err))
        }
        Err(err) => Err(SecretError::io(path, err)),
    }
}

fn secure_file(path: &Path) -> Result<(), SecretError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions).map_err(|err| SecretError::io(path, err))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
