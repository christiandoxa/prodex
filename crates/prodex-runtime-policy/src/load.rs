use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::cache::{
    RuntimePolicyCacheInvalidationPlan, cached_policy_for, replace_cached_policy,
    store_cached_policy,
};
use crate::paths::{resolve_runtime_policy_path, runtime_policy_path};
use crate::types::{
    RuntimePolicyConfig, RuntimePolicyFile, RuntimePolicyRuntimeSettings,
    RuntimePolicySecretsSettings,
};
use crate::validate::validate_runtime_policy_file;
use crate::validate_secrets::parse_secret_backend_kind;

pub fn load_runtime_policy_cached(root: &Path) -> Result<Option<RuntimePolicyConfig>> {
    if let Some(cached) = cached_policy_for(root) {
        return Ok(cached);
    }
    let loaded = load_runtime_policy_from_root(root)?;
    store_cached_policy(root, loaded.clone());
    Ok(loaded)
}

pub fn reload_runtime_policy_cached(root: &Path) -> Result<Option<RuntimePolicyConfig>> {
    reload_runtime_policy_cached_with_invalidation(root).map(|(_, policy)| policy)
}

pub fn reload_runtime_policy_cached_with_invalidation(
    root: &Path,
) -> Result<(
    RuntimePolicyCacheInvalidationPlan,
    Option<RuntimePolicyConfig>,
)> {
    let loaded = load_runtime_policy_from_root(root)?;
    let invalidation = replace_cached_policy(root, loaded.clone());
    Ok((invalidation, loaded))
}

pub fn load_runtime_policy_from_root(root: &Path) -> Result<Option<RuntimePolicyConfig>> {
    let path = runtime_policy_path(root);
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: RuntimePolicyFile =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    validate_runtime_policy_file(&parsed, &path)?;

    let runtime = RuntimePolicyRuntimeSettings {
        log_format: parsed.runtime.log_format,
        log_dir: parsed
            .runtime
            .log_dir
            .as_deref()
            .map(|value| resolve_runtime_policy_path(root, value))
            .transpose()?,
    };
    let secrets = RuntimePolicySecretsSettings {
        backend: parsed
            .secrets
            .backend
            .as_deref()
            .map(parse_secret_backend_kind)
            .transpose()?,
        keyring_service: parsed.secrets.keyring_service,
        production: parsed.secrets.production,
        projected_root: parsed
            .secrets
            .projected_root
            .as_deref()
            .map(|value| resolve_runtime_policy_path(root, value))
            .transpose()?,
        projected_provider: parsed.secrets.projected_provider,
    };

    Ok(Some(RuntimePolicyConfig {
        path,
        version: parsed.version,
        service_mode: parsed.service_mode,
        runtime,
        runtime_proxy: parsed.runtime_proxy,
        gateway: parsed.gateway,
        secrets,
        governance: parsed.governance,
    }))
}
