mod cache;
mod load;
mod paths;
mod types;

pub use self::cache::{
    RuntimePolicyCacheInvalidationPlan, clear_runtime_policy_cache,
    invalidate_runtime_policy_cache_for, plan_runtime_policy_cache_invalidation,
};
pub use self::load::{
    load_runtime_policy_cached, load_runtime_policy_from_root, reload_runtime_policy_cached,
    reload_runtime_policy_cached_with_invalidation,
};
pub use self::paths::{resolve_runtime_policy_path, runtime_policy_path};
pub use self::types::{
    PRODEX_POLICY_FILE_NAME, PRODEX_POLICY_VERSION, PRODEX_RUNTIME_PROXY_PRESET_ENV,
    RuntimeLogFormat, RuntimePolicyConfig, RuntimePolicyFile, RuntimePolicyProxyPreset,
    RuntimePolicyProxyPresetSelection, RuntimePolicyProxySettings, RuntimePolicyRuntimeFile,
    RuntimePolicyRuntimeSettings, RuntimePolicySecretsFile, RuntimePolicySecretsSettings,
    RuntimePolicySummary,
};

use anyhow::Result;
use prodex_core::AppPaths;

pub fn ensure_runtime_policy_valid() -> Result<()> {
    if runtime_policy_enabled_for_current_process() {
        let paths = AppPaths::discover()?;
        let _ = load_runtime_policy_cached(&paths.root)?;
    }
    Ok(())
}

pub fn runtime_policy_summary() -> Result<Option<RuntimePolicySummary>> {
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

pub fn runtime_policy_runtime() -> Option<RuntimePolicyRuntimeSettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.runtime)
}

pub fn runtime_policy_proxy() -> Option<RuntimePolicyProxySettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    runtime_policy_proxy_from_root(&paths.root, runtime_proxy_preset_from_env())
}

fn runtime_policy_proxy_from_root(
    root: &std::path::Path,
    env_preset: Option<RuntimePolicyProxyPreset>,
) -> Option<RuntimePolicyProxySettings> {
    if let Some(config) = load_runtime_policy_cached(root).ok().flatten() {
        return Some(config.runtime_proxy.with_effective_preset(env_preset));
    }
    env_preset
        .map(|preset| RuntimePolicyProxySettings::default().with_effective_preset(Some(preset)))
}

pub fn runtime_proxy_preset_from_env() -> Option<RuntimePolicyProxyPreset> {
    std::env::var(PRODEX_RUNTIME_PROXY_PRESET_ENV)
        .ok()
        .and_then(|value| RuntimePolicyProxyPreset::parse(&value))
}

pub fn runtime_policy_secrets() -> Option<RuntimePolicySecretsSettings> {
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
