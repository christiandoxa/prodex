use anyhow::Result;
use chrono::Local;
use prodex_runtime_broker::{RuntimeBrokerBootstrap, read_runtime_broker_bootstrap};
use redaction::redaction_redact_secret_like_text;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::{
    AppPaths, AppState, AppStateIoExt, RUNTIME_BROKER_IDLE_GRACE_SECONDS,
    RUNTIME_BROKER_LEASE_SCAN_INTERVAL_MS, RUNTIME_BROKER_POLL_INTERVAL_MS,
    RUNTIME_PROXY_OPENAI_MOUNT_PATH, RuntimeBrokerArgs, RuntimeBrokerMetadata,
    RuntimeBrokerRegistry, RuntimeRotationProxy, RuntimeRotationProxyStartOptions,
    audit_log_event_best_effort, cleanup_runtime_broker_stale_leases,
    register_runtime_broker_metadata, register_runtime_proxy_persistence_mode,
    remove_runtime_broker_capability_if_matches,
    remove_runtime_broker_registry_if_instance_matches, runtime_broker_startup_grace_seconds,
    runtime_current_prodex_version_identity, runtime_proxy_log_to_path,
    runtime_upstream_proxy_mode_label, save_runtime_broker_capability,
    save_runtime_broker_registry, start_runtime_rotation_proxy_with_options,
    try_acquire_runtime_owner_lock,
};

pub(crate) fn handle_runtime_broker(_args: RuntimeBrokerArgs) -> Result<()> {
    let bootstrap = read_runtime_broker_bootstrap(std::io::stdin().lock())?;
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let mut proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths: &paths,
        state: &state,
        current_profile: &bootstrap.current_profile,
        upstream_base_url: bootstrap.upstream_base_url.clone(),
        include_code_review: bootstrap.include_code_review,
        upstream_no_proxy: bootstrap.upstream_no_proxy,
        auto_redeem: false,
        smart_context_enabled: bootstrap.smart_context_enabled,
        presidio_redaction_enabled: false,
        model_context_window_tokens: bootstrap.model_context_window_tokens,
        preferred_listen_addr: bootstrap.listen_addr.as_deref(),
    })?;
    if proxy.owner_lock.is_none() {
        runtime_proxy_log_to_path(
            &proxy.log_path,
            "runtime_broker_persistence_follower reason=owner_lock_busy",
        );
    }

    let metadata = runtime_broker_publish_start(&paths, &bootstrap, &proxy)?;

    let startup_grace_until =
        metadata
            .started_at
            .saturating_add(runtime_broker_startup_grace_seconds(
                proxy.runtime_config.broker_ready_timeout_ms,
            ));
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    let lease_scan_interval = Duration::from_millis(
        RUNTIME_BROKER_LEASE_SCAN_INTERVAL_MS.max(RUNTIME_BROKER_POLL_INTERVAL_MS),
    );
    let mut idle_started_at = None::<i64>;
    let mut cached_live_leases = 0usize;
    let mut last_lease_scan_at = Instant::now() - lease_scan_interval;
    let mut last_owner_promotion_attempt_at = Instant::now() - lease_scan_interval;
    loop {
        let active_requests = proxy.active_request_count.load(Ordering::SeqCst);
        if proxy.owner_lock.is_none()
            && last_owner_promotion_attempt_at.elapsed() >= lease_scan_interval
        {
            runtime_broker_try_promote_persistence_owner(&paths, &mut proxy);
            last_owner_promotion_attempt_at = Instant::now();
        }
        if active_requests == 0 && last_lease_scan_at.elapsed() >= lease_scan_interval {
            cached_live_leases = cleanup_runtime_broker_stale_leases(&paths, &bootstrap.broker_key);
            last_lease_scan_at = Instant::now();
        }
        if cached_live_leases > 0 || active_requests > 0 {
            idle_started_at = None;
        } else {
            let now = Local::now().timestamp();
            if now < startup_grace_until {
                idle_started_at = None;
                thread::sleep(poll_interval);
                continue;
            }
            let idle_since = idle_started_at.get_or_insert(now);
            if now.saturating_sub(*idle_since) >= RUNTIME_BROKER_IDLE_GRACE_SECONDS {
                runtime_proxy_log_to_path(
                    &proxy.log_path,
                    &format!(
                        "runtime_broker_idle_shutdown broker_key={} idle_seconds={}",
                        bootstrap.broker_key,
                        now.saturating_sub(*idle_since)
                    ),
                );
                break;
            }
        }
        thread::sleep(poll_interval);
    }

    drop(proxy);
    remove_runtime_broker_registry_if_instance_matches(
        &paths,
        &bootstrap.broker_key,
        &bootstrap.instance_id,
    );
    remove_runtime_broker_capability_if_matches(
        &paths,
        &bootstrap.broker_key,
        &bootstrap.instance_id,
        &bootstrap.admin_token,
    );
    Ok(())
}

pub(crate) fn runtime_broker_publish_start(
    paths: &AppPaths,
    bootstrap: &RuntimeBrokerBootstrap,
    proxy: &RuntimeRotationProxy,
) -> Result<RuntimeBrokerMetadata> {
    let current_identity = runtime_current_prodex_version_identity();
    let metadata = RuntimeBrokerMetadata {
        broker_key: bootstrap.broker_key.clone(),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        current_profile: bootstrap.current_profile.clone(),
        include_code_review: bootstrap.include_code_review,
        upstream_no_proxy: bootstrap.upstream_no_proxy,
        instance_id: bootstrap.instance_id.clone(),
        admin_token: bootstrap.admin_token.clone(),
        prodex_version: current_identity.prodex_version.clone(),
        executable_path: current_identity
            .executable_path
            .as_ref()
            .map(|path| path.display().to_string()),
        executable_sha256: current_identity.executable_sha256.clone(),
    };
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: metadata.started_at,
        upstream_base_url: bootstrap.upstream_base_url.clone(),
        include_code_review: bootstrap.include_code_review,
        upstream_no_proxy: bootstrap.upstream_no_proxy,
        smart_context_enabled: bootstrap.smart_context_enabled,
        current_profile: bootstrap.current_profile.clone(),
        instance_id: bootstrap.instance_id.clone(),
        prodex_version: current_identity.prodex_version.clone(),
        executable_path: current_identity
            .executable_path
            .as_ref()
            .map(|path| path.display().to_string()),
        executable_sha256: current_identity.executable_sha256.clone(),
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(paths, &bootstrap.broker_key, &registry)?;
    if let Err(err) = save_runtime_broker_capability(
        paths,
        &bootstrap.broker_key,
        &bootstrap.instance_id,
        &bootstrap.admin_token,
    ) {
        remove_runtime_broker_registry_if_instance_matches(
            paths,
            &bootstrap.broker_key,
            &bootstrap.instance_id,
        );
        return Err(err);
    }
    register_runtime_broker_metadata(&proxy.log_path, metadata.clone());
    runtime_proxy_log_to_path(
        &proxy.log_path,
        &format!(
            "runtime_broker_started listen_addr={} broker_key={} current_profile={} include_code_review={} smart_context_enabled={} upstream_proxy_mode={} prodex_version={} executable_path={} executable_sha256={}",
            proxy.listen_addr,
            bootstrap.broker_key,
            bootstrap.current_profile,
            bootstrap.include_code_review,
            bootstrap.smart_context_enabled,
            runtime_upstream_proxy_mode_label(bootstrap.upstream_no_proxy),
            metadata.prodex_version.as_deref().unwrap_or("-"),
            metadata.executable_path.as_deref().unwrap_or("-"),
            metadata.executable_sha256.as_deref().unwrap_or("-")
        ),
    );
    audit_log_event_best_effort(
        "runtime_broker",
        "start",
        "success",
        serde_json::json!({
            "broker_key": bootstrap.broker_key,
            "listen_addr": proxy.listen_addr.to_string(),
            "current_profile": bootstrap.current_profile,
            "include_code_review": bootstrap.include_code_review,
            "smart_context_enabled": bootstrap.smart_context_enabled,
            "upstream_proxy_mode": runtime_upstream_proxy_mode_label(bootstrap.upstream_no_proxy),
            "upstream_base_url": bootstrap.upstream_base_url,
            "prodex_version": metadata.prodex_version,
            "executable_path": metadata.executable_path,
            "executable_sha256": metadata.executable_sha256,
        }),
    );
    Ok(metadata)
}

fn runtime_broker_try_promote_persistence_owner(
    paths: &AppPaths,
    proxy: &mut RuntimeRotationProxy,
) {
    if proxy.owner_lock.is_some() {
        return;
    }
    match try_acquire_runtime_owner_lock(paths) {
        Ok(Some(owner_lock)) => {
            proxy.owner_lock = Some(owner_lock);
            register_runtime_proxy_persistence_mode(&proxy.log_path, true);
            runtime_proxy_log_to_path(
                &proxy.log_path,
                "runtime_broker_persistence_promoted role=owner",
            );
        }
        Ok(None) => {}
        Err(err) => runtime_proxy_log_to_path(
            &proxy.log_path,
            &runtime_broker_persistence_promotion_error_log(&err),
        ),
    }
}

fn runtime_broker_persistence_promotion_error_log(err: &anyhow::Error) -> String {
    format!(
        "runtime_broker_persistence_promotion_error error={}",
        redaction_redact_secret_like_text(&format!("{err:#}"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_persistence_promotion_error_log_redacts_secret_like_chain() {
        let err = anyhow::anyhow!("failed: Authorization: Bearer broker-owner-token")
            .context("owner promotion failed");

        let message = runtime_broker_persistence_promotion_error_log(&err);

        assert!(message.contains("runtime_broker_persistence_promotion_error"));
        assert!(message.contains("owner promotion failed"));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(!message.contains("broker-owner-token"));
    }
}
