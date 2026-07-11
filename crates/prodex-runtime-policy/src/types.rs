use prodex_domain::SecretRef;
use secret_store::SecretBackendKind;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::path::PathBuf;

pub const PRODEX_POLICY_FILE_NAME: &str = "policy.toml";
pub const PRODEX_POLICY_VERSION: u32 = 1;
pub const PRODEX_RUNTIME_PROXY_PRESET_ENV: &str = "PRODEX_RUNTIME_PROXY_PRESET";

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
    pub gateway: RuntimePolicyGatewaySettings,
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
    pub production: bool,
    pub projected_root: Option<PathBuf>,
    pub projected_provider: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewaySettings {
    pub listen_addr: Option<String>,
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub require_auth: Option<bool>,
    pub auth_token_ref: Option<SecretRef>,
    pub provider_api_key_ref: Option<SecretRef>,
    #[serde(default)]
    pub adaptive_routing: RuntimePolicyAdaptiveRoutingSettings,
    #[serde(default)]
    pub state: RuntimePolicyGatewayStateSettings,
    #[serde(default)]
    pub admin_tokens: Vec<RuntimePolicyGatewayAdminToken>,
    #[serde(default)]
    pub sso: RuntimePolicyGatewaySsoSettings,
    #[serde(default)]
    pub route_aliases: Vec<RuntimePolicyGatewayRouteAlias>,
    #[serde(default)]
    pub virtual_keys: Vec<RuntimePolicyGatewayVirtualKey>,
    #[serde(default)]
    pub observability: RuntimePolicyGatewayObservabilitySettings,
    #[serde(default)]
    pub guardrails: RuntimePolicyGatewayGuardrailsSettings,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyAdaptiveRoutingSettings {
    pub enabled: Option<bool>,
    pub shadow_mode: Option<bool>,
    pub window_size: Option<usize>,
    pub min_samples: Option<u64>,
    pub exploration_rate: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayStateSettings {
    pub backend: Option<String>,
    pub sqlite_path: Option<String>,
    pub postgres_url_env: Option<String>,
    pub redis_url_env: Option<String>,
    pub postgres_url_ref: Option<SecretRef>,
    pub redis_url_ref: Option<SecretRef>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayAdminToken {
    pub name: String,
    #[serde(default)]
    pub token_env: String,
    pub token_ref: Option<SecretRef>,
    pub role: Option<String>,
    pub tenant_id: Option<String>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub budget_id: Option<String>,
    #[serde(default)]
    pub allowed_key_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewaySsoSettings {
    pub proxy_token_env: Option<String>,
    pub proxy_token_ref: Option<SecretRef>,
    pub require_tenant: Option<bool>,
    pub token_header: Option<String>,
    pub user_header: Option<String>,
    pub role_header: Option<String>,
    pub tenant_header: Option<String>,
    pub key_prefixes_header: Option<String>,
    pub oidc_issuer: Option<String>,
    pub oidc_audience: Option<String>,
    pub oidc_jwks_url: Option<String>,
    pub oidc_user_claim: Option<String>,
    pub oidc_role_claim: Option<String>,
    pub oidc_tenant_claim: Option<String>,
    pub oidc_key_prefixes_claim: Option<String>,
    pub default_role: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayRouteAlias {
    pub alias: String,
    #[serde(default)]
    pub models: Vec<String>,
    pub strategy: Option<String>,
    #[serde(default)]
    pub model_metrics: Vec<RuntimePolicyGatewayRouteModelMetrics>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayRouteModelMetrics {
    pub model: String,
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
    pub latency_ms: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayVirtualKey {
    pub name: String,
    #[serde(default)]
    pub token_env: String,
    pub token_ref: Option<SecretRef>,
    pub tenant_id: Option<String>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub budget_id: Option<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    pub budget_usd: Option<f64>,
    pub request_budget: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayObservabilitySettings {
    #[serde(default)]
    pub sinks: Vec<String>,
    pub call_id_header: Option<String>,
    pub jsonl_path: Option<String>,
    pub http_endpoint: Option<String>,
    pub http_schema: Option<String>,
    pub http_bearer_token_env: Option<String>,
    pub http_bearer_token_ref: Option<SecretRef>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayGuardrailsSettings {
    #[serde(default)]
    pub blocked_keywords: Vec<String>,
    #[serde(default)]
    pub blocked_output_keywords: Vec<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    pub presidio_redaction: Option<bool>,
    pub prompt_injection_detection: Option<bool>,
    pub pii_redaction: Option<bool>,
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub webhook_phases: Vec<String>,
    pub webhook_bearer_token_env: Option<String>,
    pub webhook_bearer_token_ref: Option<SecretRef>,
    pub webhook_fail_closed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePolicyProxyPreset {
    Low,
    Default,
    ManyTerminals,
    Aggressive,
}

impl RuntimePolicyProxyPreset {
    pub const VALID_VALUES: &'static [&'static str] =
        &["low", "default", "many-terminals", "aggressive"];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Default => "default",
            Self::ManyTerminals => "many-terminals",
            Self::Aggressive => "aggressive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value != value.trim() {
            return None;
        }
        match value.to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "default" => Some(Self::Default),
            "many-terminals" | "many_terminals" => Some(Self::ManyTerminals),
            "aggressive" => Some(Self::Aggressive),
            _ => None,
        }
    }

    fn settings(self) -> RuntimePolicyProxySettings {
        let preset = RuntimePolicyProxyPresetSelection::selected(self);
        // Presets only tune local concurrency/admission knobs. Transport timeouts stay unset
        // to preserve upstream Codex stream and reconnect behavior.
        match self {
            Self::Low => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(4),
                long_lived_worker_count: Some(8),
                probe_refresh_worker_count: Some(2),
                async_worker_count: Some(2),
                long_lived_queue_capacity: Some(128),
                active_request_limit: Some(48),
                profile_inflight_soft_limit: Some(2),
                profile_inflight_hard_limit: Some(4),
                responses_active_limit: Some(36),
                compact_active_limit: Some(3),
                websocket_active_limit: Some(8),
                standard_active_limit: Some(2),
                websocket_connect_worker_count: Some(4),
                websocket_connect_queue_capacity: Some(32),
                websocket_connect_overflow_capacity: Some(64),
                websocket_dns_worker_count: Some(2),
                websocket_dns_queue_capacity: Some(16),
                websocket_dns_overflow_capacity: Some(32),
                startup_sync_probe_warm_limit: Some(1),
                ..RuntimePolicyProxySettings::default()
            },
            Self::Default => RuntimePolicyProxySettings {
                preset,
                ..RuntimePolicyProxySettings::default()
            },
            Self::ManyTerminals => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(12),
                long_lived_worker_count: Some(32),
                probe_refresh_worker_count: Some(4),
                async_worker_count: Some(4),
                long_lived_queue_capacity: Some(512),
                active_request_limit: Some(160),
                profile_inflight_soft_limit: Some(4),
                profile_inflight_hard_limit: Some(8),
                responses_active_limit: Some(120),
                compact_active_limit: Some(8),
                websocket_active_limit: Some(32),
                standard_active_limit: Some(8),
                websocket_connect_worker_count: Some(12),
                websocket_connect_queue_capacity: Some(96),
                websocket_connect_overflow_capacity: Some(384),
                websocket_dns_worker_count: Some(6),
                websocket_dns_queue_capacity: Some(48),
                websocket_dns_overflow_capacity: Some(96),
                startup_sync_probe_warm_limit: Some(2),
                ..RuntimePolicyProxySettings::default()
            },
            Self::Aggressive => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(24),
                long_lived_worker_count: Some(96),
                probe_refresh_worker_count: Some(8),
                async_worker_count: Some(8),
                long_lived_queue_capacity: Some(1024),
                active_request_limit: Some(384),
                profile_inflight_soft_limit: Some(8),
                profile_inflight_hard_limit: Some(16),
                responses_active_limit: Some(288),
                compact_active_limit: Some(16),
                websocket_active_limit: Some(96),
                standard_active_limit: Some(16),
                websocket_connect_worker_count: Some(16),
                websocket_connect_queue_capacity: Some(128),
                websocket_connect_overflow_capacity: Some(512),
                websocket_dns_worker_count: Some(8),
                websocket_dns_queue_capacity: Some(64),
                websocket_dns_overflow_capacity: Some(128),
                startup_sync_probe_warm_limit: Some(3),
                ..RuntimePolicyProxySettings::default()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimePolicyProxyPresetSelection(Option<RuntimePolicyProxyPreset>);

impl RuntimePolicyProxyPresetSelection {
    pub fn selected(preset: RuntimePolicyProxyPreset) -> Self {
        Self(Some(preset))
    }

    pub fn get(self) -> Option<RuntimePolicyProxyPreset> {
        self.0
    }
}

impl<'de> Deserialize<'de> for RuntimePolicyProxyPresetSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        RuntimePolicyProxyPreset::parse(&value)
            .map(Self::selected)
            .ok_or_else(|| {
                de::Error::unknown_variant(value.as_str(), RuntimePolicyProxyPreset::VALID_VALUES)
            })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyProxySettings {
    #[serde(default)]
    pub preset: RuntimePolicyProxyPresetSelection,
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
    pub compact_request_timeout_ms: Option<u64>,
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

impl RuntimePolicyProxySettings {
    pub fn preset(&self) -> Option<RuntimePolicyProxyPreset> {
        self.preset.get()
    }

    pub fn with_effective_preset(
        self,
        env_preset: Option<RuntimePolicyProxyPreset>,
    ) -> RuntimePolicyProxySettings {
        let selected_preset = env_preset.or_else(|| self.preset());
        let Some(selected_preset) = selected_preset else {
            return self;
        };

        let mut effective = selected_preset.settings();
        effective.apply_non_preset_overrides(self);
        effective.preset = RuntimePolicyProxyPresetSelection::selected(selected_preset);
        effective
    }

    fn apply_non_preset_overrides(&mut self, overrides: RuntimePolicyProxySettings) {
        macro_rules! apply_optional_overrides {
            ($($field:ident),+ $(,)?) => {
                $(
                    if overrides.$field.is_some() {
                        self.$field = overrides.$field;
                    }
                )+
            };
        }

        apply_optional_overrides!(
            worker_count,
            long_lived_worker_count,
            probe_refresh_worker_count,
            async_worker_count,
            long_lived_queue_capacity,
            active_request_limit,
            profile_inflight_soft_limit,
            profile_inflight_hard_limit,
            responses_active_limit,
            compact_active_limit,
            websocket_active_limit,
            standard_active_limit,
            http_connect_timeout_ms,
            stream_idle_timeout_ms,
            compact_request_timeout_ms,
            sse_lookahead_timeout_ms,
            prefetch_backpressure_retry_ms,
            prefetch_backpressure_timeout_ms,
            prefetch_max_buffered_bytes,
            websocket_connect_timeout_ms,
            websocket_happy_eyeballs_delay_ms,
            websocket_precommit_progress_timeout_ms,
            websocket_connect_worker_count,
            websocket_connect_queue_capacity,
            websocket_connect_overflow_capacity,
            websocket_dns_worker_count,
            websocket_dns_queue_capacity,
            websocket_dns_overflow_capacity,
            broker_ready_timeout_ms,
            broker_health_connect_timeout_ms,
            broker_health_read_timeout_ms,
            websocket_previous_response_reuse_stale_ms,
            admission_wait_budget_ms,
            pressure_admission_wait_budget_ms,
            long_lived_queue_wait_budget_ms,
            pressure_long_lived_queue_wait_budget_ms,
            sync_probe_pressure_pause_ms,
            responses_critical_floor_percent,
            startup_sync_probe_warm_limit,
        );
    }
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
    pub gateway: RuntimePolicyGatewaySettings,
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
    #[serde(default)]
    pub production: bool,
    pub projected_root: Option<String>,
    pub projected_provider: Option<String>,
}
