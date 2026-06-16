use anyhow::{Context, Result, bail};
use secret_store::SecretBackendKind;
use std::path::Path;

use crate::types::{PRODEX_POLICY_VERSION, RuntimePolicyFile};

pub fn parse_secret_backend_kind(value: &str) -> Result<SecretBackendKind> {
    value
        .parse::<SecretBackendKind>()
        .map_err(anyhow::Error::new)
}

pub fn validate_runtime_policy_file(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if policy.version != PRODEX_POLICY_VERSION {
        bail!(
            "unsupported prodex policy version {} in {}; expected {}",
            policy.version,
            path.display(),
            PRODEX_POLICY_VERSION
        );
    }

    if let Some(log_dir) = policy.runtime.log_dir.as_deref()
        && log_dir.trim().is_empty()
    {
        bail!("runtime.log_dir in {} cannot be empty", path.display());
    }
    let secret_backend = if let Some(backend) = policy.secrets.backend.as_deref() {
        Some(
            parse_secret_backend_kind(backend)
                .with_context(|| format!("invalid secrets.backend in {}", path.display()))?,
        )
    } else {
        None
    };
    if secret_backend == Some(SecretBackendKind::Keyring)
        && policy
            .secrets
            .keyring_service
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty())
    {
        bail!(
            "secrets.keyring_service in {} is required when secrets.backend=keyring",
            path.display()
        );
    }
    if let Some(service) = policy.secrets.keyring_service.as_deref()
        && service.trim().is_empty()
    {
        bail!(
            "secrets.keyring_service in {} cannot be empty",
            path.display()
        );
    }

    validate_runtime_proxy_policy(policy, path)?;
    validate_gateway_policy(policy, path)?;

    Ok(())
}

pub fn validate_gateway_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if matches!(
        policy.gateway.listen_addr.as_deref().map(str::trim),
        Some("")
    ) {
        bail!("gateway.listen_addr in {} cannot be empty", path.display());
    }
    if matches!(policy.gateway.provider.as_deref().map(str::trim), Some("")) {
        bail!("gateway.provider in {} cannot be empty", path.display());
    }
    if matches!(policy.gateway.base_url.as_deref().map(str::trim), Some("")) {
        bail!("gateway.base_url in {} cannot be empty", path.display());
    }
    for (index, alias) in policy.gateway.route_aliases.iter().enumerate() {
        let field = format!("gateway.route_aliases[{index}]");
        if alias.alias.trim().is_empty() {
            bail!("{field}.alias in {} cannot be empty", path.display());
        }
        if alias.models.is_empty() {
            bail!("{field}.models in {} cannot be empty", path.display());
        }
        if alias.models.iter().any(|model| model.trim().is_empty()) {
            bail!(
                "{field}.models in {} cannot contain empty values",
                path.display()
            );
        }
        if let Some(strategy) = alias.strategy.as_deref() {
            validate_gateway_route_strategy(strategy)
                .with_context(|| format!("{field}.strategy in {} is invalid", path.display()))?;
        }
        for (metric_index, metric) in alias.model_metrics.iter().enumerate() {
            let metric_field = format!("{field}.model_metrics[{metric_index}]");
            if metric.model.trim().is_empty() {
                bail!("{metric_field}.model in {} cannot be empty", path.display());
            }
            if !alias
                .models
                .iter()
                .any(|model| model.trim() == metric.model.trim())
            {
                bail!(
                    "{metric_field}.model in {} must match one of {field}.models",
                    path.display()
                );
            }
            validate_optional_u64(
                metric.input_cost_per_million_microusd,
                path,
                &format!("{metric_field}.input_cost_per_million_microusd"),
            )?;
            validate_optional_u64(
                metric.output_cost_per_million_microusd,
                path,
                &format!("{metric_field}.output_cost_per_million_microusd"),
            )?;
            validate_optional_u64(
                metric.latency_ms,
                path,
                &format!("{metric_field}.latency_ms"),
            )?;
            validate_optional_u64(metric.rpm_limit, path, &format!("{metric_field}.rpm_limit"))?;
            validate_optional_u64(metric.tpm_limit, path, &format!("{metric_field}.tpm_limit"))?;
        }
    }
    for (index, sink) in policy.gateway.observability.sinks.iter().enumerate() {
        if sink.trim().is_empty() {
            bail!(
                "gateway.observability.sinks[{index}] in {} cannot be empty",
                path.display()
            );
        }
    }
    if matches!(
        policy
            .gateway
            .observability
            .call_id_header
            .as_deref()
            .map(str::trim),
        Some("")
    ) {
        bail!(
            "gateway.observability.call_id_header in {} cannot be empty",
            path.display()
        );
    }
    if matches!(
        policy
            .gateway
            .observability
            .jsonl_path
            .as_deref()
            .map(str::trim),
        Some("")
    ) {
        bail!(
            "gateway.observability.jsonl_path in {} cannot be empty",
            path.display()
        );
    }
    if let Some(endpoint) = policy.gateway.observability.http_endpoint.as_deref() {
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            bail!(
                "gateway.observability.http_endpoint in {} cannot be empty",
                path.display()
            );
        }
        if !gateway_observability_http_endpoint_has_http_host(endpoint) {
            bail!(
                "gateway.observability.http_endpoint in {} must be an http(s) URL with host",
                path.display()
            );
        }
    }
    if matches!(
        policy
            .gateway
            .observability
            .http_bearer_token_env
            .as_deref()
            .map(str::trim),
        Some("")
    ) {
        bail!(
            "gateway.observability.http_bearer_token_env in {} cannot be empty",
            path.display()
        );
    }
    if let Some(schema) = policy.gateway.observability.http_schema.as_deref() {
        validate_gateway_observability_http_schema(schema).with_context(|| {
            format!(
                "gateway.observability.http_schema in {} is invalid",
                path.display()
            )
        })?;
    }
    for (index, keyword) in policy
        .gateway
        .guardrails
        .blocked_keywords
        .iter()
        .enumerate()
    {
        if keyword.trim().is_empty() {
            bail!(
                "gateway.guardrails.blocked_keywords[{index}] in {} cannot be empty",
                path.display()
            );
        }
    }
    for (index, keyword) in policy
        .gateway
        .guardrails
        .blocked_output_keywords
        .iter()
        .enumerate()
    {
        if keyword.trim().is_empty() {
            bail!(
                "gateway.guardrails.blocked_output_keywords[{index}] in {} cannot be empty",
                path.display()
            );
        }
    }
    for (index, model) in policy.gateway.guardrails.allowed_models.iter().enumerate() {
        if model.trim().is_empty() {
            bail!(
                "gateway.guardrails.allowed_models[{index}] in {} cannot be empty",
                path.display()
            );
        }
    }
    if let Some(url) = policy.gateway.guardrails.webhook_url.as_deref() {
        let url = url.trim();
        if url.is_empty() {
            bail!(
                "gateway.guardrails.webhook_url in {} cannot be empty",
                path.display()
            );
        }
        if !gateway_observability_http_endpoint_has_http_host(url) {
            bail!(
                "gateway.guardrails.webhook_url in {} must be an http(s) URL with host",
                path.display()
            );
        }
    }
    for (index, phase) in policy.gateway.guardrails.webhook_phases.iter().enumerate() {
        validate_gateway_guardrail_webhook_phase(phase).with_context(|| {
            format!(
                "gateway.guardrails.webhook_phases[{index}] in {} is invalid",
                path.display()
            )
        })?;
    }
    if matches!(
        policy
            .gateway
            .guardrails
            .webhook_bearer_token_env
            .as_deref()
            .map(str::trim),
        Some("")
    ) {
        bail!(
            "gateway.guardrails.webhook_bearer_token_env in {} cannot be empty",
            path.display()
        );
    }
    Ok(())
}

fn validate_gateway_route_strategy(value: &str) -> Result<()> {
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

fn validate_gateway_observability_http_schema(value: &str) -> Result<()> {
    match value.trim().to_ascii_lowercase().as_str() {
        "generic" | "otel" | "opentelemetry" | "datadog" | "langfuse" => Ok(()),
        "" => bail!("schema cannot be empty"),
        _ => bail!("schema must be one of generic, otel, datadog, langfuse"),
    }
}

fn validate_gateway_guardrail_webhook_phase(value: &str) -> Result<()> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pre" | "request" | "post" | "response" => Ok(()),
        "" => bail!("phase cannot be empty"),
        _ => bail!("phase must be one of pre, post"),
    }
}

fn gateway_observability_http_endpoint_has_http_host(value: &str) -> bool {
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

pub fn validate_runtime_proxy_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    validate_optional_usize(
        policy.runtime_proxy.worker_count,
        path,
        "runtime_proxy.worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.long_lived_worker_count,
        path,
        "runtime_proxy.long_lived_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.probe_refresh_worker_count,
        path,
        "runtime_proxy.probe_refresh_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.async_worker_count,
        path,
        "runtime_proxy.async_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.long_lived_queue_capacity,
        path,
        "runtime_proxy.long_lived_queue_capacity",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.active_request_limit,
        path,
        "runtime_proxy.active_request_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.profile_inflight_soft_limit,
        path,
        "runtime_proxy.profile_inflight_soft_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.profile_inflight_hard_limit,
        path,
        "runtime_proxy.profile_inflight_hard_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.responses_active_limit,
        path,
        "runtime_proxy.responses_active_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.compact_active_limit,
        path,
        "runtime_proxy.compact_active_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.websocket_active_limit,
        path,
        "runtime_proxy.websocket_active_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.standard_active_limit,
        path,
        "runtime_proxy.standard_active_limit",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.http_connect_timeout_ms,
        path,
        "runtime_proxy.http_connect_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.stream_idle_timeout_ms,
        path,
        "runtime_proxy.stream_idle_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.compact_request_timeout_ms,
        path,
        "runtime_proxy.compact_request_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.sse_lookahead_timeout_ms,
        path,
        "runtime_proxy.sse_lookahead_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.prefetch_backpressure_retry_ms,
        path,
        "runtime_proxy.prefetch_backpressure_retry_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.prefetch_backpressure_timeout_ms,
        path,
        "runtime_proxy.prefetch_backpressure_timeout_ms",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.prefetch_max_buffered_bytes,
        path,
        "runtime_proxy.prefetch_max_buffered_bytes",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.websocket_connect_timeout_ms,
        path,
        "runtime_proxy.websocket_connect_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.websocket_happy_eyeballs_delay_ms,
        path,
        "runtime_proxy.websocket_happy_eyeballs_delay_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.websocket_precommit_progress_timeout_ms,
        path,
        "runtime_proxy.websocket_precommit_progress_timeout_ms",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.websocket_connect_worker_count,
        path,
        "runtime_proxy.websocket_connect_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.websocket_connect_queue_capacity,
        path,
        "runtime_proxy.websocket_connect_queue_capacity",
    )?;
    validate_optional_usize_allow_zero(
        policy.runtime_proxy.websocket_connect_overflow_capacity,
        path,
        "runtime_proxy.websocket_connect_overflow_capacity",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.websocket_dns_worker_count,
        path,
        "runtime_proxy.websocket_dns_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.websocket_dns_queue_capacity,
        path,
        "runtime_proxy.websocket_dns_queue_capacity",
    )?;
    validate_optional_usize_allow_zero(
        policy.runtime_proxy.websocket_dns_overflow_capacity,
        path,
        "runtime_proxy.websocket_dns_overflow_capacity",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.broker_ready_timeout_ms,
        path,
        "runtime_proxy.broker_ready_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.broker_health_connect_timeout_ms,
        path,
        "runtime_proxy.broker_health_connect_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.broker_health_read_timeout_ms,
        path,
        "runtime_proxy.broker_health_read_timeout_ms",
    )?;
    validate_optional_u64(
        policy
            .runtime_proxy
            .websocket_previous_response_reuse_stale_ms,
        path,
        "runtime_proxy.websocket_previous_response_reuse_stale_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.admission_wait_budget_ms,
        path,
        "runtime_proxy.admission_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.pressure_admission_wait_budget_ms,
        path,
        "runtime_proxy.pressure_admission_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.long_lived_queue_wait_budget_ms,
        path,
        "runtime_proxy.long_lived_queue_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy
            .runtime_proxy
            .pressure_long_lived_queue_wait_budget_ms,
        path,
        "runtime_proxy.pressure_long_lived_queue_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.sync_probe_pressure_pause_ms,
        path,
        "runtime_proxy.sync_probe_pressure_pause_ms",
    )?;
    validate_optional_i64_percent(
        policy.runtime_proxy.responses_critical_floor_percent,
        path,
        "runtime_proxy.responses_critical_floor_percent",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.startup_sync_probe_warm_limit,
        path,
        "runtime_proxy.startup_sync_probe_warm_limit",
    )?;

    Ok(())
}

fn validate_optional_usize(value: Option<usize>, path: &Path, field: &str) -> Result<()> {
    if matches!(value, Some(0)) {
        bail!("{field} in {} must be greater than 0", path.display());
    }
    Ok(())
}

fn validate_optional_usize_allow_zero(
    _value: Option<usize>,
    _path: &Path,
    _field: &str,
) -> Result<()> {
    Ok(())
}

fn validate_optional_u64(value: Option<u64>, path: &Path, field: &str) -> Result<()> {
    if matches!(value, Some(0)) {
        bail!("{field} in {} must be greater than 0", path.display());
    }
    Ok(())
}

fn validate_optional_i64_percent(value: Option<i64>, path: &Path, field: &str) -> Result<()> {
    if let Some(value) = value
        && !(1..=10).contains(&value)
    {
        bail!("{field} in {} must be between 1 and 10", path.display());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/src/validate.rs"]
mod tests;
