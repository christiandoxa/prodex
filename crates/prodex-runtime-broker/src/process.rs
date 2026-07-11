use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeBrokerObservation {
    pub broker_key: String,
    pub listen_addr: String,
    pub metrics: RuntimeBrokerMetrics,
}

#[derive(Debug, Clone)]
pub struct RuntimeBrokerMetadata {
    pub broker_key: String,
    pub listen_addr: String,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub instance_token: String,
    pub admin_token: String,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBrokerSpawnConfig<'a> {
    pub current_profile: &'a str,
    pub upstream_base_url: &'a str,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub smart_context_enabled: bool,
    pub model_context_window_tokens: Option<u64>,
    pub broker_key: &'a str,
    pub instance_token: &'a str,
    pub admin_token: &'a str,
    pub listen_addr: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBrokerProcessCommandPlan {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub prodex_home: PathBuf,
}

pub fn runtime_broker_process_args(config: RuntimeBrokerSpawnConfig<'_>) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("__runtime-broker"),
        OsString::from("--current-profile"),
        OsString::from(config.current_profile),
        OsString::from("--upstream-base-url"),
        OsString::from(config.upstream_base_url),
    ];
    if config.include_code_review {
        args.push(OsString::from("--include-code-review"));
    }
    if config.upstream_no_proxy {
        args.push(OsString::from("--upstream-no-proxy"));
    }
    if config.smart_context_enabled {
        args.push(OsString::from("--smart-context"));
        if let Some(model_context_window_tokens) = config.model_context_window_tokens {
            args.push(OsString::from("--model-context-window-tokens"));
            args.push(OsString::from(model_context_window_tokens.to_string()));
        }
    }
    args.extend([
        OsString::from("--broker-key"),
        OsString::from(config.broker_key),
        OsString::from("--instance-token"),
        OsString::from(config.instance_token),
        OsString::from("--admin-token"),
        OsString::from(config.admin_token),
    ]);
    if let Some(listen_addr) = config.listen_addr {
        args.push(OsString::from("--listen-addr"));
        args.push(OsString::from(listen_addr));
    }
    args
}

pub fn runtime_broker_process_command_plan(
    executable: impl Into<PathBuf>,
    prodex_home: impl Into<PathBuf>,
    config: RuntimeBrokerSpawnConfig<'_>,
) -> RuntimeBrokerProcessCommandPlan {
    RuntimeBrokerProcessCommandPlan {
        executable: executable.into(),
        args: runtime_broker_process_args(config),
        prodex_home: prodex_home.into(),
    }
}

pub fn runtime_broker_startup_grace_seconds(ready_timeout_ms: u64, idle_grace_seconds: i64) -> i64 {
    let ready_timeout_seconds = ready_timeout_ms.div_ceil(1_000) as i64;
    ready_timeout_seconds
        .saturating_add(1)
        .max(idle_grace_seconds)
}
