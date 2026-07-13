use anyhow::Result;
use prodex_core::AppPaths;
pub use prodex_presidio::{PresidioLanguageMode, RuntimePresidioRedactionConfig};

pub(crate) fn runtime_presidio_redaction_config(
    paths: &AppPaths,
) -> Result<RuntimePresidioRedactionConfig> {
    prodex_presidio::runtime_presidio_redaction_config(&paths.root)
}

pub(crate) fn runtime_governed_presidio_redaction_config(
    paths: &AppPaths,
    runtime_config: &crate::RuntimeConfig,
) -> Result<RuntimePresidioRedactionConfig> {
    let config = runtime_presidio_redaction_config(paths)?;
    if runtime_config.governance.mode != prodex_config::GovernanceMode::Personal {
        prodex_presidio::validate_enterprise_presidio_endpoints(&config)?;
    }
    if runtime_config.governance.mode == prodex_config::GovernanceMode::BankEnforce
        && !config.fail_closed
    {
        anyhow::bail!("bank governance mode requires fail-closed Presidio inspection");
    }
    Ok(config)
}
