use super::validate_gateway_exact_identifier;
use crate::types::RuntimePolicyFile;
use crate::validate_helpers::validate_gateway_state_backend;
use crate::validate_secrets::{validate_gateway_secret_ref, validate_gateway_secret_source};
use anyhow::{Context, Result, bail};
use std::path::Path;

pub(super) fn validate_gateway_core(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    validate_gateway_secret_ref(
        policy,
        path,
        "gateway.auth_token_ref",
        policy.gateway.auth_token_ref.as_ref(),
    )?;
    validate_gateway_secret_ref(
        policy,
        path,
        "gateway.provider_api_key_ref",
        policy.gateway.provider_api_key_ref.as_ref(),
    )?;
    if policy.secrets.production {
        if policy.gateway.require_auth != Some(true) {
            bail!(
                "gateway.require_auth in {} must be true when secrets.production=true",
                path.display()
            );
        }
        if policy.gateway.provider_api_key_ref.is_none() {
            bail!(
                "gateway.provider_api_key_ref in {} is required when secrets.production=true",
                path.display()
            );
        }
        if policy.gateway.auth_token_ref.is_none() && policy.gateway.virtual_keys.is_empty() {
            bail!(
                "gateway.auth_token_ref or [[gateway.virtual_keys]] in {} is required when secrets.production=true",
                path.display()
            );
        }
    }
    if let Some(listen_addr) = policy.gateway.listen_addr.as_deref() {
        validate_gateway_exact_identifier(listen_addr, path, "gateway.listen_addr")?;
    }
    if let Some(provider) = policy.gateway.provider.as_deref() {
        validate_gateway_exact_identifier(provider, path, "gateway.provider")?;
        match provider.to_ascii_lowercase().as_str() {
            "anthropic" | "claude" | "copilot" | "github-copilot" | "github_copilot"
            | "deepseek" | "gemini" | "kiro" => {}
            _ => bail!(
                "gateway.provider in {} must be a supported provider preset",
                path.display()
            ),
        }
    }
    if let Some(base_url) = policy.gateway.base_url.as_deref() {
        if base_url.is_empty() {
            bail!("gateway.base_url in {} cannot be empty", path.display());
        }
        if base_url.chars().any(char::is_whitespace) {
            bail!(
                "gateway.base_url in {} must not contain whitespace",
                path.display()
            );
        }
    }
    Ok(())
}

pub(super) fn validate_gateway_state(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let state = &policy.gateway.state;
    if let Some(backend) = state.backend.as_deref() {
        validate_gateway_state_backend(backend)
            .with_context(|| format!("gateway.state.backend in {} is invalid", path.display()))?;
    }
    if matches!(state.sqlite_path.as_deref().map(str::trim), Some("")) {
        bail!(
            "gateway.state.sqlite_path in {} cannot be empty",
            path.display()
        );
    }
    if let Some(mode) = state.postgres_tls_mode.as_deref()
        && !matches!(mode, "verify-full" | "disable")
    {
        bail!(
            "gateway.state.postgres_tls_mode in {} must be verify-full or disable",
            path.display()
        );
    }
    if matches!(
        state.postgres_tls_ca_path.as_deref().map(str::trim),
        Some("")
    ) {
        bail!(
            "gateway.state.postgres_tls_ca_path in {} cannot be empty",
            path.display()
        );
    }
    if state.postgres_tls_ca_path.is_some()
        && matches!(state.postgres_tls_mode.as_deref(), Some("disable"))
    {
        bail!(
            "gateway.state.postgres_tls_ca_path in {} requires postgres_tls_mode=verify-full",
            path.display()
        );
    }
    if policy.secrets.production && matches!(state.postgres_tls_mode.as_deref(), Some("disable")) {
        bail!(
            "gateway.state.postgres_tls_mode in {} cannot be disable when secrets.production=true",
            path.display()
        );
    }
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.state.postgres_url",
        state.postgres_url_env.as_deref(),
        state.postgres_url_ref.as_ref(),
        false,
    )?;
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.state.redis_url",
        state.redis_url_env.as_deref(),
        state.redis_url_ref.as_ref(),
        false,
    )?;
    match state
        .backend
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("postgres")
            if state.postgres_url_env.is_none() && state.postgres_url_ref.is_none() =>
        {
            bail!(
                "gateway.state.postgres_url_env or gateway.state.postgres_url_ref in {} is required when gateway.state.backend=postgres",
                path.display()
            );
        }
        Some("redis") if state.redis_url_env.is_none() && state.redis_url_ref.is_none() => {
            bail!(
                "gateway.state.redis_url_env or gateway.state.redis_url_ref in {} is required when gateway.state.backend=redis",
                path.display()
            );
        }
        _ => {}
    }
    Ok(())
}
