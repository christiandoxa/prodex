use anyhow::{Context, Result, bail};
use std::path::Path;

use super::{
    SecretBackendKind,
    types::{PRODEX_POLICY_VERSION, RuntimePolicyFile},
};

pub(crate) fn parse_secret_backend_kind(value: &str) -> Result<SecretBackendKind> {
    value
        .parse::<SecretBackendKind>()
        .map_err(anyhow::Error::new)
}

pub(crate) fn validate_runtime_policy_file(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
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

    Ok(())
}

fn validate_runtime_proxy_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
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
mod tests {
    use super::*;

    fn parse_policy(input: &str) -> RuntimePolicyFile {
        toml::from_str(input).expect("policy TOML should parse")
    }

    #[test]
    fn validate_runtime_policy_allows_zero_websocket_executor_overflow_capacities() {
        let policy = parse_policy(
            r#"
version = 1

[runtime_proxy]
websocket_connect_worker_count = 4
websocket_connect_queue_capacity = 16
websocket_connect_overflow_capacity = 0
websocket_dns_worker_count = 2
websocket_dns_queue_capacity = 8
websocket_dns_overflow_capacity = 0
"#,
        );

        validate_runtime_policy_file(&policy, Path::new("policy.toml"))
            .expect("zero websocket overflow capacities should be valid");
    }

    #[test]
    fn validate_runtime_policy_rejects_zero_websocket_executor_non_overflow_values() {
        let policy = parse_policy(
            r#"
version = 1

[runtime_proxy]
websocket_connect_worker_count = 0
websocket_connect_overflow_capacity = 0
websocket_dns_overflow_capacity = 0
"#,
        );

        let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
            .expect_err("zero websocket executor worker count should be rejected");
        assert!(
            err.to_string()
                .contains("runtime_proxy.websocket_connect_worker_count")
        );
    }
}
