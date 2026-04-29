#[cfg(test)]
pub(super) use prodex_runtime_policy::clear_runtime_policy_cache;
pub(super) use prodex_runtime_policy::{
    RuntimeLogFormat, RuntimePolicyProxySettings, RuntimePolicyRuntimeSettings,
    RuntimePolicySecretsSettings, RuntimePolicySummary,
};

use anyhow::Result;
use prodex_runtime_policy::load_runtime_policy_cached;

use super::AppPaths;

pub(super) fn ensure_runtime_policy_valid() -> Result<()> {
    if !runtime_policy_enabled_for_current_process() {
        return Ok(());
    }
    let paths = AppPaths::discover()?;
    let _ = load_runtime_policy_cached(&paths.root)?;
    Ok(())
}

pub(super) fn runtime_policy_summary() -> Result<Option<RuntimePolicySummary>> {
    if !runtime_policy_enabled_for_current_process() {
        return Ok(None);
    }
    let paths = AppPaths::discover()?;
    Ok(
        load_runtime_policy_cached(&paths.root)?.map(|config| RuntimePolicySummary {
            path: config.path,
            version: config.version,
        }),
    )
}

pub(super) fn runtime_policy_runtime() -> Option<RuntimePolicyRuntimeSettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.runtime)
}

pub(super) fn runtime_policy_proxy() -> Option<RuntimePolicyProxySettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.runtime_proxy)
}

pub(super) fn runtime_policy_secrets() -> Option<RuntimePolicySecretsSettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.secrets)
}

#[cfg(test)]
fn runtime_policy_enabled_for_current_process() -> bool {
    std::env::var_os("PRODEX_HOME").is_some()
}

#[cfg(not(test))]
fn runtime_policy_enabled_for_current_process() -> bool {
    true
}
