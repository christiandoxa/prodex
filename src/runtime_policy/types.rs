use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::SecretBackendKind;

pub(crate) const PRODEX_POLICY_FILE_NAME: &str = "policy.toml";
pub(crate) const PRODEX_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RuntimeLogFormat {
    Text,
    Json,
}

impl RuntimeLogFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimePolicySummary {
    pub(crate) path: PathBuf,
    pub(crate) version: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePolicyConfig {
    pub(crate) path: PathBuf,
    pub(crate) version: u32,
    pub(crate) runtime: RuntimePolicyRuntimeSettings,
    pub(crate) runtime_proxy: RuntimePolicyProxySettings,
    pub(crate) secrets: RuntimePolicySecretsSettings,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimePolicyRuntimeSettings {
    pub(crate) log_format: Option<RuntimeLogFormat>,
    pub(crate) log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimePolicySecretsSettings {
    pub(crate) backend: Option<SecretBackendKind>,
    pub(crate) keyring_service: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimePolicyProxySettings {
    pub(crate) worker_count: Option<usize>,
    pub(crate) long_lived_worker_count: Option<usize>,
    pub(crate) probe_refresh_worker_count: Option<usize>,
    pub(crate) async_worker_count: Option<usize>,
    pub(crate) long_lived_queue_capacity: Option<usize>,
    pub(crate) active_request_limit: Option<usize>,
    pub(crate) profile_inflight_soft_limit: Option<usize>,
    pub(crate) profile_inflight_hard_limit: Option<usize>,
    pub(crate) responses_active_limit: Option<usize>,
    pub(crate) compact_active_limit: Option<usize>,
    pub(crate) websocket_active_limit: Option<usize>,
    pub(crate) standard_active_limit: Option<usize>,
    pub(crate) http_connect_timeout_ms: Option<u64>,
    pub(crate) stream_idle_timeout_ms: Option<u64>,
    pub(crate) sse_lookahead_timeout_ms: Option<u64>,
    pub(crate) prefetch_backpressure_retry_ms: Option<u64>,
    pub(crate) prefetch_backpressure_timeout_ms: Option<u64>,
    pub(crate) prefetch_max_buffered_bytes: Option<usize>,
    pub(crate) websocket_connect_timeout_ms: Option<u64>,
    pub(crate) websocket_happy_eyeballs_delay_ms: Option<u64>,
    pub(crate) websocket_precommit_progress_timeout_ms: Option<u64>,
    pub(crate) websocket_connect_worker_count: Option<usize>,
    pub(crate) websocket_connect_queue_capacity: Option<usize>,
    pub(crate) websocket_connect_overflow_capacity: Option<usize>,
    pub(crate) websocket_dns_worker_count: Option<usize>,
    pub(crate) websocket_dns_queue_capacity: Option<usize>,
    pub(crate) websocket_dns_overflow_capacity: Option<usize>,
    pub(crate) broker_ready_timeout_ms: Option<u64>,
    pub(crate) broker_health_connect_timeout_ms: Option<u64>,
    pub(crate) broker_health_read_timeout_ms: Option<u64>,
    pub(crate) websocket_previous_response_reuse_stale_ms: Option<u64>,
    pub(crate) admission_wait_budget_ms: Option<u64>,
    pub(crate) pressure_admission_wait_budget_ms: Option<u64>,
    pub(crate) long_lived_queue_wait_budget_ms: Option<u64>,
    pub(crate) pressure_long_lived_queue_wait_budget_ms: Option<u64>,
    pub(crate) sync_probe_pressure_pause_ms: Option<u64>,
    pub(crate) responses_critical_floor_percent: Option<i64>,
    pub(crate) startup_sync_probe_warm_limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimePolicyFile {
    pub(crate) version: u32,
    #[serde(default)]
    pub(crate) runtime: RuntimePolicyRuntimeFile,
    #[serde(default)]
    pub(crate) runtime_proxy: RuntimePolicyProxySettings,
    #[serde(default)]
    pub(crate) secrets: RuntimePolicySecretsFile,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimePolicyRuntimeFile {
    pub(crate) log_format: Option<RuntimeLogFormat>,
    pub(crate) log_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimePolicySecretsFile {
    pub(crate) backend: Option<String>,
    pub(crate) keyring_service: Option<String>,
}
