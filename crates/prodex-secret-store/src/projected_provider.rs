use prodex_domain::{
    SecretMaterial, SecretProvider, SecretProviderDescriptor, SecretResolutionError,
    SecretResolutionRequest,
};
use std::fmt;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Component, Path, PathBuf};

use crate::SecretError;

pub(crate) const PROJECTED_SECRET_MAX_BYTES: u64 = 64 * 1024;
const PROJECTED_VERSION_MAX_BYTES: u64 = 128;

#[derive(Clone)]
pub struct ProjectedSecretProvider {
    root: PathBuf,
    provider_name: String,
}

impl ProjectedSecretProvider {
    pub fn new(
        root: impl AsRef<Path>,
        provider_name: impl Into<String>,
    ) -> Result<Self, SecretError> {
        let provider_name = provider_name.into();
        let provider_probe =
            prodex_domain::SecretRef::new(provider_name.clone(), "probe", None::<String>);
        if !provider_probe.is_well_formed() {
            return Err(SecretError::invalid_location(
                "projected secret provider name is invalid",
            ));
        }

        let root = fs::canonicalize(root.as_ref())
            .map_err(|error| SecretError::io(root.as_ref(), error))?;
        let metadata = fs::metadata(&root).map_err(|error| SecretError::io(&root, error))?;
        if !metadata.is_dir() {
            return Err(SecretError::invalid_location(
                "projected secret root must be a directory",
            ));
        }
        Ok(Self {
            root,
            provider_name,
        })
    }

    pub(crate) fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn resolve_path_from(
        &self,
        root: &Path,
        relative: &str,
    ) -> Result<PathBuf, SecretResolutionError> {
        let relative = Path::new(relative);
        if relative.components().next().is_none()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(SecretResolutionError::PermissionDenied);
        }
        let resolved = fs::canonicalize(root.join(relative)).map_err(map_resolution_io)?;
        if resolved == root || !resolved.starts_with(root) {
            return Err(SecretResolutionError::PermissionDenied);
        }
        Ok(resolved)
    }

    fn projected_generation_root(&self) -> Result<Option<PathBuf>, SecretResolutionError> {
        let data_link = self.root.join("..data");
        match fs::symlink_metadata(&data_link) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(map_resolution_io(error)),
        }

        let resolved = fs::canonicalize(data_link).map_err(map_resolution_io)?;
        if resolved == self.root || !resolved.starts_with(&self.root) {
            return Err(SecretResolutionError::PermissionDenied);
        }
        if !fs::metadata(&resolved).map_err(map_resolution_io)?.is_dir() {
            return Err(SecretResolutionError::ProviderUnavailable);
        }
        Ok(Some(resolved))
    }

    pub(crate) fn read_projected_file(
        &self,
        relative: &str,
        max_bytes: u64,
    ) -> Result<Vec<u8>, SecretResolutionError> {
        self.read_projected_file_from(&self.root, relative, max_bytes)
    }

    fn read_projected_file_from(
        &self,
        root: &Path,
        relative: &str,
        max_bytes: u64,
    ) -> Result<Vec<u8>, SecretResolutionError> {
        let path = self.resolve_path_from(root, relative)?;
        let metadata = fs::metadata(&path).map_err(map_resolution_io)?;
        if !metadata.is_file() || metadata.len() > max_bytes {
            return Err(SecretResolutionError::ProviderUnavailable);
        }
        require_private_mode(&metadata)?;

        let file = fs::File::open(&path).map_err(map_resolution_io)?;
        let opened = file.metadata().map_err(map_resolution_io)?;
        if !same_file(&metadata, &opened) {
            return Err(SecretResolutionError::ProviderUnavailable);
        }
        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(map_resolution_io)?;
        if bytes.len() as u64 > max_bytes {
            return Err(SecretResolutionError::ProviderUnavailable);
        }
        Ok(bytes)
    }

    fn read_version(
        &self,
        root: &Path,
        name: &str,
    ) -> Result<Option<String>, SecretResolutionError> {
        let version_name = format!("{name}.version");
        match self.read_projected_file_from(root, &version_name, PROJECTED_VERSION_MAX_BYTES) {
            Ok(bytes) => String::from_utf8(bytes)
                .ok()
                .filter(|value| {
                    prodex_domain::SecretRef::new(
                        self.provider_name.clone(),
                        "probe",
                        Some(value.clone()),
                    )
                    .is_well_formed()
                })
                .map(Some)
                .ok_or(SecretResolutionError::ProviderUnavailable),
            Err(SecretResolutionError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl fmt::Debug for ProjectedSecretProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectedSecretProvider")
            .field("root", &"<redacted>")
            .field("provider_name", &"<redacted>")
            .finish()
    }
}

impl SecretProvider for ProjectedSecretProvider {
    fn descriptor(&self) -> SecretProviderDescriptor {
        SecretProviderDescriptor::external(self.provider_name.clone())
    }

    fn resolve(
        &self,
        request: &SecretResolutionRequest,
    ) -> Result<SecretMaterial, SecretResolutionError> {
        if !request.reference.is_well_formed() || request.reference.provider() != self.provider_name
        {
            return Err(SecretResolutionError::NotFound);
        }
        let generation = self.projected_generation_root()?;
        let root = generation.as_deref().unwrap_or(&self.root);
        let bytes = self.read_projected_file_from(
            root,
            request.reference.name(),
            PROJECTED_SECRET_MAX_BYTES,
        )?;
        let version = self.read_version(root, request.reference.name())?;
        if request.reference.version().is_some()
            && request.reference.version() != version.as_deref()
        {
            return Err(SecretResolutionError::StaleVersion);
        }
        Ok(SecretMaterial::new(bytes, version))
    }
}

fn map_resolution_io(error: io::Error) -> SecretResolutionError {
    match error.kind() {
        io::ErrorKind::NotFound => SecretResolutionError::NotFound,
        io::ErrorKind::PermissionDenied => SecretResolutionError::PermissionDenied,
        _ => SecretResolutionError::ProviderUnavailable,
    }
}

#[cfg(unix)]
fn require_private_mode(metadata: &fs::Metadata) -> Result<(), SecretResolutionError> {
    use std::os::unix::fs::PermissionsExt as _;
    if metadata.permissions().mode() & 0o007 != 0 {
        return Err(SecretResolutionError::PermissionDenied);
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_private_mode(_metadata: &fs::Metadata) -> Result<(), SecretResolutionError> {
    Ok(())
}

#[cfg(unix)]
fn same_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn same_file(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}
