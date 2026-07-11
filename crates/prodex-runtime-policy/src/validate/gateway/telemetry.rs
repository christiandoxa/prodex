use super::validate_gateway_exact_identifier;
use crate::types::RuntimePolicyFile;
use crate::validate_helpers::{
    gateway_observability_http_endpoint_has_http_host, validate_gateway_guardrail_webhook_phase,
    validate_gateway_observability_http_schema,
};
use crate::validate_secrets::validate_gateway_secret_source;
use anyhow::{Context, Result, bail};
use std::path::Path;

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
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.observability.http_bearer_token",
        observability.http_bearer_token_env.as_deref(),
        observability.http_bearer_token_ref.as_ref(),
        false,
    )?;
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
