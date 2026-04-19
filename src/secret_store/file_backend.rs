use crate::secret_store::{
    SecretBackend, SecretError, SecretLocation, SecretRevision, SecretRevisionBackend, SecretValue,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FileSecretBackend;

impl FileSecretBackend {
    pub fn new() -> Self {
        Self
    }
}

impl SecretBackend for FileSecretBackend {
    fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {
        let path = file_path(location)?;
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
        let path = file_path(location)?;

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
        let path = file_path(location)?;

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
        let path = file_path(location)?;

        match fs::metadata(path) {
            Ok(metadata) => Ok(Some(SecretRevision::from_metadata(&metadata))),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(SecretError::io(path, err)),
        }
    }
}

fn file_path(location: &SecretLocation) -> Result<&Path, SecretError> {
    match location {
        SecretLocation::File(path) => Ok(path),
        SecretLocation::Keyring { service, account } => Err(SecretError::unsupported(format!(
            "keyring://{service}/{account}"
        ))),
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
