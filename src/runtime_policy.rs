use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(test)]
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::{AppPaths, secret_store::SecretBackendKind};

const PRODEX_POLICY_FILE_NAME: &str = "policy.toml";
pub(super) const PRODEX_POLICY_VERSION: u32 = 1;

static RUNTIME_POLICY_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum RuntimeLogFormat {
    Text,
    Json,
}

impl RuntimeLogFormat {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimePolicySummary {
    pub(super) path: PathBuf,
    pub(super) version: u32,
}

#[derive(Debug, Clone)]
struct RuntimePolicyConfig {
    path: PathBuf,
    version: u32,
    runtime: RuntimePolicyRuntimeSettings,
    runtime_proxy: RuntimePolicyProxySettings,
    secrets: RuntimePolicySecretsSettings,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimePolicyRuntimeSettings {
    pub(super) log_format: Option<RuntimeLogFormat>,
    pub(super) log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimePolicySecretsSettings {
    pub(super) backend: Option<SecretBackendKind>,
    pub(super) keyring_service: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimePolicyProxySettings {
    pub(super) worker_count: Option<usize>,
    pub(super) long_lived_worker_count: Option<usize>,
    pub(super) probe_refresh_worker_count: Option<usize>,
    pub(super) async_worker_count: Option<usize>,
    pub(super) long_lived_queue_capacity: Option<usize>,
    pub(super) active_request_limit: Option<usize>,
    pub(super) profile_inflight_soft_limit: Option<usize>,
    pub(super) profile_inflight_hard_limit: Option<usize>,
    pub(super) responses_active_limit: Option<usize>,
    pub(super) compact_active_limit: Option<usize>,
    pub(super) websocket_active_limit: Option<usize>,
    pub(super) standard_active_limit: Option<usize>,
    pub(super) http_connect_timeout_ms: Option<u64>,
    pub(super) stream_idle_timeout_ms: Option<u64>,
    pub(super) sse_lookahead_timeout_ms: Option<u64>,
    pub(super) prefetch_backpressure_retry_ms: Option<u64>,
    pub(super) prefetch_backpressure_timeout_ms: Option<u64>,
    pub(super) prefetch_max_buffered_bytes: Option<usize>,
    pub(super) websocket_connect_timeout_ms: Option<u64>,
    pub(super) websocket_happy_eyeballs_delay_ms: Option<u64>,
    pub(super) websocket_precommit_progress_timeout_ms: Option<u64>,
    pub(super) broker_ready_timeout_ms: Option<u64>,
    pub(super) broker_health_connect_timeout_ms: Option<u64>,
    pub(super) broker_health_read_timeout_ms: Option<u64>,
    pub(super) websocket_previous_response_reuse_stale_ms: Option<u64>,
    pub(super) admission_wait_budget_ms: Option<u64>,
    pub(super) pressure_admission_wait_budget_ms: Option<u64>,
    pub(super) long_lived_queue_wait_budget_ms: Option<u64>,
    pub(super) pressure_long_lived_queue_wait_budget_ms: Option<u64>,
    pub(super) sync_probe_pressure_pause_ms: Option<u64>,
    pub(super) responses_critical_floor_percent: Option<i64>,
    pub(super) startup_sync_probe_warm_limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimePolicyFile {
    version: u32,
    #[serde(default)]
    runtime: RuntimePolicyRuntimeFile,
    #[serde(default)]
    runtime_proxy: RuntimePolicyProxySettings,
    #[serde(default)]
    secrets: RuntimePolicySecretsFile,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimePolicyRuntimeFile {
    log_format: Option<RuntimeLogFormat>,
    log_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimePolicySecretsFile {
    backend: Option<String>,
    keyring_service: Option<String>,
}

fn runtime_policy_cache() -> &'static Mutex<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>> {
    RUNTIME_POLICY_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[cfg(test)]
pub(super) fn clear_runtime_policy_cache() {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

fn runtime_policy_path(root: &Path) -> PathBuf {
    root.join(PRODEX_POLICY_FILE_NAME)
}

#[cfg(test)]
fn runtime_policy_enabled_for_current_process() -> bool {
    env::var_os("PRODEX_HOME").is_some()
}

#[cfg(not(test))]
fn runtime_policy_enabled_for_current_process() -> bool {
    true
}

pub(super) fn ensure_runtime_policy_valid() -> Result<()> {
    if !runtime_policy_enabled_for_current_process() {
        return Ok(());
    }
    let paths = AppPaths::discover()?;
    let _ = load_runtime_policy_cached(&paths.root)?;
    Ok(())
}

pub(super) fn runtime_policy_summary() -> Result<Option<RuntimePolicySummary>> {
    if !runtime_policy_enabled_for_current_process() {
        return Ok(None);
    }
    let paths = AppPaths::discover()?;
    Ok(
        load_runtime_policy_cached(&paths.root)?.map(|config| RuntimePolicySummary {
            path: config.path,
            version: config.version,
        }),
    )
}

pub(super) fn runtime_policy_runtime() -> Option<RuntimePolicyRuntimeSettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.runtime)
}

pub(super) fn runtime_policy_proxy() -> Option<RuntimePolicyProxySettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.runtime_proxy)
}

pub(super) fn runtime_policy_secrets() -> Option<RuntimePolicySecretsSettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.secrets)
}

fn load_runtime_policy_cached(root: &Path) -> Result<Option<RuntimePolicyConfig>> {
    let mut cache = runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(cached) = cache.get(root).cloned() {
        return Ok(cached);
    }
    let loaded = load_runtime_policy_from_root(root)?;
    cache.insert(root.to_path_buf(), loaded.clone());
    Ok(loaded)
}

fn load_runtime_policy_from_root(root: &Path) -> Result<Option<RuntimePolicyConfig>> {
    let path = runtime_policy_path(root);
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: RuntimePolicyFile =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    validate_runtime_policy_file(&parsed, &path)?;

    let runtime = RuntimePolicyRuntimeSettings {
        log_format: parsed.runtime.log_format,
        log_dir: parsed
            .runtime
            .log_dir
            .as_deref()
            .map(|value| resolve_runtime_policy_path(root, value))
            .transpose()?,
    };
    let secrets = RuntimePolicySecretsSettings {
        backend: parsed
            .secrets
            .backend
            .as_deref()
            .map(parse_secret_backend_kind)
            .transpose()?,
        keyring_service: parsed
            .secrets
            .keyring_service
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    };

    Ok(Some(RuntimePolicyConfig {
        path,
        version: parsed.version,
        runtime,
        runtime_proxy: parsed.runtime_proxy,
        secrets,
    }))
}

fn resolve_runtime_policy_path(root: &Path, value: &str) -> Result<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("policy path values cannot be empty");
    }
    let path = PathBuf::from(trimmed);
    Ok(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

fn validate_runtime_policy_file(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if policy.version != PRODEX_POLICY_VERSION {
        bail!(
            "unsupported prodex policy version {} in {}; expected {}",
            policy.version,
            path.display(),
            PRODEX_POLICY_VERSION
        );
    }

    if let Some(log_dir) = policy.runtime.log_dir.as_deref()
        && log_dir.trim().is_empty()
    {
        bail!("runtime.log_dir in {} cannot be empty", path.display());
    }
    let secret_backend = if let Some(backend) = policy.secrets.backend.as_deref() {
        Some(
            parse_secret_backend_kind(backend)
                .with_context(|| format!("invalid secrets.backend in {}", path.display()))?,
        )
    } else {
        None
    };
    if secret_backend == Some(SecretBackendKind::Keyring)
        && policy
            .secrets
            .keyring_service
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty())
    {
        bail!(
            "secrets.keyring_service in {} is required when secrets.backend=keyring",
            path.display()
        );
    }
    if let Some(service) = policy.secrets.keyring_service.as_deref()
        && service.trim().is_empty()
    {
        bail!(
            "secrets.keyring_service in {} cannot be empty",
            path.display()
        );
    }

    validate_optional_usize(
        policy.runtime_proxy.worker_count,
        path,
        "runtime_proxy.worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.long_lived_worker_count,
        path,
        "runtime_proxy.long_lived_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.probe_refresh_worker_count,
        path,
        "runtime_proxy.probe_refresh_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.async_worker_count,
        path,
        "runtime_proxy.async_worker_count",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.long_lived_queue_capacity,
        path,
        "runtime_proxy.long_lived_queue_capacity",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.active_request_limit,
        path,
        "runtime_proxy.active_request_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.profile_inflight_soft_limit,
        path,
        "runtime_proxy.profile_inflight_soft_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.profile_inflight_hard_limit,
        path,
        "runtime_proxy.profile_inflight_hard_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.responses_active_limit,
        path,
        "runtime_proxy.responses_active_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.compact_active_limit,
        path,
        "runtime_proxy.compact_active_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.websocket_active_limit,
        path,
        "runtime_proxy.websocket_active_limit",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.standard_active_limit,
        path,
        "runtime_proxy.standard_active_limit",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.http_connect_timeout_ms,
        path,
        "runtime_proxy.http_connect_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.stream_idle_timeout_ms,
        path,
        "runtime_proxy.stream_idle_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.sse_lookahead_timeout_ms,
        path,
        "runtime_proxy.sse_lookahead_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.prefetch_backpressure_retry_ms,
        path,
        "runtime_proxy.prefetch_backpressure_retry_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.prefetch_backpressure_timeout_ms,
        path,
        "runtime_proxy.prefetch_backpressure_timeout_ms",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.prefetch_max_buffered_bytes,
        path,
        "runtime_proxy.prefetch_max_buffered_bytes",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.websocket_connect_timeout_ms,
        path,
        "runtime_proxy.websocket_connect_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.websocket_happy_eyeballs_delay_ms,
        path,
        "runtime_proxy.websocket_happy_eyeballs_delay_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.websocket_precommit_progress_timeout_ms,
        path,
        "runtime_proxy.websocket_precommit_progress_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.broker_ready_timeout_ms,
        path,
        "runtime_proxy.broker_ready_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.broker_health_connect_timeout_ms,
        path,
        "runtime_proxy.broker_health_connect_timeout_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.broker_health_read_timeout_ms,
        path,
        "runtime_proxy.broker_health_read_timeout_ms",
    )?;
    validate_optional_u64(
        policy
            .runtime_proxy
            .websocket_previous_response_reuse_stale_ms,
        path,
        "runtime_proxy.websocket_previous_response_reuse_stale_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.admission_wait_budget_ms,
        path,
        "runtime_proxy.admission_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.pressure_admission_wait_budget_ms,
        path,
        "runtime_proxy.pressure_admission_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.long_lived_queue_wait_budget_ms,
        path,
        "runtime_proxy.long_lived_queue_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy
            .runtime_proxy
            .pressure_long_lived_queue_wait_budget_ms,
        path,
        "runtime_proxy.pressure_long_lived_queue_wait_budget_ms",
    )?;
    validate_optional_u64(
        policy.runtime_proxy.sync_probe_pressure_pause_ms,
        path,
        "runtime_proxy.sync_probe_pressure_pause_ms",
    )?;
    validate_optional_i64_percent(
        policy.runtime_proxy.responses_critical_floor_percent,
        path,
        "runtime_proxy.responses_critical_floor_percent",
    )?;
    validate_optional_usize(
        policy.runtime_proxy.startup_sync_probe_warm_limit,
        path,
        "runtime_proxy.startup_sync_probe_warm_limit",
    )?;

    Ok(())
}

fn parse_secret_backend_kind(value: &str) -> Result<SecretBackendKind> {
    value
        .parse::<SecretBackendKind>()
        .map_err(anyhow::Error::new)
}

fn validate_optional_usize(value: Option<usize>, path: &Path, field: &str) -> Result<()> {
    if matches!(value, Some(0)) {
        bail!("{field} in {} must be greater than 0", path.display());
    }
    Ok(())
}

fn validate_optional_u64(value: Option<u64>, path: &Path, field: &str) -> Result<()> {
    if matches!(value, Some(0)) {
        bail!("{field} in {} must be greater than 0", path.display());
    }
    Ok(())
}

fn validate_optional_i64_percent(value: Option<i64>, path: &Path, field: &str) -> Result<()> {
    if let Some(value) = value
        && !(1..=10).contains(&value)
    {
        bail!("{field} in {} must be between 1 and 10", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-policy-{name}-{}-{nanos:x}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn load_runtime_policy_from_root_reads_versioned_policy_and_resolves_relative_log_dir() {
        clear_runtime_policy_cache();
        let root = temp_root("loads");
        let path = runtime_policy_path(&root);
        fs::write(
            &path,
            r#"
version = 1

[runtime]
log_format = "json"
log_dir = "runtime-logs"

[secrets]
backend = "file"

[runtime_proxy]
worker_count = 12
active_request_limit = 96
profile_inflight_soft_limit = 5
profile_inflight_hard_limit = 9
"#,
        )
        .unwrap();

        let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.runtime.log_format, Some(RuntimeLogFormat::Json));
        assert_eq!(loaded.runtime.log_dir, Some(root.join("runtime-logs")));
        assert_eq!(loaded.secrets.backend, Some(SecretBackendKind::File));
        assert_eq!(loaded.runtime_proxy.worker_count, Some(12));
        assert_eq!(loaded.runtime_proxy.active_request_limit, Some(96));
        assert_eq!(loaded.runtime_proxy.profile_inflight_soft_limit, Some(5));
        assert_eq!(loaded.runtime_proxy.profile_inflight_hard_limit, Some(9));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_runtime_policy_from_root_rejects_unsupported_version() {
        clear_runtime_policy_cache();
        let root = temp_root("version");
        let path = runtime_policy_path(&root);
        fs::write(
            &path,
            r#"
version = 2

[runtime]
log_format = "json"
"#,
        )
        .unwrap();

        let err = load_runtime_policy_from_root(&root).unwrap_err();
        assert!(
            err.to_string()
                .contains("unsupported prodex policy version")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_runtime_policy_from_root_parses_secret_settings() {
        clear_runtime_policy_cache();
        let root = temp_root("secrets");
        let path = runtime_policy_path(&root);
        fs::write(
            &path,
            r#"
version = 1

[secrets]
backend = "keyring"
keyring_service = "prodex"
"#,
        )
        .unwrap();

        let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
        assert_eq!(loaded.secrets.backend, Some(SecretBackendKind::Keyring));
        assert_eq!(loaded.secrets.keyring_service.as_deref(), Some("prodex"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_runtime_policy_from_root_rejects_keyring_backend_without_service() {
        clear_runtime_policy_cache();
        let root = temp_root("secrets-missing-service");
        let path = runtime_policy_path(&root);
        fs::write(
            &path,
            r#"
version = 1

[secrets]
backend = "keyring"
"#,
        )
        .unwrap();

        let err = load_runtime_policy_from_root(&root).unwrap_err();
        assert!(err.to_string().contains("secrets.keyring_service"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_runtime_policy_from_root_rejects_zero_profile_inflight_limits() {
        clear_runtime_policy_cache();
        let root = temp_root("inflight-zero");
        let path = runtime_policy_path(&root);
        fs::write(
            &path,
            r#"
version = 1

[runtime_proxy]
profile_inflight_soft_limit = 0
profile_inflight_hard_limit = 1
"#,
        )
        .unwrap();

        let err = load_runtime_policy_from_root(&root).unwrap_err();
        assert!(
            err.to_string()
                .contains("runtime_proxy.profile_inflight_soft_limit")
        );

        let _ = fs::remove_dir_all(root);
    }
}
