use prodex_domain::{
    SecretMaterial, SecretProvider, SecretProviderDescriptor, SecretResolutionError,
    SecretResolutionRequest,
};
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::{
    SecretError,
    secure_file::{FileSecurity, SecureDirectory},
};

pub(crate) const PROJECTED_SECRET_MAX_BYTES: u64 = 64 * 1024;
const PROJECTED_VERSION_MAX_BYTES: u64 = 128;

#[derive(Clone)]
pub struct ProjectedSecretProvider {
    root: Arc<SecureDirectory>,
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

        let root = SecureDirectory::open(root.as_ref(), false)
            .map_err(|error| SecretError::io(root.as_ref(), error))?;
        Ok(Self {
            root: Arc::new(root),
            provider_name,
        })
    }

    pub(crate) fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn projected_generation_root(&self) -> Result<Option<SecureDirectory>, SecretResolutionError> {
        let generation = match self.root.read_link_component("..data".as_ref()) {
            Ok(generation) => generation,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(map_resolution_io(error)),
        };
        let generation = PathBuf::from(generation);
        let mut components = generation.components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(SecretResolutionError::PermissionDenied);
        }
        self.root
            .open_directory_beneath(&generation)
            .map(Some)
            .map_err(map_resolution_io)
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
        root: &SecureDirectory,
        relative: &str,
        max_bytes: u64,
    ) -> Result<Vec<u8>, SecretResolutionError> {
        let opened = root
            .open_file_beneath(Path::new(relative), FileSecurity::Projected)
            .map_err(map_resolution_io)?;
        if opened.metadata().len() > max_bytes {
            return Err(SecretResolutionError::ProviderUnavailable);
        }
        opened.read_bounded(max_bytes).map_err(map_resolution_io)
    }

    fn read_version(
        &self,
        root: &SecureDirectory,
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
        let root = generation.as_ref().unwrap_or(&self.root);
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
        io::ErrorKind::InvalidInput
        | io::ErrorKind::NotADirectory
        | io::ErrorKind::PermissionDenied => SecretResolutionError::PermissionDenied,
        _ => SecretResolutionError::ProviderUnavailable,
    }
}
