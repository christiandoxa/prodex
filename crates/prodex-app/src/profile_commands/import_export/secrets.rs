use anyhow::{Context, Result};
use std::path::Path;

pub(in crate::profile_commands) fn write_secret_text_file(
    path: &Path,
    content: &str,
) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), content)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}
