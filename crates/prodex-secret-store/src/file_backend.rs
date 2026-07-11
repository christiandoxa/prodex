use crate::{
    SecretBackend, SecretError, SecretLocation, SecretRevision, SecretRevisionBackend, SecretValue,
};
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Read as _, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FILE_SECRET_MAX_BYTES: u64 = 1024 * 1024;

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
        let Some(metadata) = secret_file_metadata(path)? else {
            return Ok(None);
        };
        if metadata.len() > FILE_SECRET_MAX_BYTES {
            return Err(secret_size_error(path));
        }

        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(SecretError::io(path, err)),
        };
        let opened_metadata = file.metadata().map_err(|err| SecretError::io(path, err))?;
        if !same_secret_file_metadata(&metadata, &opened_metadata) {
            return Err(SecretError::invalid_location(format!(
                "{} changed while reading",
                path.display()
            )));
        }

        let mut bytes = Vec::new();
        file.take(FILE_SECRET_MAX_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|err| SecretError::io(path, err))?;
        if bytes.len() as u64 > FILE_SECRET_MAX_BYTES {
            return Err(secret_size_error(path));
        }

        match String::from_utf8(bytes) {
            Ok(text) => Ok(Some(SecretValue::Text(text))),
            Err(err) => Ok(Some(SecretValue::Bytes(err.into_bytes()))),
        }
    }

    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        let path = file_path(location)?;
        let bytes = value.into_bytes();
        if bytes.len() as u64 > FILE_SECRET_MAX_BYTES {
            return Err(secret_size_error(path));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| SecretError::io(parent, err))?;
        }

        let temp_path = unique_temp_path(path);
        write_secure_temp_file(&temp_path, &bytes)?;
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

        match secret_file_metadata(path)? {
            Some(metadata) => Ok(Some(SecretRevision::from_metadata(&metadata))),
            None => Ok(None),
        }
    }
}

fn file_path(location: &SecretLocation) -> Result<&Path, SecretError> {
    match location {
        SecretLocation::File(path) => {
            if path.as_os_str().is_empty() {
                return Err(SecretError::invalid_location(
                    "file secret path cannot be empty",
                ));
            }
            Ok(path)
        }
        SecretLocation::Keyring { service, account } => Err(SecretError::unsupported(format!(
            "keyring://{service}/{account}"
        ))),
    }
}

fn secret_file_metadata(path: &Path) -> Result<Option<fs::Metadata>, SecretError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(SecretError::io(path, err)),
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() || !file_type.is_file() {
        return Err(SecretError::invalid_location(format!(
            "{} is not a regular secret file",
            path.display()
        )));
    }
    Ok(Some(metadata))
}

#[cfg(unix)]
fn same_secret_file_metadata(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn same_secret_file_metadata(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}

fn secret_size_error(path: &Path) -> SecretError {
    SecretError::invalid_location(format!(
        "{} exceeds safe size limit ({} bytes)",
        path.display(),
        FILE_SECRET_MAX_BYTES
    ))
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

fn write_secure_temp_file(path: &Path, bytes: &[u8]) -> Result<(), SecretError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(path)
        .map_err(|err| SecretError::io(path, err))?;
    file.write_all(bytes)
        .map_err(|err| SecretError::io(path, err))?;
    Ok(())
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
