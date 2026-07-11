use anyhow::Result;
use prodex_core::AppPaths;
pub use prodex_presidio::{PresidioLanguageMode, RuntimePresidioRedactionConfig};

pub(crate) fn runtime_presidio_redaction_config(
    paths: &AppPaths,
) -> Result<RuntimePresidioRedactionConfig> {
    prodex_presidio::runtime_presidio_redaction_config(&paths.root)
}
