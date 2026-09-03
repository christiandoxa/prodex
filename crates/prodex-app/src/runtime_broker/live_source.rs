use crate::{
    AppPaths, RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX, RUNTIME_PROXY_OPENAI_MOUNT_PATH,
    RuntimeBrokerMetadata, RuntimeBrokerRegistry, RuntimeRotationProxy,
    register_runtime_broker_metadata, remove_runtime_broker_registry_if_instance_matches,
    runtime_current_prodex_binary_identity, runtime_process_birth_identity, runtime_random_token,
    save_runtime_broker_artifacts,
};
use anyhow::{Context, Result};
use chrono::Local;
use std::sync::Arc;

pub(crate) struct RuntimeLiveLogSourceGuard {
    paths: AppPaths,
    source_key: String,
    instance_id: String,
}

impl Drop for RuntimeLiveLogSourceGuard {
    fn drop(&mut self) {
        remove_runtime_broker_registry_if_instance_matches(
            &self.paths,
            &self.source_key,
            &self.instance_id,
        );
    }
}

pub(crate) fn publish_runtime_live_log_source(
    paths: &AppPaths,
    proxy: &RuntimeRotationProxy,
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    smart_context_enabled: bool,
) -> Result<RuntimeLiveLogSourceGuard> {
    let instance_id = runtime_random_token("runtime")?;
    let source_key = format!("{RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX}{instance_id}");
    let admin_token = Arc::new(
        prodex_runtime_broker::RuntimeBrokerSecret::new(runtime_random_token("admin")?)
            .context("failed to create direct runtime log capability")?,
    );
    let identity = runtime_current_prodex_binary_identity();
    let started_at = Local::now().timestamp();
    let metadata = RuntimeBrokerMetadata {
        broker_key: source_key.clone(),
        listen_addr: proxy.listen_addr.to_string(),
        started_at,
        current_profile: current_profile.to_string(),
        include_code_review,
        upstream_no_proxy,
        instance_id: instance_id.clone(),
        admin_token: Arc::clone(&admin_token),
        prodex_version: identity.prodex_version.clone(),
        executable_path: identity
            .executable_path
            .as_ref()
            .map(|path| path.display().to_string()),
        executable_sha256: identity.executable_sha256.clone(),
    };
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        process_birth_identity: runtime_process_birth_identity(std::process::id()),
        listen_addr: proxy.listen_addr.to_string(),
        started_at,
        upstream_base_url: upstream_base_url.to_string(),
        include_code_review,
        upstream_no_proxy,
        smart_context_enabled,
        current_profile: current_profile.to_string(),
        instance_id: instance_id.clone(),
        prodex_version: identity.prodex_version,
        executable_path: identity
            .executable_path
            .as_ref()
            .map(|path| path.display().to_string()),
        executable_sha256: identity.executable_sha256,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        realtime_ws_addr: proxy.realtime_ws_sidecar_addr.map(|addr| addr.to_string()),
    };
    save_runtime_broker_artifacts(
        paths,
        &source_key,
        &instance_id,
        admin_token.as_ref(),
        &registry,
    )?;
    register_runtime_broker_metadata(&proxy.log_path, metadata);
    Ok(RuntimeLiveLogSourceGuard {
        paths: paths.clone(),
        source_key,
        instance_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        runtime_broker_capability_file_path, runtime_broker_registry_file_path,
        save_runtime_broker_artifacts,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn live_source_guard_removes_private_registry_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "prodex-live-source-cleanup-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let paths = AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
            root,
        };
        let source_key = "live-source".to_string();
        let instance_id = "instance".to_string();
        let registry = RuntimeBrokerRegistry {
            pid: 1,
            process_birth_identity: None,
            listen_addr: "127.0.0.1:1".to_string(),
            started_at: 100,
            upstream_base_url: "https://upstream.example".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: true,
            current_profile: "main".to_string(),
            instance_id: instance_id.clone(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
            realtime_ws_addr: None,
        };
        let secret = prodex_runtime_broker::RuntimeBrokerSecret::new("capability").unwrap();
        save_runtime_broker_artifacts(&paths, &source_key, &instance_id, &secret, &registry)
            .unwrap();
        let guard = RuntimeLiveLogSourceGuard {
            paths: paths.clone(),
            source_key: source_key.clone(),
            instance_id,
        };
        drop(guard);
        assert!(!runtime_broker_registry_file_path(&paths, &source_key).exists());
        assert!(!runtime_broker_capability_file_path(&paths, &source_key).exists());
        fs::remove_dir_all(paths.root).unwrap();
    }
}
