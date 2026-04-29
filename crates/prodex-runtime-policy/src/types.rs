use secret_store::SecretBackendKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const PRODEX_POLICY_FILE_NAME: &str = "policy.toml";
pub const PRODEX_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeLogFormat {
    Text,
    Json,
}

impl RuntimeLogFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimePolicySummary {
    pub path: PathBuf,
    pub version: u32,
}

#[derive(Debug, Clone)]
pub struct RuntimePolicyConfig {
    pub path: PathBuf,
    pub version: u32,
    pub runtime: RuntimePolicyRuntimeSettings,
    pub runtime_proxy: RuntimePolicyProxySettings,
    pub secrets: RuntimePolicySecretsSettings,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimePolicyRuntimeSettings {
    pub log_format: Option<RuntimeLogFormat>,
    pub log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimePolicySecretsSettings {
    pub backend: Option<SecretBackendKind>,
    pub keyring_service: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyProxySettings {
    pub worker_count: Option<usize>,
    pub long_lived_worker_count: Option<usize>,
    pub probe_refresh_worker_count: Option<usize>,
    pub async_worker_count: Option<usize>,
    pub long_lived_queue_capacity: Option<usize>,
    pub active_request_limit: Option<usize>,
    pub profile_inflight_soft_limit: Option<usize>,
    pub profile_inflight_hard_limit: Option<usize>,
    pub responses_active_limit: Option<usize>,
    pub compact_active_limit: Option<usize>,
    pub websocket_active_limit: Option<usize>,
    pub standard_active_limit: Option<usize>,
    pub http_connect_timeout_ms: Option<u64>,
    pub stream_idle_timeout_ms: Option<u64>,
    pub sse_lookahead_timeout_ms: Option<u64>,
    pub prefetch_backpressure_retry_ms: Option<u64>,
    pub prefetch_backpressure_timeout_ms: Option<u64>,
    pub prefetch_max_buffered_bytes: Option<usize>,
    pub websocket_connect_timeout_ms: Option<u64>,
    pub websocket_happy_eyeballs_delay_ms: Option<u64>,
    pub websocket_precommit_progress_timeout_ms: Option<u64>,
    pub websocket_connect_worker_count: Option<usize>,
    pub websocket_connect_queue_capacity: Option<usize>,
    pub websocket_connect_overflow_capacity: Option<usize>,
    pub websocket_dns_worker_count: Option<usize>,
    pub websocket_dns_queue_capacity: Option<usize>,
    pub websocket_dns_overflow_capacity: Option<usize>,
    pub broker_ready_timeout_ms: Option<u64>,
    pub broker_health_connect_timeout_ms: Option<u64>,
    pub broker_health_read_timeout_ms: Option<u64>,
    pub websocket_previous_response_reuse_stale_ms: Option<u64>,
    pub admission_wait_budget_ms: Option<u64>,
    pub pressure_admission_wait_budget_ms: Option<u64>,
    pub long_lived_queue_wait_budget_ms: Option<u64>,
    pub pressure_long_lived_queue_wait_budget_ms: Option<u64>,
    pub sync_probe_pressure_pause_ms: Option<u64>,
    pub responses_critical_floor_percent: Option<i64>,
    pub startup_sync_probe_warm_limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyFile {
    pub version: u32,
    #[serde(default)]
    pub runtime: RuntimePolicyRuntimeFile,
    #[serde(default)]
    pub runtime_proxy: RuntimePolicyProxySettings,
    #[serde(default)]
    pub secrets: RuntimePolicySecretsFile,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyRuntimeFile {
    pub log_format: Option<RuntimeLogFormat>,
    pub log_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicySecretsFile {
    pub backend: Option<String>,
    pub keyring_service: Option<String>,
}
