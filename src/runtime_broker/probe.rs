use super::*;

pub(crate) fn runtime_broker_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_broker_health_connect_timeout_ms(),
        ))
        .timeout(Duration::from_millis(
            runtime_broker_health_read_timeout_ms(),
        ))
        .build()
        .context("failed to build runtime broker control client")
}

pub(crate) fn runtime_broker_health_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/health", registry.listen_addr)
}

pub(crate) fn runtime_broker_metrics_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/metrics", registry.listen_addr)
}

pub(crate) fn runtime_broker_metrics_prometheus_url(registry: &RuntimeBrokerRegistry) -> String {
    format!(
        "http://{}/__prodex/runtime/metrics/prometheus",
        registry.listen_addr
    )
}

pub(crate) fn runtime_broker_activate_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/activate", registry.listen_addr)
}

pub(crate) fn probe_runtime_broker_health(
    client: &Client,
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerHealth>> {
    let response = match client
        .get(runtime_broker_health_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .send()
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let health = response
        .json::<RuntimeBrokerHealth>()
        .context("failed to decode runtime broker health response")?;
    Ok(Some(health))
}

pub(crate) fn probe_runtime_broker_metrics(
    client: &Client,
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerMetrics>> {
    let response = match client
        .get(runtime_broker_metrics_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .send()
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let metrics = response
        .json::<RuntimeBrokerMetrics>()
        .context("failed to decode runtime broker metrics response")?;
    Ok(Some(metrics))
}

pub(crate) fn collect_live_runtime_broker_observations(
    paths: &AppPaths,
) -> Vec<RuntimeBrokerObservation> {
    let Ok(client) = runtime_broker_client() else {
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
        let Ok(Some(metrics)) = probe_runtime_broker_metrics(&client, &registry) else {
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
    match targets {
        [] => "-".to_string(),
        [target] => target.clone(),
        [first, rest @ ..] => format!("{first} (+{} more)", rest.len()),
    }
}

pub(crate) fn activate_runtime_broker_profile(
    client: &Client,
    registry: &RuntimeBrokerRegistry,
    current_profile: &str,
) -> Result<()> {
    let response = client
        .post(runtime_broker_activate_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .json(&serde_json::json!({
            "current_profile": current_profile,
        }))
        .send()
        .context("failed to send runtime broker activation request")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
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
