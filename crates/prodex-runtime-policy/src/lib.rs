mod cache;
mod load;
mod paths;
mod types;

pub use self::cache::{
    RuntimePolicyCacheInvalidationPlan, clear_runtime_policy_cache,
    invalidate_runtime_policy_cache_for, plan_runtime_policy_cache_invalidation,
};
pub use self::load::{
    load_runtime_policy_cached, load_runtime_policy_from_root, reload_runtime_policy_cached,
    reload_runtime_policy_cached_with_invalidation,
};
pub use self::paths::{resolve_runtime_policy_path, runtime_policy_path};
pub use self::types::{
    PRODEX_POLICY_FILE_NAME, PRODEX_POLICY_VERSION, PRODEX_RUNTIME_PROXY_PRESET_ENV,
    RuntimeLogFormat, RuntimePolicyConfig, RuntimePolicyFile, RuntimePolicyProxyPreset,
    RuntimePolicyProxyPresetSelection, RuntimePolicyProxySettings, RuntimePolicyRuntimeFile,
    RuntimePolicyRuntimeSettings, RuntimePolicySecretsFile, RuntimePolicySecretsSettings,
    RuntimePolicySummary,
};

use anyhow::Result;
use prodex_core::AppPaths;

pub fn ensure_runtime_policy_valid() -> Result<()> {
    if runtime_policy_enabled_for_current_process() {
        let paths = AppPaths::discover()?;
        let _ = load_runtime_policy_cached(&paths.root)?;
    }
    Ok(())
}

pub fn runtime_policy_summary() -> Result<Option<RuntimePolicySummary>> {
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

pub fn runtime_policy_runtime() -> Option<RuntimePolicyRuntimeSettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.runtime)
}

pub fn runtime_policy_proxy() -> Option<RuntimePolicyProxySettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    runtime_policy_proxy_from_root(&paths.root, runtime_proxy_preset_from_env())
}

fn runtime_policy_proxy_from_root(
    root: &std::path::Path,
    env_preset: Option<RuntimePolicyProxyPreset>,
) -> Option<RuntimePolicyProxySettings> {
    if let Some(config) = load_runtime_policy_cached(root).ok().flatten() {
        return Some(config.runtime_proxy.with_effective_preset(env_preset));
    }
    env_preset
        .map(|preset| RuntimePolicyProxySettings::default().with_effective_preset(Some(preset)))
}

pub fn runtime_proxy_preset_from_env() -> Option<RuntimePolicyProxyPreset> {
    std::env::var(PRODEX_RUNTIME_PROXY_PRESET_ENV)
        .ok()
        .and_then(|value| RuntimePolicyProxyPreset::parse(&value))
}

pub fn runtime_policy_secrets() -> Option<RuntimePolicySecretsSettings> {
    if !runtime_policy_enabled_for_current_process() {
        return None;
    }
    let paths = AppPaths::discover().ok()?;
    load_runtime_policy_cached(&paths.root)
        .ok()
        .flatten()
        .map(|config| config.secrets)
}

#[cfg(test)]
fn runtime_policy_enabled_for_current_process() -> bool {
    std::env::var_os("PRODEX_HOME").is_some()
}

#[cfg(not(test))]
fn runtime_policy_enabled_for_current_process() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    struct TestPolicyRoot(PathBuf);

    impl TestPolicyRoot {
        fn new(policy: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "prodex-runtime-policy-{}-{}",
                std::process::id(),
                NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).expect("create isolated runtime-policy test root");
            fs::write(runtime_policy_path(&root), policy)
                .expect("write runtime-policy test config");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestPolicyRoot {
        fn drop(&mut self) {
            invalidate_runtime_policy_cache_for(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn runtime_policy_proxy_caller_uses_mojo_preset_plan() {
        let root = TestPolicyRoot::new(
            r#"
version = 1

[runtime_proxy]
preset = "low"
worker_count = 7
http_connect_timeout_ms = 0
responses_critical_floor_percent = -5
"#,
        );

        let effective = runtime_policy_proxy_from_root(
            root.path(),
            Some(RuntimePolicyProxyPreset::ManyTerminals),
        )
        .expect("configured runtime policy should resolve");

        assert_eq!(
            effective,
            RuntimePolicyProxySettings {
                preset: RuntimePolicyProxyPresetSelection::selected(
                    RuntimePolicyProxyPreset::ManyTerminals,
                ),
                worker_count: Some(7),
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
                http_connect_timeout_ms: Some(0),
                websocket_connect_worker_count: Some(12),
                websocket_connect_queue_capacity: Some(96),
                websocket_connect_overflow_capacity: Some(384),
                websocket_dns_worker_count: Some(6),
                websocket_dns_queue_capacity: Some(48),
                websocket_dns_overflow_capacity: Some(96),
                responses_critical_floor_percent: Some(-5),
                startup_sync_probe_warm_limit: Some(2),
                ..RuntimePolicyProxySettings::default()
            }
        );
    }
}
