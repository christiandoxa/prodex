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

    pub fn read_bounded(
        &self,
        location: &SecretLocation,
        max_bytes: u64,
    ) -> Result<Option<SecretValue>, SecretError> {
        let path = file_path(location)?;
        let Some(opened) = secure_file::open_file(path, FileSecurity::Private)
            .map_err(|error| secure_error(path, error))?
        else {
            return Ok(None);
        };
        if opened.metadata().len() > max_bytes {
            return Err(secret_size_error(path, max_bytes));
        }
        let bytes = opened
            .read_bounded(max_bytes)
            .map_err(|error| secure_error(path, error))?;

        match String::from_utf8(bytes) {
            Ok(text) => Ok(Some(SecretValue::text(text))),
            Err(err) => Ok(Some(SecretValue::bytes(err.into_bytes()))),
        }
    }

    pub fn write_bounded(
        &self,
        location: &SecretLocation,
        value: SecretValue,
        max_bytes: u64,
    ) -> Result<(), SecretError> {
        let path = file_path(location)?;
        value.with_bytes(|bytes| {
            if bytes.len() as u64 > max_bytes {
                return Err(secret_size_error(path, max_bytes));
            }
            secure_file::write_private_atomic(path, bytes)
                .map_err(|error| secure_error(path, error))
        })
    }

    /// Removes only the final entry beneath validated parent directories.
    ///
    /// This is for quarantining an invalid legacy entry after a normal private
    /// open failed. The entry is never followed.
    pub fn remove_untrusted_entry(&self, location: &SecretLocation) -> Result<(), SecretError> {
        let path = file_path(location)?;
        secure_file::remove_untrusted_entry(path).map_err(|error| secure_error(path, error))
    }
}

impl SecretBackend for FileSecretBackend {
    fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {
        self.read_bounded(location, FILE_SECRET_MAX_BYTES)
    }

    fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {
        self.write_bounded(location, value, FILE_SECRET_MAX_BYTES)
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

fn secret_size_error(path: &Path, max_bytes: u64) -> SecretError {
    SecretError::invalid_location(format!(
        "{} exceeds safe size limit ({} bytes)",
        path.display(),
        max_bytes
    ))
}

fn secure_error(path: &Path, error: io::Error) -> SecretError {
    match error.kind() {
        io::ErrorKind::InvalidData if error.to_string().contains("safe size limit") => {
            SecretError::invalid_location(format!("{}: {error}", path.display()))
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
