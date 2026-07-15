use super::{
    load_runtime_broker_capability, load_runtime_broker_registry, runtime_broker_registry_keys,
    runtime_process_pid_alive,
};
use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, RuntimeConfig,
    read_blocking_response_body_with_limit, read_blocking_response_text_with_limit,
};
use anyhow::{Context, Result, bail};
use prodex_core::AppPaths;
use prodex_runtime_broker::{
    RuntimeBrokerHealth, RuntimeBrokerMetrics, RuntimeBrokerObservation, RuntimeBrokerRegistry,
    RuntimeBrokerSecret,
};
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use std::time::Duration;

fn runtime_broker_admin_header(capability: &RuntimeBrokerSecret) -> Result<HeaderValue> {
    let mut value = HeaderValue::from_str(capability.expose())
        .context("runtime broker capability is not a valid HTTP header value")?;
    value.set_sensitive(true);
    Ok(value)
}

pub(crate) fn runtime_broker_client_with_config(config: &RuntimeConfig) -> Result<Client> {
    Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_millis(
            config.broker_health_connect_timeout_ms,
        ))
        .timeout(Duration::from_millis(config.broker_health_read_timeout_ms))
        .build()
        .context("failed to build runtime broker control client")
}

#[cfg(test)]
pub(crate) fn runtime_broker_client() -> Result<Client> {
    runtime_broker_client_with_config(&RuntimeConfig::compatibility_current())
}

pub(crate) fn runtime_broker_health_url(registry: &RuntimeBrokerRegistry) -> String {
    prodex_runtime_broker::runtime_broker_health_url(registry)
}

pub(crate) fn runtime_broker_metrics_url(registry: &RuntimeBrokerRegistry) -> String {
    prodex_runtime_broker::runtime_broker_metrics_url(registry)
}

pub(crate) fn runtime_broker_metrics_prometheus_url(registry: &RuntimeBrokerRegistry) -> String {
    prodex_runtime_broker::runtime_broker_metrics_prometheus_url(registry)
}

pub(crate) fn runtime_broker_activate_url(registry: &RuntimeBrokerRegistry) -> String {
    prodex_runtime_broker::runtime_broker_activate_url(registry)
}

pub(crate) fn runtime_broker_release_session_affinity_url(
    registry: &RuntimeBrokerRegistry,
) -> String {
    prodex_runtime_broker::runtime_broker_release_session_affinity_url(registry)
}

pub(crate) fn probe_runtime_broker_health(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerHealth>> {
    let Ok(capability) = load_runtime_broker_capability(paths, broker_key, &registry.instance_id)
    else {
        return Ok(None);
    };
    let response = match client
        .get(runtime_broker_health_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            runtime_broker_admin_header(&capability)?,
        )
        .send()
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read runtime broker health response",
    )?;
    let health = serde_json::from_slice::<RuntimeBrokerHealth>(&body)
        .context("failed to decode runtime broker health response")?;
    Ok(Some(health))
}

pub(crate) fn probe_runtime_broker_metrics(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerMetrics>> {
    let Ok(capability) = load_runtime_broker_capability(paths, broker_key, &registry.instance_id)
    else {
        return Ok(None);
    };
    let response = match client
        .get(runtime_broker_metrics_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            runtime_broker_admin_header(&capability)?,
        )
        .send()
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read runtime broker metrics response",
    )?;
    let metrics = serde_json::from_slice::<RuntimeBrokerMetrics>(&body)
        .context("failed to decode runtime broker metrics response")?;
    Ok(Some(metrics))
}

pub(crate) fn collect_live_runtime_broker_observations(
    paths: &AppPaths,
) -> Vec<RuntimeBrokerObservation> {
    let Ok(config) = RuntimeConfig::from_env_policy_and_cli(paths) else {
        return Vec::new();
    };
    let Ok(client) = runtime_broker_client_with_config(&config) else {
        return Vec::new();
    };

    let mut observations = Vec::new();
    for broker_key in runtime_broker_registry_keys(paths) {
        let Ok(Some(registry)) = load_runtime_broker_registry(paths, &broker_key) else {
            continue;
        };
        if !runtime_process_pid_alive(registry.pid) {
            continue;
        }
        let Ok(Some(metrics)) =
            probe_runtime_broker_metrics(&client, paths, &broker_key, &registry)
        else {
            continue;
        };
        observations.push(RuntimeBrokerObservation {
            broker_key,
            listen_addr: registry.listen_addr,
            metrics,
        });
    }
    observations
}

pub(crate) fn collect_runtime_broker_metrics_targets(paths: &AppPaths) -> Vec<String> {
    let mut targets = Vec::new();
    for broker_key in runtime_broker_registry_keys(paths) {
        let Ok(Some(registry)) = load_runtime_broker_registry(paths, &broker_key) else {
            continue;
        };
        if !runtime_process_pid_alive(registry.pid) {
            continue;
        }
        targets.push(runtime_broker_metrics_prometheus_url(&registry));
    }
    targets
}

pub(crate) fn format_runtime_broker_metrics_targets(targets: &[String]) -> String {
    prodex_runtime_broker::format_runtime_broker_metrics_targets(targets)
}

pub(crate) fn activate_runtime_broker_profile(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    current_profile: &str,
) -> Result<()> {
    let capability = load_runtime_broker_capability(paths, broker_key, &registry.instance_id)?;
    let response = client
        .post(runtime_broker_activate_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            runtime_broker_admin_header(&capability)?,
        )
        .json(&serde_json::json!({
            "current_profile": current_profile,
        }))
        .send()
        .context("failed to send runtime broker activation request")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = read_blocking_response_text_with_limit(
            response,
            RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
            "failed to read runtime broker activation response",
        )
        .unwrap_or_default();
        bail!(
            "runtime broker activation failed with HTTP {}{}",
            status,
            if body.is_empty() {
                String::new()
            } else {
                format!(": {body}")
            }
        );
    }
    Ok(())
}

pub(crate) fn release_runtime_broker_session_affinity(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    session_id: &str,
) -> Result<()> {
    let capability = load_runtime_broker_capability(paths, broker_key, &registry.instance_id)?;
    let response = client
        .post(runtime_broker_release_session_affinity_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            runtime_broker_admin_header(&capability)?,
        )
        .json(&serde_json::json!({
            "session_id": session_id,
        }))
        .send()
        .context("failed to send runtime broker session affinity release request")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = read_blocking_response_text_with_limit(
            response,
            RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
            "failed to read runtime broker session affinity release response",
        )
        .unwrap_or_default();
        bail!(
            "runtime broker session affinity release failed with HTTP {}{}",
            status,
            if body.is_empty() {
                String::new()
            } else {
                format!(": {body}")
            }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_broker_admin_header_debug_is_redacted() {
        let capability = RuntimeBrokerSecret::new("debug-secret-capability").unwrap();
        let header = runtime_broker_admin_header(&capability).unwrap();

        assert!(header.is_sensitive());
        assert!(!format!("{header:?}").contains(capability.expose()));
    }
}
