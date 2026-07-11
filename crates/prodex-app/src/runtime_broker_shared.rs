use crate::RuntimeRotationProxy;
use crate::runtime_broker::create_runtime_broker_lease_in_dir_for_pid;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[cfg(test)]
pub(super) use prodex_runtime_broker::RuntimeBrokerMetrics;
pub(super) use prodex_runtime_broker::{
    RuntimeBrokerHealth, RuntimeBrokerMetadata, RuntimeBrokerRegistry,
    RuntimeBrokerVersionGuardOutcome, RuntimeProdexBinaryIdentity, parse_prodex_version_output,
    runtime_health_prodex_binary_identity, runtime_prodex_binary_identity_key,
    runtime_prodex_binary_identity_matches, runtime_registry_prodex_binary_identity,
};

#[cfg(test)]
pub(super) fn runtime_broker_test_secret(
    value: &str,
) -> std::sync::Arc<prodex_runtime_broker::RuntimeBrokerSecret> {
    std::sync::Arc::new(prodex_runtime_broker::RuntimeBrokerSecret::new(value).unwrap())
}

#[cfg(test)]
pub(super) fn save_runtime_broker_test_capability(
    paths: &prodex_core::AppPaths,
    broker_key: &str,
    instance_id: &str,
    value: &str,
) {
    let secret = prodex_runtime_broker::RuntimeBrokerSecret::new(value).unwrap();
    crate::save_runtime_broker_capability(paths, broker_key, instance_id, &secret).unwrap();
}

#[derive(Debug)]
pub(super) struct RuntimeBrokerLease {
    pub(super) path: PathBuf,
}

pub(super) struct RuntimeProxyEndpoint {
    pub(super) listen_addr: std::net::SocketAddr,
    pub(super) openai_mount_path: String,
    pub(super) local_model_provider_id: Option<String>,
    pub(super) realtime_ws_base_url: Option<String>,
    pub(super) realtime_ws_model: Option<String>,
    pub(super) lease_dir: PathBuf,
    pub(super) _lease: Option<RuntimeBrokerLease>,
    pub(super) _direct_proxy: Option<RuntimeRotationProxy>,
}

impl std::fmt::Debug for RuntimeProxyEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeProxyEndpoint")
            .field("listen_addr", &self.listen_addr)
            .field("openai_mount_path", &self.openai_mount_path)
            .field("local_model_provider_id", &self.local_model_provider_id)
            .field("realtime_ws_base_url", &self.realtime_ws_base_url)
            .field("realtime_ws_model", &self.realtime_ws_model)
            .field("lease_dir", &self.lease_dir)
            .field("has_lease", &self._lease.is_some())
            .field("has_direct_proxy", &self._direct_proxy.is_some())
            .finish()
    }
}

impl Drop for RuntimeBrokerLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl RuntimeProxyEndpoint {
    pub(super) fn create_child_lease(&self, pid: u32) -> Result<RuntimeBrokerLease> {
        create_runtime_broker_lease_in_dir_for_pid(&self.lease_dir, pid)
    }
}
