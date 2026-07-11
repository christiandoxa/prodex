use anyhow::{Result, bail};
use prodex_domain::SecretRef;
use std::path::Path;

use crate::types::RuntimePolicyFile;

pub(super) fn validate_secret_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if let Some(root) = policy.secrets.projected_root.as_deref()
        && root.trim().is_empty()
    {
        bail!(
            "secrets.projected_root in {} cannot be empty",
            path.display()
        );
    }
    if let Some(provider) = policy.secrets.projected_provider.as_deref()
        && !exact_identifier(provider)
    {
        bail!(
            "secrets.projected_provider in {} must be non-empty without whitespace",
            path.display()
        );
    }
    if policy.secrets.projected_root.is_some() != policy.secrets.projected_provider.is_some() {
        bail!(
            "secrets.projected_root and secrets.projected_provider in {} must be configured together",
            path.display()
        );
    }
    if policy.secrets.production
        && (policy.secrets.projected_root.is_none() || policy.secrets.projected_provider.is_none())
    {
        bail!(
            "secrets.projected_root and secrets.projected_provider in {} are required when secrets.production=true",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn validate_gateway_secret_source(
    policy: &RuntimePolicyFile,
    path: &Path,
    field: &str,
    env_name: Option<&str>,
    reference: Option<&SecretRef>,
    required: bool,
) -> Result<()> {
    if env_name.is_some() && reference.is_some() {
        bail!(
            "{field} in {} must use exactly one secret source",
            path.display()
        );
    }
    if required && env_name.is_none() && reference.is_none() {
        bail!("{field} in {} requires a secret source", path.display());
    }
    if let Some(env_name) = env_name {
        if !exact_identifier(env_name) {
            bail!(
                "{field}_env in {} must be non-empty without whitespace",
                path.display()
            );
        }
        if policy.secrets.production {
            bail!(
                "{field}_env in {} is forbidden when secrets.production=true; use {field}_ref",
                path.display()
            );
        }
    }
    validate_gateway_secret_ref(policy, path, &format!("{field}_ref"), reference)
}

pub(super) fn validate_gateway_secret_ref(
    policy: &RuntimePolicyFile,
    path: &Path,
    field: &str,
    reference: Option<&SecretRef>,
) -> Result<()> {
    let Some(reference) = reference else {
        return Ok(());
    };
    if !reference.is_well_formed() {
        bail!("{field} in {} is malformed", path.display());
    }
    if policy.secrets.production
        && policy.secrets.projected_provider.as_deref() != Some(reference.provider())
    {
        bail!(
            "{field} in {} must use secrets.projected_provider when secrets.production=true",
            path.display()
        );
    }
    Ok(())
}

fn exact_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
