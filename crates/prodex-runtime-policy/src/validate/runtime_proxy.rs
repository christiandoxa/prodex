use crate::types::RuntimePolicyFile;
use crate::validate_helpers::{NumericRule, failed_numeric_rules};
use anyhow::{Result, bail};
use std::path::Path;

pub fn validate_runtime_proxy_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let mut rules = Vec::new();
    let mut names = Vec::new();
    let settings = &policy.runtime_proxy;

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
        push_non_zero(&mut rules, &mut names, name, value);
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
        push_non_zero(&mut rules, &mut names, name, value);
    }

    push_non_zero(
        &mut rules,
        &mut names,
        "runtime_proxy.prefetch_max_buffered_bytes",
        settings
            .prefetch_max_buffered_bytes
            .map(|value| value as u64),
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
        push_non_zero(&mut rules, &mut names, name, value);
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
        push_non_zero(&mut rules, &mut names, name, value);
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
        push_non_zero(&mut rules, &mut names, name, value);
    }

    if let Some(value) = settings.responses_critical_floor_percent {
        rules.push(NumericRule::Range {
            value: value as u64,
            minimum: 1,
            maximum: 10,
        });
        names.push("runtime_proxy.responses_critical_floor_percent");
    }
    push_non_zero(
        &mut rules,
        &mut names,
        "runtime_proxy.startup_sync_probe_warm_limit",
        settings
            .startup_sync_probe_warm_limit
            .map(|value| value as u64),
    );

    if let Some(index) = failed_numeric_rules(&rules)?.first().copied() {
        if matches!(rules[index], NumericRule::Range { .. }) {
            bail!(
                "{} in {} must be between 1 and 10",
                names[index],
                path.display()
            );
        }
        bail!(
            "{} in {} must be greater than 0",
            names[index],
            path.display()
        );
    }
    Ok(())
}

fn push_non_zero(
    rules: &mut Vec<NumericRule>,
    names: &mut Vec<&'static str>,
    name: &'static str,
    value: Option<u64>,
) {
    if let Some(value) = value {
        rules.push(NumericRule::NonZero(value));
        names.push(name);
    }
}
