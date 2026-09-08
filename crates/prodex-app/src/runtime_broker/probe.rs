use super::{
    load_runtime_broker_capability, load_runtime_broker_registry,
    runtime_broker_registry_identity_is_valid, runtime_broker_registry_keys,
};
use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, RuntimeConfig,
    read_blocking_response_body_with_limit, read_blocking_response_text_with_limit,
};
use anyhow::{Context, Result, bail};
use prodex_core::AppPaths;
use prodex_runtime_broker::{
    RuntimeBrokerHealth, RuntimeBrokerMetrics, RuntimeBrokerObservation, RuntimeBrokerRegistry,
};
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct RuntimeBrokerLogSnapshotEntry {
    sequence: u64,
    line: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeBrokerLogSnapshotResponse {
    cursor: u64,
    dropped: u64,
    entries: Vec<RuntimeBrokerLogSnapshotEntry>,
}

pub(crate) fn runtime_broker_registry_admission(registry: &RuntimeBrokerRegistry) -> Result<()> {
    anyhow::ensure!(
        prodex_runtime_broker::runtime_broker_listen_addr_is_loopback(&registry.listen_addr),
        "runtime broker listen address must be loopback"
    );
    anyhow::ensure!(
        runtime_broker_registry_identity_is_valid(registry),
        "runtime broker process identity is not established"
    );
    Ok(())
}

pub(crate) fn runtime_broker_admin_header(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<HeaderValue> {
    runtime_broker_registry_admission(registry)?;
    let capability = load_runtime_broker_capability(paths, broker_key, &registry.instance_id)?;
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

#[cfg(all(test, unix))]
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

pub(crate) fn runtime_broker_log_snapshot_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.log_snapshot_url()
}

pub(crate) fn probe_runtime_broker_health(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerHealth>> {
    let Ok(admin_header) = runtime_broker_admin_header(paths, broker_key, registry) else {
        return Ok(None);
    };
    let response = match client
        .get(runtime_broker_health_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            admin_header,
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
    let Ok(admin_header) = runtime_broker_admin_header(paths, broker_key, registry) else {
        return Ok(None);
    };
    let response = match client
        .get(runtime_broker_metrics_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            admin_header,
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

pub(crate) fn probe_runtime_broker_log_snapshot(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    after: u64,
) -> Result<Option<runtime_log::RuntimeLiveLogSnapshot>> {
    let Ok(admin_header) = runtime_broker_admin_header(paths, broker_key, registry) else {
        return Ok(None);
    };
    let url = format!(
        "{}?after={after}&limit={}",
        runtime_broker_log_snapshot_url(registry),
        runtime_log::DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES
    );
    let response = match client
        .get(url)
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            admin_header,
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
        "failed to read runtime broker log snapshot response",
    )?;
    let snapshot = serde_json::from_slice::<RuntimeBrokerLogSnapshotResponse>(&body)
        .context("failed to decode runtime broker log snapshot response")?;
    Ok(Some(runtime_log::RuntimeLiveLogSnapshot {
        cursor: snapshot.cursor,
        dropped: snapshot.dropped,
        entries: snapshot
            .entries
            .into_iter()
            .map(|entry| runtime_log::RuntimeLiveLogEntry {
                sequence: entry.sequence,
                line: entry.line,
            })
            .collect(),
    }))
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
        if runtime_broker_registry_admission(&registry).is_err() {
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
        if runtime_broker_registry_admission(&registry).is_err() {
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
    let admin_header = runtime_broker_admin_header(paths, broker_key, registry)?;
    let response = client
        .post(runtime_broker_activate_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            admin_header,
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
    let admin_header = runtime_broker_admin_header(paths, broker_key, registry)?;
    let response = client
        .post(runtime_broker_release_session_affinity_url(registry))
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            admin_header,
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

pub(crate) fn send_runtime_broker_log_event(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    message: &str,
) -> Result<()> {
    let admin_header = runtime_broker_admin_header(paths, broker_key, registry)?;
    let response = client
        .post(registry.log_event_url())
        .header(
            prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER,
            admin_header,
        )
        .json(&serde_json::json!({"message": message}))
        .send()
        .context("failed to send runtime broker log event")?;
    anyhow::ensure!(
        response.status().is_success(),
        "runtime broker log event failed with HTTP {}",
        response.status()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{runtime_current_prodex_binary_identity, runtime_process_birth_identity};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-broker-probe-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
            root,
        }
    }

    fn current_process_registry(listen_addr: &str) -> RuntimeBrokerRegistry {
        let pid = std::process::id();
        let identity = runtime_current_prodex_binary_identity();
        RuntimeBrokerRegistry {
            pid,
            process_birth_identity: runtime_process_birth_identity(pid),
            listen_addr: listen_addr.to_string(),
            started_at: 100,
            upstream_base_url: "https://upstream.example".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
            instance_id: "instance".to_string(),
            prodex_version: identity.prodex_version,
            executable_path: identity
                .executable_path
                .map(|path| path.display().to_string()),
            executable_sha256: identity.executable_sha256,
            openai_mount_path: None,
            realtime_ws_addr: None,
        }
    }

    #[test]
    fn runtime_broker_admin_header_debug_is_redacted() {
        let paths = test_paths("redacted");
        let registry = current_process_registry("127.0.0.1:4567");
        let capability =
            prodex_runtime_broker::RuntimeBrokerSecret::new("debug-secret-capability").unwrap();
        crate::save_runtime_broker_capability(&paths, "broker", "instance", &capability).unwrap();
        let header = runtime_broker_admin_header(&paths, "broker", &registry).unwrap();

        assert!(header.is_sensitive());
        assert!(!format!("{header:?}").contains(capability.expose()));
        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn runtime_broker_admin_header_rejects_non_loopback_address() {
        let paths = test_paths("non-loopback");
        let registry = current_process_registry("192.0.2.10:4567");
        let error = runtime_broker_admin_header(&paths, "broker", &registry).unwrap_err();

        assert!(error.to_string().contains("must be loopback"));
        let _ = fs::remove_dir_all(paths.root);
    }
}
