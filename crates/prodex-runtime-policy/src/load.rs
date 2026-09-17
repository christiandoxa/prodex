use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use crate::cache::{
    RuntimePolicyCacheInvalidationPlan, cached_policy_for, replace_cached_policy,
    store_cached_policy,
};
use crate::paths::{resolve_runtime_policy_path, runtime_policy_path};
use crate::types::{
    PRODEX_POLICY_VERSION, RuntimePolicyConfig, RuntimePolicyFile, RuntimePolicyRuntimeSettings,
    RuntimePolicySecretsSettings,
};

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
    if parsed.version != PRODEX_POLICY_VERSION {
        bail!(
            "unsupported policy version {} in {}; expected {}",
            parsed.version,
            path.display(),
            PRODEX_POLICY_VERSION
        );
    }

    let runtime = RuntimePolicyRuntimeSettings {
        log_format: parsed.runtime.log_format,
        log_dir: parsed
            .runtime
            .log_dir
            .as_deref()
            .map(|value| resolve_runtime_policy_path(root, value))
            .transpose()?,
    };
    let backend = parsed
        .secrets
        .backend
        .as_deref()
        .map(str::parse::<secret_store::SecretBackendKind>)
        .transpose()
        .map_err(anyhow::Error::new)?;
    let keyring_service = parsed.secrets.keyring_service;
    if keyring_service
        .as_deref()
        .is_some_and(|value| value.is_empty() || value.chars().any(char::is_whitespace))
    {
        bail!(
            "secrets.keyring_service in {} must be non-empty without whitespace",
            path.display()
        );
    }
    if keyring_service.is_some() && backend != Some(secret_store::SecretBackendKind::Keyring) {
        bail!(
            "secrets.keyring_service in {} requires secrets.backend=keyring",
            path.display()
        );
    }
    if backend == Some(secret_store::SecretBackendKind::Keyring) && keyring_service.is_none() {
        bail!(
            "secrets.keyring_service in {} is required when secrets.backend=keyring",
            path.display()
        );
    }

    Ok(Some(RuntimePolicyConfig {
        path,
        version: parsed.version,
        runtime,
        runtime_proxy: parsed.runtime_proxy,
        secrets: RuntimePolicySecretsSettings {
            backend,
            keyring_service,
        },
    }))
}
