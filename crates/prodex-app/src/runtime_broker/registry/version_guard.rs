use std::env;
use std::path::Path;

use anyhow::Result;

use super::runtime_broker_observed_binary_identity;
use crate::{
    AppPaths, RuntimeBrokerHealth, RuntimeBrokerRegistry, RuntimeBrokerVersionGuardOutcome,
    RuntimeProdexBinaryIdentity, audit_log_event, cleanup_runtime_broker_stale_leases,
    remove_runtime_broker_registry_if_instance_matches, runtime_current_prodex_binary_identity,
    runtime_current_prodex_version_identity, runtime_process_absence_proven,
    terminate_runtime_process,
};

pub(crate) fn replace_runtime_broker_if_version_mismatch_with_health(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
) -> Result<RuntimeBrokerVersionGuardOutcome> {
    let process_alive = !runtime_process_absence_proven(registry.pid);
    let observed_identity = if process_alive {
        runtime_broker_observed_binary_identity(registry, health)
    } else {
        RuntimeProdexBinaryIdentity::default()
    };
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
        process_alive,
        current_identity_for_decision,
        &current_version_identity,
        &observed_identity,
        active_requests,
        live_leases,
    );
    if decision.outcome != RuntimeBrokerVersionGuardOutcome::Replaced {
        return Ok(decision.outcome);
    }
    let current_identity = decision.current_identity;
    let replacement_reason = decision.replacement_reason.unwrap_or_else(|| {
        prodex_runtime_broker::runtime_broker_replacement_reason(
            &current_identity,
            &observed_identity,
        )
    });
    let termination = terminate_runtime_process(
        registry.pid,
        registry.process_birth_identity.as_deref(),
        registry.executable_path.as_deref().map(Path::new),
    );
    let termination_code = match termination {
        super::process::RuntimeProcessTerminationOutcome::NotRunning => 0,
        super::process::RuntimeProcessTerminationOutcome::OwnershipUnproven => 1,
        super::process::RuntimeProcessTerminationOutcome::OwnershipChanged => 2,
        super::process::RuntimeProcessTerminationOutcome::Terminated => 3,
        super::process::RuntimeProcessTerminationOutcome::StillRunning => 4,
    };
    let termination_plan =
        prodex_runtime_broker::runtime_broker_termination_outcome_plan(termination_code)
            .expect("Mojo runtime broker termination outcome policy returned invalid output");
    if termination_plan == prodex_runtime_broker::BrokerTerminationOutcomePlan::Failure {
        return Ok(RuntimeBrokerVersionGuardOutcome::TerminationFailed);
    }
    let discard_stale =
        termination_plan == prodex_runtime_broker::BrokerTerminationOutcomePlan::DiscardStale;
    audit_log_event(
        "runtime_broker",
        if discard_stale {
            "discard_stale_registry"
        } else {
            "replace_stale_broker"
        },
        "success",
        serde_json::json!({
            "reason": if discard_stale { "process_identity_mismatch" } else { replacement_reason },
            "broker_key": broker_key,
            "pid": registry.pid,
            "termination_outcome": format!("{termination:?}"),
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
    )?;
    remove_runtime_broker_registry_if_instance_matches(paths, broker_key, &registry.instance_id);
    Ok(RuntimeBrokerVersionGuardOutcome::Replaced)
}
