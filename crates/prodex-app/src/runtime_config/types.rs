use prodex_runtime_policy::RuntimeLogFormat;
use prodex_runtime_tuning::RuntimeTuningSnapshot;
use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

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
    pub(crate) websocket_environment: RuntimeWebsocketEnvironment,
    pub(crate) oidc: RuntimeOidcTimingConfig,
    pub(crate) gateway: RuntimeGatewayConfig,
    pub(crate) governance: prodex_config::GovernanceConfig,
    pub(crate) governance_policy: prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    pub(crate) governance_snapshot: Option<prodex_application::ApplicationGovernanceSnapshot>,
    pub(crate) gemini: RuntimeGeminiConfig,
    pub(super) compatibility_defaults: Vec<&'static str>,
}

#[derive(Clone)]
pub(crate) struct RuntimeGeminiConfig {
    pub(crate) home_dir: Option<PathBuf>,
    pub(crate) config_dir: Option<PathBuf>,
    pub(crate) system_settings_path: Option<PathBuf>,
    pub(crate) system_defaults_path: Option<PathBuf>,
    pub(crate) extension_dirs: Vec<PathBuf>,
    pub(crate) extension_selection: RuntimeGeminiExtensionSelection,
    pub(crate) export_checkpoint_path: Option<PathBuf>,
    pub(crate) import_paths: Vec<PathBuf>,
    pub(crate) tool_output_mask_threshold: usize,
    pub(crate) tool_output_dir: Option<PathBuf>,
    pub(crate) memory_files_disabled: bool,
    pub(crate) memory_files_default: bool,
    pub(crate) extension_memory_paths: Vec<PathBuf>,
    pub(crate) live_url: Option<String>,
    pub(crate) live_model: Option<String>,
    pub(crate) sticky_fresh_oauth: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeGatewayConfig {
    pub(crate) replica_count: u16,
    pub(crate) require_multi_replica_accounting_checks: bool,
    pub(crate) launch: RuntimeGatewayLaunchEnvironment,
}

#[derive(Clone, Default)]
pub(crate) enum RuntimeGatewayLaunchEnvironment {
    #[default]
    Unspecified,
    ControlPlane,
    DataPlane {
        upstream_base_url: String,
        deepseek: Option<RuntimeGatewayDeepSeekConfig>,
        gemini_model_resolution: Option<crate::RuntimeGeminiModelResolution>,
        present_secret_env: BTreeSet<&'static str>,
    },
}

#[derive(Clone)]
pub(crate) struct RuntimeGatewayDeepSeekConfig {
    pub(crate) strict_tools: bool,
    pub(crate) beta_base_url: String,
    pub(crate) web_search_mode: crate::RuntimeDeepSeekWebSearchMode,
}

impl fmt::Debug for RuntimeGatewayLaunchEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unspecified => formatter.write_str("Unspecified"),
            Self::ControlPlane => formatter.write_str("ControlPlane"),
            Self::DataPlane {
                deepseek,
                gemini_model_resolution,
                present_secret_env,
                ..
            } => formatter
                .debug_struct("DataPlane")
                .field("upstream_base_url", &"<redacted>")
                .field("deepseek", deepseek)
                .field(
                    "gemini_model_resolution_configured",
                    &gemini_model_resolution.is_some(),
                )
                .field("present_secret_env", present_secret_env)
                .finish(),
        }
    }
}

impl fmt::Debug for RuntimeGatewayDeepSeekConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeGatewayDeepSeekConfig")
            .field("strict_tools", &self.strict_tools)
            .field("beta_base_url", &"<redacted>")
            .field("web_search_mode", &self.web_search_mode)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeEnvironmentSecretRef {
    pub(crate) name: &'static str,
    pub(crate) list: bool,
}

impl RuntimeGatewayLaunchEnvironment {
    pub(crate) fn upstream_base_url(&self) -> Option<&str> {
        match self {
            Self::DataPlane {
                upstream_base_url, ..
            } => Some(upstream_base_url),
            Self::Unspecified | Self::ControlPlane => None,
        }
    }

    pub(crate) fn deepseek(&self) -> Option<&RuntimeGatewayDeepSeekConfig> {
        match self {
            Self::DataPlane { deepseek, .. } => deepseek.as_ref(),
            Self::Unspecified | Self::ControlPlane => None,
        }
    }

    pub(crate) fn gemini_model_resolution(&self) -> Option<&crate::RuntimeGeminiModelResolution> {
        match self {
            Self::DataPlane {
                gemini_model_resolution,
                ..
            } => gemini_model_resolution.as_ref(),
            Self::Unspecified | Self::ControlPlane => None,
        }
    }

    pub(crate) fn secret_env_present(&self, name: &str) -> bool {
        matches!(
            self,
            Self::DataPlane {
                present_secret_env,
                ..
            } if present_secret_env.contains(name)
        )
    }

    pub(crate) fn first_secret_env(
        &self,
        names: &[&'static str],
        list: bool,
    ) -> Option<RuntimeEnvironmentSecretRef> {
        names
            .iter()
            .copied()
            .find(|name| self.secret_env_present(name))
            .map(|name| RuntimeEnvironmentSecretRef { name, list })
    }
}

impl RuntimeGeminiConfig {
    pub(crate) const DEFAULT_TOOL_OUTPUT_MASK_THRESHOLD: usize = 50_000;

    pub(crate) fn extension_enabled_override(&self, name: &str) -> Option<bool> {
        match &self.extension_selection {
            RuntimeGeminiExtensionSelection::All => None,
            RuntimeGeminiExtensionSelection::None => Some(false),
            RuntimeGeminiExtensionSelection::Names(names) => {
                Some(names.contains(&name.to_ascii_lowercase()))
            }
        }
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeGeminiExtensionSelection {
    All,
    None,
    Names(BTreeSet<String>),
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
            .field("gateway", &self.gateway)
            .field("governance_mode", &self.governance.mode.as_str())
            .field(
                "governance_snapshot_loaded",
                &self.governance_snapshot.is_some(),
            )
            .field("gemini_home_configured", &self.gemini.home_dir.is_some())
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeOidcTimingConfig {
    pub(crate) prefetch_timeout: Duration,
    pub(crate) http_cache_ttl: Duration,
    pub(crate) refresh_failure_backoff: Duration,
    pub(crate) last_known_good_window: Duration,
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
