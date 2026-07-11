use crate::{
    SecretBackend, SecretError, SecretLocation, SecretRevision, SecretRevisionBackend, SecretValue,
    secure_file::{self, FileSecurity},
};
use std::io;
use std::path::Path;

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
        let Some(opened) = secure_file::open_file(path, FileSecurity::Private)
            .map_err(|error| secure_error(path, error))?
        else {
            return Ok(None);
        };
        if opened.metadata().len() > FILE_SECRET_MAX_BYTES {
            return Err(secret_size_error(path));
        }
        let bytes = opened
            .read_bounded(FILE_SECRET_MAX_BYTES)
            .map_err(|error| secure_error(path, error))?;

        match String::from_utf8(bytes) {
            Ok(text) => Ok(Some(SecretValue::text(text))),
            Err(err) => Ok(Some(SecretValue::bytes(err.into_bytes()))),
        }
    }

    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        let path = file_path(location)?;
        value.with_bytes(|bytes| {
            if bytes.len() as u64 > FILE_SECRET_MAX_BYTES {
                return Err(secret_size_error(path));
            }
            secure_file::write_private_atomic(path, bytes)
                .map_err(|error| secure_error(path, error))
        })
    }

    fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {
        let path = file_path(location)?;

        secure_file::delete_private(path).map_err(|error| secure_error(path, error))
    }
}

impl SecretRevisionBackend for FileSecretBackend {
    fn probe_revision(
        &self,
        location: &SecretLocation,
    ) -> Result<Option<SecretRevision>, SecretError> {
        let path = file_path(location)?;

        match secure_file::open_file(path, FileSecurity::Private)
            .map_err(|error| secure_error(path, error))?
        {
            Some(opened) => Ok(Some(SecretRevision::from_metadata(opened.metadata()))),
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

fn secret_size_error(path: &Path) -> SecretError {
    SecretError::invalid_location(format!(
        "{} exceeds safe size limit ({} bytes)",
        path.display(),
        FILE_SECRET_MAX_BYTES
    ))
}

fn secure_error(path: &Path, error: io::Error) -> SecretError {
    match error.kind() {
        io::ErrorKind::InvalidData if error.to_string().contains("safe size limit") => {
            secret_size_error(path)
        }
        io::ErrorKind::InvalidInput
        | io::ErrorKind::InvalidData
        | io::ErrorKind::NotADirectory
        | io::ErrorKind::PermissionDenied => {
            SecretError::invalid_location(format!("{}: {error}", path.display()))
        }
        _ => SecretError::io(path, error),
    }
}
