use prodex_domain::{
    SecretMaterial, SecretProvider, SecretProviderDescriptor, SecretResolutionError,
    SecretResolutionRequest,
};
use std::env;
use std::fmt;
use std::path::Path;
use zeroize::Zeroizing;

use crate::{PROJECTED_SECRET_MAX_BYTES, ProjectedSecretProvider, SecretError};

pub const DEVELOPMENT_SECRET_PROVIDER_NAME: &str = "development";

const ENV_REFERENCE_PREFIX: &str = "env:";
const FILE_REFERENCE_PREFIX: &str = "file:";

fn process_environment(name: &str) -> Result<String, env::VarError> {
    env::var(name)
}

/// A local-only secret provider for development configuration.
///
/// References use `env:NAME` for an environment variable or
/// `file:relative/path` for a private file below the configured root.
#[derive(Clone)]
pub struct DevelopmentSecretProvider {
    projected: ProjectedSecretProvider,
    environment_reader: fn(&str) -> Result<String, env::VarError>,
}

impl DevelopmentSecretProvider {
    pub fn new(
        root: impl AsRef<Path>,
        provider_name: impl Into<String>,
    ) -> Result<Self, SecretError> {
        Ok(Self {
            projected: ProjectedSecretProvider::new(root, provider_name)?,
            environment_reader: process_environment,
        })
    }

    /// Replaces environment access with a deterministic reader.
    ///
    /// This keeps callers and tests from mutating process-global environment
    /// state while preserving the same validation and size limits.
    pub fn with_environment_reader(
        mut self,
        reader: fn(&str) -> Result<String, env::VarError>,
    ) -> Self {
        self.environment_reader = reader;
        self
    }

    pub fn for_development(root: impl AsRef<Path>) -> Result<Self, SecretError> {
        Self::new(root, DEVELOPMENT_SECRET_PROVIDER_NAME)
    }

    pub fn provider_name(&self) -> &str {
        self.projected.provider_name()
    }
}

impl fmt::Debug for DevelopmentSecretProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DevelopmentSecretProvider")
            .field("root", &"<redacted>")
            .field("provider_name", &"<redacted>")
            .finish()
    }
}

impl SecretProvider for DevelopmentSecretProvider {
    fn descriptor(&self) -> SecretProviderDescriptor {
        SecretProviderDescriptor::development(self.provider_name().to_string())
    }

    fn resolve(
        &self,
        request: &SecretResolutionRequest,
    ) -> Result<SecretMaterial, SecretResolutionError> {
        if !request.reference.is_well_formed()
            || request.reference.provider() != self.provider_name()
        {
            return Err(SecretResolutionError::NotFound);
        }

        let bytes = if let Some(name) = request.reference.name().strip_prefix(ENV_REFERENCE_PREFIX)
        {
            read_environment(name, self.environment_reader)?
        } else if let Some(name) = request.reference.name().strip_prefix(FILE_REFERENCE_PREFIX) {
            self.projected
                .read_projected_file(name, PROJECTED_SECRET_MAX_BYTES)?
        } else {
            return Err(SecretResolutionError::PermissionDenied);
        };

        if request.reference.version().is_some() {
            return Err(SecretResolutionError::StaleVersion);
        }
        Ok(SecretMaterial::new(bytes, None::<String>))
    }
}

fn read_environment(
    name: &str,
    reader: fn(&str) -> Result<String, env::VarError>,
) -> Result<Vec<u8>, SecretResolutionError> {
    if !valid_environment_name(name) {
        return Err(SecretResolutionError::PermissionDenied);
    }

    let mut value = match reader(name) {
        Ok(value) => Zeroizing::new(value),
        Err(env::VarError::NotPresent) => return Err(SecretResolutionError::NotFound),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(SecretResolutionError::ProviderUnavailable);
        }
    };
    if value.len() as u64 > PROJECTED_SECRET_MAX_BYTES {
        return Err(SecretResolutionError::ProviderUnavailable);
    }
    Ok(std::mem::take(&mut *value).into_bytes())
}

fn valid_environment_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
