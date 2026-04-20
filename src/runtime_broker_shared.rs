use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct RuntimeBrokerRegistry {
    pub(super) pid: u32,
    pub(super) listen_addr: String,
    pub(super) started_at: i64,
    pub(super) upstream_base_url: String,
    pub(super) include_code_review: bool,
    pub(super) current_profile: String,
    pub(super) instance_token: String,
    pub(super) admin_token: String,
    #[serde(default)]
    pub(super) prodex_version: Option<String>,
    #[serde(default)]
    pub(super) executable_path: Option<String>,
    #[serde(default)]
    pub(super) executable_sha256: Option<String>,
    #[serde(default)]
    pub(super) openai_mount_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct RuntimeBrokerHealth {
    pub(super) pid: u32,
    pub(super) started_at: i64,
    pub(super) current_profile: String,
    pub(super) include_code_review: bool,
    pub(super) active_requests: usize,
    pub(super) instance_token: String,
    pub(super) persistence_role: String,
    #[serde(default)]
    pub(super) prodex_version: Option<String>,
    #[serde(default)]
    pub(super) executable_path: Option<String>,
    #[serde(default)]
    pub(super) executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct RuntimeBrokerLaneMetrics {
    pub(super) active: usize,
    pub(super) limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct RuntimeBrokerTrafficMetrics {
    pub(super) responses: RuntimeBrokerLaneMetrics,
    pub(super) compact: RuntimeBrokerLaneMetrics,
    pub(super) websocket: RuntimeBrokerLaneMetrics,
    pub(super) standard: RuntimeBrokerLaneMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct RuntimeBrokerContinuationMetrics {
    pub(super) response_bindings: usize,
    pub(super) turn_state_bindings: usize,
    pub(super) session_id_bindings: usize,
    pub(super) warm: usize,
    pub(super) verified: usize,
    pub(super) suspect: usize,
    pub(super) dead: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct RuntimeBrokerMetrics {
    pub(super) health: RuntimeBrokerHealth,
    pub(super) active_request_limit: usize,
    pub(super) local_overload_backoff_remaining_seconds: u64,
    pub(super) traffic: RuntimeBrokerTrafficMetrics,
    pub(super) profile_inflight: BTreeMap<String, usize>,
    pub(super) retry_backoffs: usize,
    pub(super) transport_backoffs: usize,
    pub(super) route_circuits: usize,
    pub(super) degraded_profiles: usize,
    pub(super) degraded_routes: usize,
    pub(super) continuations: RuntimeBrokerContinuationMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimeBrokerObservation {
    pub(super) broker_key: String,
    pub(super) listen_addr: String,
    pub(super) metrics: RuntimeBrokerMetrics,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(super) struct RuntimeBrokerMetadata {
    pub(super) broker_key: String,
    pub(super) listen_addr: String,
    pub(super) started_at: i64,
    pub(super) current_profile: String,
    pub(super) include_code_review: bool,
    pub(super) instance_token: String,
    pub(super) admin_token: String,
    pub(super) prodex_version: Option<String>,
    pub(super) executable_path: Option<String>,
    pub(super) executable_sha256: Option<String>,
}

#[derive(Debug)]
pub(super) struct RuntimeBrokerLease {
    pub(super) path: PathBuf,
}

#[derive(Debug)]
pub(super) struct RuntimeProxyEndpoint {
    pub(super) listen_addr: std::net::SocketAddr,
    pub(super) openai_mount_path: String,
    pub(super) lease_dir: PathBuf,
    pub(super) _lease: Option<RuntimeBrokerLease>,
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
