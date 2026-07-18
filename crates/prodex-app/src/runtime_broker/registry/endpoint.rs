use anyhow::{Context, Result};

use crate::{
    AppPaths, LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX, RuntimeBrokerRegistry,
    RuntimeBrokerSessionAffinityControl, RuntimeProxyEndpoint, create_runtime_broker_lease,
    runtime_broker_lease_dir, runtime_process_prodex_version,
};

pub(crate) fn legacy_runtime_proxy_openai_mount_path(version: &str) -> String {
    prodex_runtime_broker::runtime_broker_legacy_openai_mount_path(
        LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX,
        version,
    )
}

pub(crate) fn runtime_broker_openai_mount_path(registry: &RuntimeBrokerRegistry) -> Result<String> {
    if let Some(openai_mount_path) =
        prodex_runtime_broker::runtime_broker_registry_openai_mount_path(registry)
    {
        return Ok(openai_mount_path);
    }

    let version = runtime_process_prodex_version(registry.pid).with_context(|| {
        format!(
            "failed to resolve prodex version for runtime broker pid {}",
            registry.pid
        )
    })?;
    Ok(legacy_runtime_proxy_openai_mount_path(&version))
}

pub(crate) fn runtime_proxy_endpoint_from_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    client: &reqwest::blocking::Client,
) -> Result<RuntimeProxyEndpoint> {
    let lease = create_runtime_broker_lease(paths, broker_key)?;
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    let listen_addr = registry.listen_addr.parse().with_context(|| {
        format!(
            "invalid runtime broker listen address {}",
            registry.listen_addr
        )
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr,
        openai_mount_path: runtime_broker_openai_mount_path(registry)?,
        local_model_provider_id: None,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir,
        broker_session_affinity_control: Some(RuntimeBrokerSessionAffinityControl {
            client: client.clone(),
            paths: paths.clone(),
            broker_key: broker_key.to_string(),
            registry: registry.clone(),
        }),
        _lease: Some(lease),
        _direct_proxy: None,
        _kiro_connect_proxy: None,
    })
}
