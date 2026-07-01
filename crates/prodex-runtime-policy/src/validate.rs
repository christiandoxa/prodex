use anyhow::{Context, Result, bail};
use secret_store::SecretBackendKind;
use std::path::Path;

use crate::types::{PRODEX_POLICY_VERSION, RuntimePolicyFile};
use crate::validate_helpers::{
    gateway_observability_http_endpoint_has_http_host, validate_gateway_admin_role,
    validate_gateway_guardrail_webhook_phase, validate_gateway_observability_http_schema,
    validate_gateway_route_strategy, validate_gateway_state_backend, validate_optional_i64_percent,
    validate_optional_u64, validate_optional_usize, validate_optional_usize_allow_zero,
};

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
            .is_none_or(|value| value.is_empty())
    {
        bail!(
            "secrets.keyring_service in {} is required when secrets.backend=keyring",
            path.display()
        );
    }
    if let Some(service) = policy.secrets.keyring_service.as_deref()
        && (service.is_empty() || service.chars().any(char::is_whitespace))
    {
        bail!(
            "secrets.keyring_service in {} must be non-empty without whitespace",
            path.display()
        );
    }

    validate_runtime_proxy_policy(policy, path)?;
    validate_gateway_policy(policy, path)?;

    Ok(())
}

pub fn validate_gateway_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if let Some(listen_addr) = policy.gateway.listen_addr.as_deref() {
        validate_gateway_exact_identifier(listen_addr, path, "gateway.listen_addr")?;
    }
    if let Some(provider) = policy.gateway.provider.as_deref() {
        validate_gateway_exact_identifier(provider, path, "gateway.provider")?;
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
    validate_optional_usize(
        policy.gateway.adaptive_routing.window_size,
        path,
        "gateway.adaptive_routing.window_size",
    )?;
    validate_optional_u64(
        policy.gateway.adaptive_routing.min_samples,
        path,
        "gateway.adaptive_routing.min_samples",
    )?;
    if let Some(rate) = policy.gateway.adaptive_routing.exploration_rate
        && !(0.0..=1.0).contains(&rate)
    {
        bail!(
            "gateway.adaptive_routing.exploration_rate in {} must be between 0.0 and 1.0",
            path.display()
        );
    }
    if let Some(backend) = policy.gateway.state.backend.as_deref() {
        validate_gateway_state_backend(backend)
            .with_context(|| format!("gateway.state.backend in {} is invalid", path.display()))?;
    }
    if matches!(
        policy.gateway.state.sqlite_path.as_deref().map(str::trim),
        Some("")
    ) {
        bail!(
            "gateway.state.sqlite_path in {} cannot be empty",
            path.display()
        );
    }
    if let Some(value) = policy.gateway.state.postgres_url_env.as_deref() {
        validate_gateway_exact_identifier(value, path, "gateway.state.postgres_url_env")?;
    }
    if let Some(value) = policy.gateway.state.redis_url_env.as_deref() {
        validate_gateway_exact_identifier(value, path, "gateway.state.redis_url_env")?;
    }
    match policy
        .gateway
        .state
        .backend
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("postgres") if policy.gateway.state.postgres_url_env.is_none() => {
            bail!(
                "gateway.state.postgres_url_env in {} is required when gateway.state.backend=postgres",
                path.display()
            );
        }
        Some("redis") if policy.gateway.state.redis_url_env.is_none() => {
            bail!(
                "gateway.state.redis_url_env in {} is required when gateway.state.backend=redis",
                path.display()
            );
        }
        _ => {}
    }
    for (index, token) in policy.gateway.admin_tokens.iter().enumerate() {
        let field = format!("gateway.admin_tokens[{index}]");
        validate_gateway_exact_identifier(&token.name, path, &format!("{field}.name"))?;
        validate_gateway_exact_identifier(&token.token_env, path, &format!("{field}.token_env"))?;
        if let Some(role) = token.role.as_deref() {
            validate_gateway_admin_role(role)
                .with_context(|| format!("{field}.role in {} is invalid", path.display()))?;
        }
        validate_gateway_optional_scope(token.tenant_id.as_deref(), path, &field, "tenant_id")?;
        for (name, value) in [
            ("team_id", token.team_id.as_deref()),
            ("project_id", token.project_id.as_deref()),
            ("user_id", token.user_id.as_deref()),
            ("budget_id", token.budget_id.as_deref()),
        ] {
            validate_gateway_optional_scope(value, path, &field, name)?;
        }
        for (prefix_index, prefix) in token.allowed_key_prefixes.iter().enumerate() {
            if prefix.is_empty() || prefix.chars().any(char::is_whitespace) {
                bail!(
                    "{field}.allowed_key_prefixes[{prefix_index}] in {} must be non-empty without whitespace",
                    path.display()
                );
            }
        }
    }
    validate_gateway_sso_policy(policy, path)?;
    for (index, alias) in policy.gateway.route_aliases.iter().enumerate() {
        let field = format!("gateway.route_aliases[{index}]");
        validate_gateway_exact_identifier(&alias.alias, path, &format!("{field}.alias"))?;
        if alias.models.is_empty() {
            bail!("{field}.models in {} cannot be empty", path.display());
        }
        for (model_index, model) in alias.models.iter().enumerate() {
            validate_gateway_exact_identifier(
                model,
                path,
                &format!("{field}.models[{model_index}]"),
            )?;
        }
        if let Some(strategy) = alias.strategy.as_deref() {
            validate_gateway_route_strategy(strategy)
                .with_context(|| format!("{field}.strategy in {} is invalid", path.display()))?;
        }
        for (metric_index, metric) in alias.model_metrics.iter().enumerate() {
            let metric_field = format!("{field}.model_metrics[{metric_index}]");
            validate_gateway_exact_identifier(
                &metric.model,
                path,
                &format!("{metric_field}.model"),
            )?;
            if !alias.models.iter().any(|model| model == &metric.model) {
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
    for (index, key) in policy.gateway.virtual_keys.iter().enumerate() {
        let field = format!("gateway.virtual_keys[{index}]");
        validate_gateway_exact_identifier(&key.name, path, &format!("{field}.name"))?;
        validate_gateway_exact_identifier(&key.token_env, path, &format!("{field}.token_env"))?;
        validate_gateway_optional_scope(key.tenant_id.as_deref(), path, &field, "tenant_id")?;
        for (name, value) in [
            ("team_id", key.team_id.as_deref()),
            ("project_id", key.project_id.as_deref()),
            ("user_id", key.user_id.as_deref()),
            ("budget_id", key.budget_id.as_deref()),
        ] {
            validate_gateway_optional_scope(value, path, &field, name)?;
        }
        for (model_index, model) in key.allowed_models.iter().enumerate() {
            validate_gateway_exact_identifier(
                model,
                path,
                &format!("{field}.allowed_models[{model_index}]"),
            )?;
        }
        if let Some(budget_usd) = key.budget_usd
            && (!budget_usd.is_finite() || budget_usd <= 0.0)
        {
            bail!(
                "{field}.budget_usd in {} must be greater than 0",
                path.display()
            );
        }
        validate_optional_u64(key.request_budget, path, &format!("{field}.request_budget"))?;
        validate_optional_u64(key.rpm_limit, path, &format!("{field}.rpm_limit"))?;
        validate_optional_u64(key.tpm_limit, path, &format!("{field}.tpm_limit"))?;
    }
    for (index, sink) in policy.gateway.observability.sinks.iter().enumerate() {
        if sink.is_empty() || sink.chars().any(char::is_whitespace) {
            bail!(
                "gateway.observability.sinks[{index}] in {} must be non-empty without whitespace",
                path.display()
            );
        }
    }
    if let Some(value) = policy.gateway.observability.call_id_header.as_deref() {
        validate_gateway_exact_identifier(value, path, "gateway.observability.call_id_header")?;
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
        if endpoint.is_empty() {
            bail!(
                "gateway.observability.http_endpoint in {} cannot be empty",
                path.display()
            );
        }
        if endpoint.chars().any(char::is_whitespace) {
            bail!(
                "gateway.observability.http_endpoint in {} must not contain whitespace",
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
    if let Some(value) = policy
        .gateway
        .observability
        .http_bearer_token_env
        .as_deref()
    {
        validate_gateway_exact_identifier(
            value,
            path,
            "gateway.observability.http_bearer_token_env",
        )?;
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
        validate_gateway_exact_identifier(
            model,
            path,
            &format!("gateway.guardrails.allowed_models[{index}]"),
        )?;
    }
    if let Some(url) = policy.gateway.guardrails.webhook_url.as_deref() {
        if url.is_empty() {
            bail!(
                "gateway.guardrails.webhook_url in {} cannot be empty",
                path.display()
            );
        }
        if url.chars().any(char::is_whitespace) {
            bail!(
                "gateway.guardrails.webhook_url in {} must not contain whitespace",
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
    if let Some(value) = policy
        .gateway
        .guardrails
        .webhook_bearer_token_env
        .as_deref()
    {
        validate_gateway_exact_identifier(
            value,
            path,
            "gateway.guardrails.webhook_bearer_token_env",
        )?;
    }
    Ok(())
}

fn validate_gateway_optional_scope(
    value: Option<&str>,
    path: &Path,
    field: &str,
    name: &str,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        bail!(
            "{field}.{name} in {} must be non-empty without whitespace",
            path.display()
        );
    }
    Ok(())
}

fn validate_gateway_exact_identifier(value: &str, path: &Path, field: &str) -> Result<()> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        bail!(
            "{field} in {} must be non-empty without whitespace",
            path.display()
        );
    }
    Ok(())
}

fn validate_gateway_sso_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let sso = &policy.gateway.sso;
    if let Some(value) = sso.proxy_token_env.as_deref() {
        validate_gateway_exact_identifier(value, path, "gateway.sso.proxy_token_env")?;
    }
    for (field, value) in [
        ("gateway.sso.token_header", sso.token_header.as_deref()),
        ("gateway.sso.user_header", sso.user_header.as_deref()),
        ("gateway.sso.role_header", sso.role_header.as_deref()),
        ("gateway.sso.tenant_header", sso.tenant_header.as_deref()),
        (
            "gateway.sso.key_prefixes_header",
            sso.key_prefixes_header.as_deref(),
        ),
        ("gateway.sso.oidc_audience", sso.oidc_audience.as_deref()),
        (
            "gateway.sso.oidc_user_claim",
            sso.oidc_user_claim.as_deref(),
        ),
        (
            "gateway.sso.oidc_role_claim",
            sso.oidc_role_claim.as_deref(),
        ),
        (
            "gateway.sso.oidc_tenant_claim",
            sso.oidc_tenant_claim.as_deref(),
        ),
        (
            "gateway.sso.oidc_key_prefixes_claim",
            sso.oidc_key_prefixes_claim.as_deref(),
        ),
    ] {
        if let Some(value) = value {
            validate_gateway_exact_identifier(value, path, field)?;
        }
    }
    for (field, value) in [
        ("gateway.sso.oidc_issuer", sso.oidc_issuer.as_deref()),
        ("gateway.sso.oidc_jwks_url", sso.oidc_jwks_url.as_deref()),
    ] {
        if matches!(value.map(str::trim), Some("")) {
            bail!("{field} in {} cannot be empty", path.display());
        }
    }
    let oidc_enabled =
        sso.oidc_issuer.is_some() || sso.oidc_audience.is_some() || sso.oidc_jwks_url.is_some();
    if oidc_enabled {
        if sso.oidc_issuer.is_none() || sso.oidc_audience.is_none() {
            bail!(
                "gateway.sso OIDC in {} requires oidc_issuer and oidc_audience",
                path.display()
            );
        }
        if let Some(issuer) = sso.oidc_issuer.as_deref() {
            if issuer.chars().any(char::is_whitespace) {
                bail!(
                    "gateway.sso.oidc_issuer in {} must not contain whitespace",
                    path.display()
                );
            }
            if !issuer.starts_with("https://") {
                bail!(
                    "gateway.sso.oidc_issuer in {} must be an https URL",
                    path.display()
                );
            }
        }
        if let Some(jwks_url) = sso.oidc_jwks_url.as_deref() {
            if jwks_url.chars().any(char::is_whitespace) {
                bail!(
                    "gateway.sso.oidc_jwks_url in {} must not contain whitespace",
                    path.display()
                );
            }
            if !jwks_url.starts_with("https://") {
                bail!(
                    "gateway.sso.oidc_jwks_url in {} must be an https URL",
                    path.display()
                );
            }
        }
    }
    if let Some(role) = sso.default_role.as_deref() {
        validate_gateway_admin_role(role).with_context(|| {
            format!("gateway.sso.default_role in {} is invalid", path.display())
        })?;
    }
    Ok(())
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

#[cfg(test)]
#[path = "../tests/src/validate.rs"]
mod tests;
