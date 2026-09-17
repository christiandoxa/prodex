use prodex_runtime_policy::RuntimeLogFormat;
use prodex_runtime_tuning::RuntimeTuningSnapshot;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct RuntimeConfig {
    pub(crate) tuning: RuntimeTuningSnapshot,
    pub(crate) compact_request_timeout_ms: u64,
    pub(crate) prefetch_backpressure_retry_ms: u64,
    pub(crate) prefetch_backpressure_timeout_ms: u64,
    pub(crate) prefetch_max_buffered_bytes: usize,
    pub(crate) sync_probe_pressure_pause_ms: u64,
    pub(crate) responses_quota_critical_floor_percent: i64,
    pub(crate) startup_sync_probe_warm_limit: usize,
    pub(crate) broker_ready_timeout_ms: u64,
    pub(crate) broker_health_connect_timeout_ms: u64,
    pub(crate) broker_health_read_timeout_ms: u64,
    pub(crate) max_request_body_bytes: u64,
    pub(crate) debug_anthropic_compat: bool,
    pub(crate) smart_context_shadow: bool,
    pub(crate) smart_context_canary_percent: u8,
    pub(crate) fault_upstream_connect_error_once: usize,
    pub(crate) fault_stream_read_error_once: usize,
    pub(crate) fault_smart_context_panic_once: usize,
    pub(crate) fault_smart_context_unwind_once: usize,
    pub(crate) log_dir: PathBuf,
    pub(crate) log_format: RuntimeLogFormat,
    pub(crate) response_chain_trace: bool,
    pub(crate) websocket_environment: RuntimeWebsocketEnvironment,
    pub(crate) governance: prodex_config::GovernanceConfig,
    pub(crate) tenant_detector_patterns:
        crate::runtime_proxy::presidio::local::RuntimeTenantDetectorPatterns,
    pub(crate) gemini: RuntimeGeminiConfig,
    pub(super) compatibility_defaults: Vec<&'static str>,
}

#[derive(Clone)]
pub(crate) struct RuntimeGeminiConfig {
    pub(crate) sticky_fresh_oauth: bool,
}

impl fmt::Debug for RuntimeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeConfig")
            .field("tuning", &self.tuning)
            .field("max_request_body_bytes", &self.max_request_body_bytes)
            .field("debug_anthropic_compat", &self.debug_anthropic_compat)
            .field("smart_context_shadow", &self.smart_context_shadow)
            .field(
                "smart_context_canary_percent",
                &self.smart_context_canary_percent,
            )
            .field("log_format", &self.log_format)
            .field("response_chain_trace", &self.response_chain_trace)
            .field("governance_mode", &self.governance.mode.as_str())
            .field(
                "websocket_proxy_configured",
                &self.websocket_environment.has_proxy(),
            )
            .field("compatibility_defaults", &self.compatibility_defaults)
            .finish()
    }
}

impl RuntimeConfig {
    pub(crate) fn compatibility_defaults(&self) -> &[&'static str] {
        &self.compatibility_defaults
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeWebsocketEnvironment {
    pub(super) https_proxy: Option<reqwest::Url>,
    pub(super) http_proxy: Option<reqwest::Url>,
    pub(super) no_proxy: Vec<String>,
}

impl RuntimeWebsocketEnvironment {
    #[cfg(test)]
    pub(crate) fn direct() -> Self {
        Self {
            https_proxy: None,
            http_proxy: None,
            no_proxy: Vec::new(),
        }
    }

    pub(crate) fn proxy_url(&self, scheme: &str) -> Option<reqwest::Url> {
        if matches!(scheme, "wss" | "https") {
            self.https_proxy.clone()
        } else {
            self.http_proxy.clone()
        }
    }

    pub(crate) fn no_proxy_matches(&self, host: &str, port: u16) -> bool {
        self.no_proxy.iter().any(|value| {
            runtime_proxy_crate::runtime_websocket_no_proxy_value_matches(value, host, port)
        })
    }

    fn has_proxy(&self) -> bool {
        self.https_proxy.is_some() || self.http_proxy.is_some()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ConfigError {
    pub(super) key: &'static str,
    pub(super) message: String,
}

impl fmt::Debug for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigError")
            .field("key", &self.key)
            .field("message", &self.message)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConfigErrors(pub(super) Vec<ConfigError>);

impl fmt::Display for ConfigErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime configuration is invalid")?;
        for error in &self.0 {
            write!(formatter, "; {} {}", error.key, error.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigErrors {}
