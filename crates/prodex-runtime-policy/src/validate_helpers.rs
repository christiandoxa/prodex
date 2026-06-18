use anyhow::{Result, bail};
use std::path::Path;

pub(crate) fn validate_gateway_route_strategy(value: &str) -> Result<()> {
    match value.trim().to_ascii_lowercase().as_str() {
        "fallback" | "ordered-fallback" | "ordered_fallback" | "round-robin" | "round_robin"
        | "rr" | "first" | "first-available" | "first_available" | "ordered" | "least-busy"
        | "least_busy" | "least-busy-model" | "least_busy_model" | "lowest-cost"
        | "lowest_cost" | "cost" | "cost-optimized" | "cost_optimized" | "lowest-latency"
        | "lowest_latency" | "latency" | "latency-optimized" | "latency_optimized" | "rpm"
        | "rpm-headroom" | "rpm_headroom" | "tpm" | "tpm-headroom" | "tpm_headroom" => Ok(()),
        "" => bail!("strategy cannot be empty"),
        _ => bail!(
            "strategy must be one of fallback, round-robin, first, least-busy, lowest-cost, lowest-latency, rpm, tpm"
        ),
    }
}

pub(crate) fn validate_gateway_observability_http_schema(value: &str) -> Result<()> {
    match value.trim().to_ascii_lowercase().as_str() {
        "generic" | "otel" | "opentelemetry" | "datadog" | "langfuse" => Ok(()),
        "" => bail!("schema cannot be empty"),
        _ => bail!("schema must be one of generic, otel, datadog, langfuse"),
    }
}

pub(crate) fn validate_gateway_state_backend(value: &str) -> Result<()> {
    match value.trim().to_ascii_lowercase().as_str() {
        "file" | "sqlite" | "postgres" | "redis" => Ok(()),
        "" => bail!("backend cannot be empty"),
        _ => bail!("backend must be one of file, sqlite, postgres, redis"),
    }
}

pub(crate) fn validate_gateway_admin_role(value: &str) -> Result<()> {
    match value.trim().to_ascii_lowercase().as_str() {
        "admin" | "write" | "writer" | "viewer" | "read" | "readonly" | "read-only" => Ok(()),
        "" => bail!("role cannot be empty"),
        _ => bail!("role must be one of admin, viewer"),
    }
}

pub(crate) fn validate_gateway_guardrail_webhook_phase(value: &str) -> Result<()> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pre" | "request" | "post" | "response" => Ok(()),
        "" => bail!("phase cannot be empty"),
        _ => bail!("phase must be one of pre, post"),
    }
}

pub(crate) fn gateway_observability_http_endpoint_has_http_host(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") {
        return false;
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim();
    !host.is_empty() && !host.contains('@')
}

pub(crate) fn validate_optional_usize(
    value: Option<usize>,
    path: &Path,
    field: &str,
) -> Result<()> {
    if matches!(value, Some(0)) {
        bail!("{field} in {} must be greater than 0", path.display());
    }
    Ok(())
}

pub(crate) fn validate_optional_usize_allow_zero(
    _value: Option<usize>,
    _path: &Path,
    _field: &str,
) -> Result<()> {
    Ok(())
}

pub(crate) fn validate_optional_u64(value: Option<u64>, path: &Path, field: &str) -> Result<()> {
    if matches!(value, Some(0)) {
        bail!("{field} in {} must be greater than 0", path.display());
    }
    Ok(())
}

pub(crate) fn validate_optional_i64_percent(
    value: Option<i64>,
    path: &Path,
    field: &str,
) -> Result<()> {
    if let Some(value) = value
        && !(1..=10).contains(&value)
    {
        bail!("{field} in {} must be between 1 and 10", path.display());
    }
    Ok(())
}
