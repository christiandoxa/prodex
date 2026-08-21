use crate::types::RuntimePolicyFile;
#[cfg(any(not(feature = "mojo"), test))]
use crate::validate_helpers::{
    validate_optional_i64_percent, validate_optional_u64, validate_optional_usize,
};
use anyhow::Result;
#[cfg(feature = "mojo")]
use anyhow::bail;
use std::path::Path;

pub fn validate_runtime_proxy_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    #[cfg(feature = "mojo")]
    {
        validate_runtime_proxy_policy_mojo(policy, path)
    }

    #[cfg(not(feature = "mojo"))]
    validate_runtime_proxy_policy_rust(policy, path)
}

#[cfg(any(not(feature = "mojo"), test))]
fn validate_runtime_proxy_policy_rust(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
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

#[cfg(feature = "mojo")]
fn push_non_zero_u64(
    rules: &mut Vec<prodex_mojo_core::policy::NumericRule>,
    names: &mut Vec<&'static str>,
    name: &'static str,
    value: Option<u64>,
) {
    if let Some(value) = value {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_NON_ZERO,
            value,
            minimum: 0,
            maximum: u64::MAX,
            related_value: 0,
        });
        names.push(name);
    }
}

#[cfg(feature = "mojo")]
fn push_range_i64(
    rules: &mut Vec<prodex_mojo_core::policy::NumericRule>,
    names: &mut Vec<&'static str>,
    name: &'static str,
    value: Option<i64>,
) {
    if let Some(value) = value {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_RANGE,
            value: value as u64,
            minimum: 1,
            maximum: 10,
            related_value: 0,
        });
        names.push(name);
    }
}

#[cfg(feature = "mojo")]
fn validate_runtime_proxy_policy_mojo(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let mut rules = Vec::new();
    let mut names = Vec::new();
    let settings = &policy.runtime_proxy;
    let add_usize = |rules: &mut Vec<prodex_mojo_core::policy::NumericRule>,
                     names: &mut Vec<&'static str>,
                     name: &'static str,
                     value: Option<usize>| {
        push_non_zero_u64(rules, names, name, value.map(|value| value as u64));
    };

    for (name, value) in [
        (
            "runtime_proxy.worker_count",
            settings.worker_count.map(|value| value as u64),
        ),
        (
            "runtime_proxy.long_lived_worker_count",
            settings.long_lived_worker_count.map(|value| value as u64),
        ),
        (
            "runtime_proxy.probe_refresh_worker_count",
            settings
                .probe_refresh_worker_count
                .map(|value| value as u64),
        ),
        (
            "runtime_proxy.async_worker_count",
            settings.async_worker_count.map(|value| value as u64),
        ),
        (
            "runtime_proxy.long_lived_queue_capacity",
            settings.long_lived_queue_capacity.map(|value| value as u64),
        ),
        (
            "runtime_proxy.active_request_limit",
            settings.active_request_limit.map(|value| value as u64),
        ),
        (
            "runtime_proxy.profile_inflight_soft_limit",
            settings
                .profile_inflight_soft_limit
                .map(|value| value as u64),
        ),
        (
            "runtime_proxy.profile_inflight_hard_limit",
            settings
                .profile_inflight_hard_limit
                .map(|value| value as u64),
        ),
        (
            "runtime_proxy.responses_active_limit",
            settings.responses_active_limit.map(|value| value as u64),
        ),
        (
            "runtime_proxy.compact_active_limit",
            settings.compact_active_limit.map(|value| value as u64),
        ),
        (
            "runtime_proxy.websocket_active_limit",
            settings.websocket_active_limit.map(|value| value as u64),
        ),
        (
            "runtime_proxy.standard_active_limit",
            settings.standard_active_limit.map(|value| value as u64),
        ),
    ] {
        push_non_zero_u64(&mut rules, &mut names, name, value);
    }

    for (name, value) in [
        (
            "runtime_proxy.http_connect_timeout_ms",
            settings.http_connect_timeout_ms,
        ),
        (
            "runtime_proxy.stream_idle_timeout_ms",
            settings.stream_idle_timeout_ms,
        ),
        (
            "runtime_proxy.compact_request_timeout_ms",
            settings.compact_request_timeout_ms,
        ),
        (
            "runtime_proxy.sse_lookahead_timeout_ms",
            settings.sse_lookahead_timeout_ms,
        ),
        (
            "runtime_proxy.prefetch_backpressure_retry_ms",
            settings.prefetch_backpressure_retry_ms,
        ),
        (
            "runtime_proxy.prefetch_backpressure_timeout_ms",
            settings.prefetch_backpressure_timeout_ms,
        ),
    ] {
        push_non_zero_u64(&mut rules, &mut names, name, value);
    }

    add_usize(
        &mut rules,
        &mut names,
        "runtime_proxy.prefetch_max_buffered_bytes",
        settings.prefetch_max_buffered_bytes,
    );

    for (name, value) in [
        (
            "runtime_proxy.websocket_connect_timeout_ms",
            settings.websocket_connect_timeout_ms,
        ),
        (
            "runtime_proxy.websocket_happy_eyeballs_delay_ms",
            settings.websocket_happy_eyeballs_delay_ms,
        ),
        (
            "runtime_proxy.websocket_precommit_progress_timeout_ms",
            settings.websocket_precommit_progress_timeout_ms,
        ),
    ] {
        push_non_zero_u64(&mut rules, &mut names, name, value);
    }

    for (name, value) in [
        (
            "runtime_proxy.websocket_connect_worker_count",
            settings
                .websocket_connect_worker_count
                .map(|value| value as u64),
        ),
        (
            "runtime_proxy.websocket_connect_queue_capacity",
            settings
                .websocket_connect_queue_capacity
                .map(|value| value as u64),
        ),
        (
            "runtime_proxy.websocket_dns_worker_count",
            settings
                .websocket_dns_worker_count
                .map(|value| value as u64),
        ),
        (
            "runtime_proxy.websocket_dns_queue_capacity",
            settings
                .websocket_dns_queue_capacity
                .map(|value| value as u64),
        ),
    ] {
        push_non_zero_u64(&mut rules, &mut names, name, value);
    }

    for (name, value) in [
        (
            "runtime_proxy.broker_ready_timeout_ms",
            settings.broker_ready_timeout_ms,
        ),
        (
            "runtime_proxy.broker_health_connect_timeout_ms",
            settings.broker_health_connect_timeout_ms,
        ),
        (
            "runtime_proxy.broker_health_read_timeout_ms",
            settings.broker_health_read_timeout_ms,
        ),
        (
            "runtime_proxy.websocket_previous_response_reuse_stale_ms",
            settings.websocket_previous_response_reuse_stale_ms,
        ),
        (
            "runtime_proxy.admission_wait_budget_ms",
            settings.admission_wait_budget_ms,
        ),
        (
            "runtime_proxy.pressure_admission_wait_budget_ms",
            settings.pressure_admission_wait_budget_ms,
        ),
        (
            "runtime_proxy.long_lived_queue_wait_budget_ms",
            settings.long_lived_queue_wait_budget_ms,
        ),
        (
            "runtime_proxy.pressure_long_lived_queue_wait_budget_ms",
            settings.pressure_long_lived_queue_wait_budget_ms,
        ),
        (
            "runtime_proxy.sync_probe_pressure_pause_ms",
            settings.sync_probe_pressure_pause_ms,
        ),
    ] {
        push_non_zero_u64(&mut rules, &mut names, name, value);
    }

    push_range_i64(
        &mut rules,
        &mut names,
        "runtime_proxy.responses_critical_floor_percent",
        settings.responses_critical_floor_percent,
    );
    add_usize(
        &mut rules,
        &mut names,
        "runtime_proxy.startup_sync_probe_warm_limit",
        settings.startup_sync_probe_warm_limit,
    );

    let failed = prodex_mojo_core::policy::validate_numeric_rules(&rules).map_err(|_| {
        anyhow::anyhow!("runtime policy numeric validation returned invalid output")
    })?;
    if let Some(index) = failed.first() {
        if rules[*index].kind == prodex_mojo_core::policy::POLICY_NUMERIC_RANGE {
            bail!(
                "{} in {} must be between 1 and 10",
                names[*index],
                path.display()
            );
        }
        bail!(
            "{} in {} must be greater than 0",
            names[*index],
            path.display()
        );
    }
    Ok(())
}
#[cfg(all(test, feature = "mojo"))]
mod tests {
    use super::*;

    #[test]
    fn mojo_runtime_proxy_numeric_validation_matches_rust_oracle() {
        for input in [
            "version = 1",
            "version = 1\n[runtime_proxy]\nworker_count = 0",
            "version = 1\n[runtime_proxy]\nresponses_critical_floor_percent = 0",
            "version = 1\n[runtime_proxy]\nresponses_critical_floor_percent = 11",
            "version = 1\n[runtime_proxy]\nhttp_connect_timeout_ms = 0",
            "version = 1\n[runtime_proxy]\nworker_count = 0\nhttp_connect_timeout_ms = 0",
        ] {
            let policy = toml::from_str::<RuntimePolicyFile>(input).unwrap();
            let path = Path::new("policy.toml");
            assert_eq!(
                validate_runtime_proxy_policy_rust(&policy, path)
                    .map_err(|error| error.to_string()),
                validate_runtime_proxy_policy_mojo(&policy, path)
                    .map_err(|error| error.to_string()),
                "{input}"
            );
        }
    }
}
