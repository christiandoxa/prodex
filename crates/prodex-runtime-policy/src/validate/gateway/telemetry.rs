use super::validate_gateway_exact_identifier;
use crate::types::{RuntimeGovernanceMode, RuntimePolicyFile};
use crate::validate_helpers::{
    gateway_observability_http_endpoint_has_http_host, validate_gateway_guardrail_webhook_phase,
    validate_gateway_observability_http_schema,
};
use crate::validate_secrets::validate_gateway_secret_source;
use anyhow::{Context, Result, bail};
use std::path::Path;

const GATEWAY_WEBHOOK_HOST_ALLOWLIST_MAX_ENTRIES: usize = 32;

pub(super) fn validate_gateway_observability(
    policy: &RuntimePolicyFile,
    path: &Path,
) -> Result<()> {
    let observability = &policy.gateway.observability;
    for (index, sink) in observability.sinks.iter().enumerate() {
        validate_gateway_exact_identifier(
            sink,
            path,
            &format!("gateway.observability.sinks[{index}]"),
        )?;
    }
    if let Some(value) = observability.call_id_header.as_deref() {
        validate_gateway_exact_identifier(value, path, "gateway.observability.call_id_header")?;
    }
    if matches!(observability.jsonl_path.as_deref().map(str::trim), Some("")) {
        bail!(
            "gateway.observability.jsonl_path in {} cannot be empty",
            path.display()
        );
    }
    if let Some(endpoint) = observability.http_endpoint.as_deref() {
        validate_http_endpoint(endpoint, path, "gateway.observability.http_endpoint")?;
    }
    if let Some(endpoint) = observability.siem_endpoint.as_deref() {
        validate_http_endpoint(endpoint, path, "gateway.observability.siem_endpoint")?;
    }
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.observability.http_bearer_token",
        observability.http_bearer_token_env.as_deref(),
        observability.http_bearer_token_ref.as_ref(),
        false,
    )?;
    crate::validate_secrets::validate_gateway_secret_ref(
        policy,
        path,
        "gateway.observability.siem_bearer_token_ref",
        observability.siem_bearer_token_ref.as_ref(),
    )?;
    for (field, reference) in [
        (
            "gateway.observability.siem_mtls_identity_ref",
            observability.siem_mtls_identity_ref.as_ref(),
        ),
        (
            "gateway.observability.siem_signing_key_ref",
            observability.siem_signing_key_ref.as_ref(),
        ),
    ] {
        crate::validate_secrets::validate_gateway_secret_ref(policy, path, field, reference)?;
    }
    if observability.siem_endpoint.is_some() != observability.siem_bearer_token_ref.is_some() {
        bail!(
            "gateway.observability SIEM endpoint and SecretRef must be configured together in {}",
            path.display()
        );
    }
    let siem_configured = observability.siem_endpoint.is_some();
    if !siem_configured
        && (observability.siem_mtls_identity_ref.is_some()
            || observability.siem_signing_key_ref.is_some()
            || observability.siem_max_batch_events.is_some()
            || observability.siem_max_batch_bytes.is_some()
            || observability.siem_max_attempts.is_some()
            || observability.siem_retry_base_ms.is_some()
            || observability.siem_retry_max_ms.is_some()
            || observability.siem_max_lag_ms.is_some())
    {
        bail!(
            "gateway.observability SIEM worker settings in {} require a SIEM endpoint",
            path.display()
        );
    }
    if observability
        .siem_max_batch_events
        .is_some_and(|value| value == 0 || value > 256)
        || observability
            .siem_max_batch_bytes
            .is_some_and(|value| !(1024..=1024 * 1024).contains(&value))
        || observability
            .siem_max_attempts
            .is_some_and(|value| value == 0 || value > 32)
        || observability
            .siem_retry_base_ms
            .is_some_and(|value| value == 0 || value > 60_000)
        || observability
            .siem_retry_max_ms
            .is_some_and(|value| value == 0 || value > 3_600_000)
        || observability
            .siem_max_lag_ms
            .is_some_and(|value| !(1_000..=86_400_000).contains(&value))
    {
        bail!(
            "gateway.observability SIEM worker bounds in {} are invalid",
            path.display()
        );
    }
    if let (Some(base), Some(max)) = (
        observability.siem_retry_base_ms,
        observability.siem_retry_max_ms,
    ) && max < base
    {
        bail!(
            "gateway.observability.siem_retry_max_ms in {} cannot be below siem_retry_base_ms",
            path.display()
        );
    }
    if let Some(schema) = observability.http_schema.as_deref() {
        validate_gateway_observability_http_schema(schema).with_context(|| {
            format!(
                "gateway.observability.http_schema in {} is invalid",
                path.display()
            )
        })?;
    }
    Ok(())
}

pub(super) fn validate_gateway_guardrails(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let guardrails = &policy.gateway.guardrails;
    for (field, keywords) in [
        ("blocked_keywords", guardrails.blocked_keywords.as_slice()),
        (
            "blocked_output_keywords",
            guardrails.blocked_output_keywords.as_slice(),
        ),
    ] {
        for (index, keyword) in keywords.iter().enumerate() {
            if keyword.trim().is_empty() {
                bail!(
                    "gateway.guardrails.{field}[{index}] in {} cannot be empty",
                    path.display()
                );
            }
        }
    }
    for (index, model) in guardrails.allowed_models.iter().enumerate() {
        validate_gateway_exact_identifier(
            model,
            path,
            &format!("gateway.guardrails.allowed_models[{index}]"),
        )?;
    }
    if let Some(url) = guardrails.webhook_url.as_deref() {
        validate_http_endpoint(url, path, "gateway.guardrails.webhook_url")?;
        if matches!(policy.governance.mode, RuntimeGovernanceMode::BankEnforce)
            && (!url.starts_with("https://") || guardrails.webhook_host_allowlist.is_empty())
        {
            bail!(
                "gateway.guardrails.webhook_url in {} requires HTTPS and a host allowlist in bank_enforce",
                path.display()
            );
        }
    }
    if guardrails.webhook_host_allowlist.len() > GATEWAY_WEBHOOK_HOST_ALLOWLIST_MAX_ENTRIES {
        bail!(
            "gateway.guardrails.webhook_host_allowlist in {} must contain at most {} entries",
            path.display(),
            GATEWAY_WEBHOOK_HOST_ALLOWLIST_MAX_ENTRIES
        );
    }
    for (index, host) in guardrails.webhook_host_allowlist.iter().enumerate() {
        if host.is_empty()
            || host.len() > 253
            || host.chars().any(char::is_whitespace)
            || host
                .chars()
                .any(|character| matches!(character, '/' | '@' | ':' | '?' | '#' | '[' | ']'))
        {
            bail!(
                "gateway.guardrails.webhook_host_allowlist[{index}] in {} must be an exact DNS host",
                path.display()
            );
        }
    }
    for (index, phase) in guardrails.webhook_phases.iter().enumerate() {
        validate_gateway_guardrail_webhook_phase(phase).with_context(|| {
            format!(
                "gateway.guardrails.webhook_phases[{index}] in {} is invalid",
                path.display()
            )
        })?;
    }
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.guardrails.webhook_bearer_token",
        guardrails.webhook_bearer_token_env.as_deref(),
        guardrails.webhook_bearer_token_ref.as_ref(),
        false,
    )?;
    Ok(())
}

fn validate_http_endpoint(value: &str, path: &Path, field: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{field} in {} cannot be empty", path.display());
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{field} in {} must not contain whitespace", path.display());
    }
    if !gateway_observability_http_endpoint_has_http_host(value) {
        bail!(
            "{field} in {} must be an http(s) URL with host",
            path.display()
        );
    }
    Ok(())
}
