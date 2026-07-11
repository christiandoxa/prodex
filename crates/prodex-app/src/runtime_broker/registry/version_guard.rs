use std::env;

use super::runtime_broker_observed_binary_identity;
use crate::{
    AppPaths, RuntimeBrokerHealth, RuntimeBrokerRegistry, RuntimeBrokerVersionGuardOutcome,
    audit_log_event_best_effort, cleanup_runtime_broker_stale_leases,
    remove_runtime_broker_registry_if_instance_matches, runtime_current_prodex_binary_identity,
    runtime_current_prodex_version_identity, runtime_process_pid_alive, terminate_runtime_process,
};

pub(crate) fn replace_runtime_broker_if_version_mismatch_with_health(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
) -> RuntimeBrokerVersionGuardOutcome {
    if !runtime_process_pid_alive(registry.pid) {
        return RuntimeBrokerVersionGuardOutcome::Compatible;
    }

    let observed_identity = runtime_broker_observed_binary_identity(registry, health);
    let active_requests = health
        .filter(|health| health.matches_registry_instance(registry))
        .map(|health| health.active_requests)
        .unwrap_or_default();
    let live_leases = cleanup_runtime_broker_stale_leases(paths, broker_key);
    let current_version_identity = runtime_current_prodex_version_identity();
    let current_binary_identity;
    let observed_version_mismatch = prodex_runtime_broker::runtime_broker_observed_version_mismatch(
        &current_version_identity,
        &observed_identity,
    );
    let current_identity_for_decision = if observed_version_mismatch {
        &current_version_identity
    } else {
        current_binary_identity = runtime_current_prodex_binary_identity();
        &current_binary_identity
    };
    let decision = prodex_runtime_broker::runtime_broker_version_guard_decision(
        true,
        current_identity_for_decision,
        &current_version_identity,
        &observed_identity,
        active_requests,
        live_leases,
    );
    if decision.outcome != RuntimeBrokerVersionGuardOutcome::Replaced {
        return decision.outcome;
    }
    let current_identity = decision.current_identity;
    let replacement_reason = decision.replacement_reason.unwrap_or_else(|| {
        prodex_runtime_broker::runtime_broker_replacement_reason(
            &current_identity,
            &observed_identity,
        )
    });
    audit_log_event_best_effort(
        "runtime_broker",
        "replace_stale_broker",
        "success",
        serde_json::json!({
            "reason": replacement_reason,
            "broker_key": broker_key,
            "pid": registry.pid,
            "listen_addr": registry.listen_addr,
            "started_at": registry.started_at,
            "instance_id": registry.instance_id,
            "upstream_base_url": registry.upstream_base_url,
            "include_code_review": registry.include_code_review,
            "current_prodex_version": current_identity.prodex_version,
            "current_executable_path": current_identity
                .executable_path
                .map(|path| path.display().to_string()),
            "current_executable_sha256": current_identity.executable_sha256,
            "detected_prodex_version": observed_identity.prodex_version,
            "detected_executable_sha256": observed_identity.executable_sha256,
            "executable_path": observed_identity
                .executable_path
                .map(|path| path.display().to_string()),
            "active_requests": health.map(|health| health.active_requests),
            "platform": env::consts::OS,
        }),
    );
    terminate_runtime_process(registry.pid);
    remove_runtime_broker_registry_if_instance_matches(paths, broker_key, &registry.instance_id);
    RuntimeBrokerVersionGuardOutcome::Replaced
}
