use super::ConfigError;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;

const RUNTIME_CONFIG_ENV_KEYS: &[&str] = &[
    prodex_runtime_policy::PRODEX_RUNTIME_PROXY_PRESET_ENV,
    "PRODEX_RUNTIME_LOG_DIR",
    "PRODEX_RUNTIME_LOG_FORMAT",
    "PRODEX_RUNTIME_PROXY_WORKER_COUNT",
    "PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT",
    "PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT",
    "PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT",
    "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY",
    "PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT",
    "PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT",
    "PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT",
    "PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT",
    "PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT",
    "PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS",
    "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
    "PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS",
    "PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS",
    "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS",
    "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS",
    "PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES",
    "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
    "PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS",
    "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
    "PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT",
    "PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY",
    "PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY",
    "PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT",
    "PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY",
    "PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY",
    "PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS",
    "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
    "PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS",
    "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
    "PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
    "PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS",
    "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT",
    "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT",
    "PRODEX_RUNTIME_PROXY_RESPONSES_CRITICAL_FLOOR_PERCENT",
    "PRODEX_RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT",
    "PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS",
    "PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS",
    "PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS",
    "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES",
    "PRODEX_DEBUG_ANTHROPIC_COMPAT",
    "PRODEX_SMART_CONTEXT_SHADOW",
    "PRODEX_SMART_CONTEXT_CANARY_PERCENT",
    "PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE",
    "PRODEX_RUNTIME_FAULT_STREAM_READ_ERROR_ONCE",
    "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE",
    "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_UNWIND_ONCE",
    "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS",
    "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS",
    "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS",
    "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS",
    "PRODEX_GEMINI_EXTENSION_DIRS",
    "PRODEX_GEMINI_EXTENSIONS",
    "PRODEX_GEMINI_EXPORT_FILE",
    "PRODEX_GEMINI_CHECKPOINT_EXPORT_FILE",
    "PRODEX_GEMINI_SESSION_FILE",
    "PRODEX_GEMINI_CHECKPOINT_FILE",
    "PRODEX_GEMINI_IMPORT_FILE",
    "PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD",
    "PRODEX_GEMINI_TOOL_OUTPUT_DIR",
    "PRODEX_GEMINI_DISABLE_MEMORY",
    "PRODEX_GEMINI_DISABLE_CONTEXT_FILES",
    "PRODEX_GEMINI_LOAD_MEMORY",
    "PRODEX_GEMINI_MEMORY",
    "PRODEX_GEMINI_EXTENSION_MEMORY",
    "PRODEX_GEMINI_LIVE_URL",
    "PRODEX_GEMINI_LIVE_MODEL",
    "PRODEX_GEMINI_STICKY_FRESH_OAUTH",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "all_proxy",
    "PROXY",
    "proxy",
    "NO_PROXY",
    "no_proxy",
];

#[derive(Default)]
pub(super) struct RuntimeConfigEnvironment(BTreeMap<&'static str, Option<OsString>>);

impl RuntimeConfigEnvironment {
    pub(super) fn read_process() -> Self {
        Self::read_with(|key| env::var_os(key))
    }

    pub(super) fn read_with(mut read: impl FnMut(&str) -> Option<OsString>) -> Self {
        Self(
            RUNTIME_CONFIG_ENV_KEYS
                .iter()
                .copied()
                .map(|key| (key, read(key)))
                .collect(),
        )
    }

    pub(super) fn get(&self, key: &'static str) -> Option<&OsString> {
        self.0.get(key).and_then(Option::as_ref)
    }
}

pub(super) struct RuntimeConfigParser {
    pub(super) environment: RuntimeConfigEnvironment,
    pub(super) errors: Vec<ConfigError>,
    pub(super) compatibility_defaults: Vec<&'static str>,
}

impl RuntimeConfigParser {
    pub(super) fn new(environment: RuntimeConfigEnvironment) -> Self {
        Self {
            environment,
            errors: Vec::new(),
            compatibility_defaults: Vec::new(),
        }
    }

    fn strict_text(&mut self, key: &'static str) -> Option<String> {
        let value = self.environment.get(key)?;
        let Some(value) = value.to_str() else {
            self.errors.push(ConfigError {
                key,
                message: "must be valid Unicode",
            });
            return None;
        };
        if value.is_empty() {
            self.errors.push(ConfigError {
                key,
                message: "cannot be empty",
            });
            return None;
        }
        if value.chars().any(char::is_whitespace) {
            self.errors.push(ConfigError {
                key,
                message: "must not contain whitespace",
            });
            return None;
        }
        Some(value.to_string())
    }

    pub(super) fn positive_u64(
        &mut self,
        key: &'static str,
        policy: Option<u64>,
        default: u64,
    ) -> u64 {
        let Some(value) = self.strict_text(key) else {
            return policy.filter(|value| *value > 0).unwrap_or(default);
        };
        match value.parse::<u64>() {
            Ok(value) if value > 0 => value,
            Ok(_) => {
                self.errors.push(ConfigError {
                    key,
                    message: "must be greater than zero",
                });
                policy.filter(|value| *value > 0).unwrap_or(default)
            }
            Err(_) => {
                self.errors.push(ConfigError {
                    key,
                    message: "must be an unsigned integer",
                });
                policy.filter(|value| *value > 0).unwrap_or(default)
            }
        }
    }

    pub(super) fn bounded_u64(
        &mut self,
        key: &'static str,
        default: u64,
        allow_zero: bool,
        maximum: u64,
    ) -> u64 {
        let Some(value) = self.strict_text(key) else {
            return default;
        };
        match value.parse::<u64>() {
            Ok(value) if !allow_zero && value == 0 => {
                self.errors.push(ConfigError {
                    key,
                    message: "must be greater than zero",
                });
                default
            }
            Ok(value) if value > maximum => {
                self.errors.push(ConfigError {
                    key,
                    message: "must not exceed maximum",
                });
                default
            }
            Ok(value) => value,
            Err(_) => {
                self.errors.push(ConfigError {
                    key,
                    message: "must be an unsigned integer",
                });
                default
            }
        }
    }

    pub(super) fn positive_usize(
        &mut self,
        key: &'static str,
        policy: Option<usize>,
        default: usize,
    ) -> usize {
        self.positive_u64(key, policy.map(|value| value as u64), default as u64)
            .min(usize::MAX as u64) as usize
    }

    pub(super) fn usize_allow_zero(
        &mut self,
        key: &'static str,
        policy: Option<usize>,
        default: usize,
    ) -> usize {
        let Some(value) = self.strict_text(key) else {
            return policy.unwrap_or(default);
        };
        match value.parse::<usize>() {
            Ok(value) => value,
            Err(_) => {
                self.errors.push(ConfigError {
                    key,
                    message: "must be an unsigned integer",
                });
                policy.unwrap_or(default)
            }
        }
    }

    pub(super) fn positive_i64(
        &mut self,
        key: &'static str,
        policy: Option<i64>,
        default: i64,
    ) -> i64 {
        let Some(value) = self.strict_text(key) else {
            return policy.filter(|value| *value > 0).unwrap_or(default);
        };
        match value.parse::<i64>() {
            Ok(value) if value > 0 => value,
            Ok(_) => {
                self.errors.push(ConfigError {
                    key,
                    message: "must be greater than zero",
                });
                policy.filter(|value| *value > 0).unwrap_or(default)
            }
            Err(_) => {
                self.errors.push(ConfigError {
                    key,
                    message: "must be a positive integer",
                });
                policy.filter(|value| *value > 0).unwrap_or(default)
            }
        }
    }

    pub(super) fn compatibility_u64(
        &mut self,
        key: &'static str,
        fallback: u64,
        trim: bool,
        allow_zero: bool,
        maximum: u64,
    ) -> u64 {
        let Some(value) = self.environment.get(key) else {
            return fallback;
        };
        let Some(value) = value.to_str() else {
            self.compatibility_defaults.push(key);
            return fallback;
        };
        let value = if trim { value.trim() } else { value };
        match value.parse::<u64>() {
            Ok(value) if allow_zero || value > 0 => value.min(maximum),
            _ => {
                self.compatibility_defaults.push(key);
                fallback
            }
        }
    }

    pub(super) fn compatibility_flag(&self, key: &'static str) -> bool {
        self.environment
            .get(key)
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
    }

    pub(super) fn compatibility_optional_bool(&mut self, key: &'static str) -> Option<bool> {
        let value = self.environment.get(key)?;
        let Some(value) = value.to_str() else {
            self.compatibility_defaults.push(key);
            return None;
        };
        let parsed = match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        };
        if parsed.is_none() {
            self.compatibility_defaults.push(key);
        }
        parsed
    }

    pub(super) fn compatibility_text(&mut self, key: &'static str) -> Option<String> {
        let value = self.environment.get(key)?;
        let Some(value) = value.to_str() else {
            self.compatibility_defaults.push(key);
            return None;
        };
        Some(value.to_string())
    }

    pub(super) fn compatibility_proxy(&mut self, keys: &[&'static str]) -> Option<reqwest::Url> {
        for key in keys {
            let Some(value) = self.environment.get(key) else {
                continue;
            };
            let candidate = value.to_string_lossy();
            if let Some(url) =
                runtime_proxy_crate::runtime_websocket_proxy_url_candidate(&candidate)
                    .and_then(|candidate| reqwest::Url::parse(&candidate).ok())
            {
                return Some(url);
            }
            self.compatibility_defaults.push(key);
        }
        None
    }
}
