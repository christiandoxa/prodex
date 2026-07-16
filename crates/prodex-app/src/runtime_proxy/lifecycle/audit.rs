//! Runtime proxy startup state audit.

use super::*;

pub(crate) fn audit_runtime_proxy_startup_state(shared: &RuntimeRotationProxyShared) {
    let Ok(mut runtime) = shared.runtime.lock() else {
        return;
    };
    let now = Local::now().timestamp();
    let orphan_managed_dirs = collect_orphan_managed_profile_dirs(&runtime.paths, &runtime.state);
    let missing_managed_dirs = runtime
        .state
        .profiles
        .values()
        .filter(|profile| profile.managed && !profile.codex_home.exists())
        .count();
    let valid_profiles = runtime
        .state
        .profiles
        .iter()
        .filter(|(_, profile)| !profile.managed || profile.codex_home.exists())
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    let stale_response_bindings = runtime
        .state
        .response_profile_bindings
        .values()
        .filter(|binding| !valid_profiles.contains(&binding.profile_name))
        .count();
    let stale_session_bindings = runtime
        .state
        .session_profile_bindings
        .values()
        .filter(|binding| !valid_profiles.contains(&binding.profile_name))
        .count();
    let stale_probe_cache = runtime
        .profile_probe_cache
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_usage_snapshots = runtime
        .profile_usage_snapshots
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_retry_backoffs = runtime
        .profile_retry_backoff_until
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_transport_backoffs = runtime
        .profile_transport_backoff_until
        .keys()
        .filter(|key| !runtime_profile_transport_backoff_key_valid(key, &valid_profiles))
        .count();
    let stale_route_circuits = runtime
        .profile_route_circuit_open_until
        .keys()
        .filter(|key| !valid_profiles.contains(runtime_profile_route_circuit_profile_name(key)))
        .count();
    let stale_health_scores = runtime
        .profile_health
        .keys()
        .filter(|key| !valid_profiles.contains(runtime_profile_score_profile_name(key)))
        .count();
    let active_profile_missing_dir = runtime
        .state
        .active_profile
        .as_deref()
        .and_then(|name| runtime.state.profiles.get(name))
        .is_some_and(|profile| profile.managed && !profile.codex_home.exists());

    runtime
        .state
        .response_profile_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .state
        .session_profile_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .turn_state_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .session_id_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .profile_probe_cache
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_usage_snapshots
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_retry_backoff_until
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_transport_backoff_until
        .retain(|key, _| runtime_profile_transport_backoff_key_valid(key, &valid_profiles));
    runtime
        .profile_route_circuit_open_until
        .retain(|key, _| valid_profiles.contains(runtime_profile_route_circuit_profile_name(key)));
    runtime
        .profile_health
        .retain(|key, _| valid_profiles.contains(runtime_profile_score_profile_name(key)));
    let route_circuit_count_after_profile_prune = runtime.profile_route_circuit_open_until.len();
    prune_runtime_profile_route_circuits(&mut runtime, now);
    let expired_route_circuits = route_circuit_count_after_profile_prune
        .saturating_sub(runtime.profile_route_circuit_open_until.len());
    let changed = stale_response_bindings > 0
        || stale_session_bindings > 0
        || stale_probe_cache > 0
        || stale_usage_snapshots > 0
        || stale_retry_backoffs > 0
        || stale_transport_backoffs > 0
        || stale_route_circuits > 0
        || expired_route_circuits > 0
        || stale_health_scores > 0;
    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_startup_audit missing_managed_dirs={missing_managed_dirs} orphan_managed_dirs={} stale_response_bindings={stale_response_bindings} stale_session_bindings={stale_session_bindings} stale_probe_cache={stale_probe_cache} stale_usage_snapshots={stale_usage_snapshots} stale_retry_backoffs={stale_retry_backoffs} stale_transport_backoffs={stale_transport_backoffs} stale_route_circuits={stale_route_circuits} expired_route_circuits={expired_route_circuits} stale_health_scores={stale_health_scores} active_profile_missing_dir={active_profile_missing_dir}",
            orphan_managed_dirs.len(),
        ),
    );
    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            RuntimeStateMutation::StartupAudit,
        );
    }
    drop(runtime);
}
