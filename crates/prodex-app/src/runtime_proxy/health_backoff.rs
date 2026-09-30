use super::{
    RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS, RUNTIME_PROFILE_TRANSPORT_BACKOFF_MAX_SECONDS,
    RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS, RuntimeProfileBackoffs, RuntimeProfileHealth,
    RuntimeRotationProxyShared, RuntimeRotationState, runtime_profile_auth_failure_active_from_map,
    runtime_profile_cached_auth_summary_from_maps_for_selection,
    runtime_profile_effective_health_score_from_map,
    runtime_profile_quota_summary_for_route_from_state, runtime_profile_route_circuit_health_key,
    runtime_profile_route_circuit_key, runtime_proxy_log, runtime_quota_precommit_guard_reason,
    runtime_route_kind_label, runtime_soften_persisted_route_circuits_for_startup,
    schedule_runtime_state_save_from_runtime,
};
use anyhow::Result;
use chrono::Local;
use prodex_runtime_state::{RuntimeRouteKind, RuntimeStateMutation};
use prodex_runtime_store::{
    runtime_profile_transport_backoff_key, runtime_profile_transport_backoff_until_from_map,
};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) use prodex_runtime_store::{
    runtime_profile_backoff_sort_key, runtime_profile_name_in_selection_backoff,
};

pub(crate) fn runtime_profile_recovery_wait_for_route(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    include_retry_backoff: bool,
) -> Result<Option<i64>> {
    let now = Local::now().timestamp();
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    let mut earliest: Option<i64> = None;
    for profile_name in runtime.state.profiles.keys() {
        let Some(auth) = runtime_profile_cached_auth_summary_from_maps_for_selection(
            profile_name,
            &runtime.profile_usage_auth,
            &runtime.profile_probe_cache,
        ) else {
            continue;
        };
        if !auth.quota_compatible
            || runtime_profile_auth_failure_active_from_map(
                &runtime.profile_health,
                profile_name,
                now,
            )
        {
            continue;
        }
        let (quota_summary, _) = runtime_profile_quota_summary_for_route_from_state(
            &runtime,
            profile_name,
            route_kind,
            now,
        );
        if runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some() {
            continue;
        }
        let retry_until = include_retry_backoff
            .then(|| {
                runtime
                    .profile_retry_backoff_until
                    .get(profile_name)
                    .copied()
                    .filter(|until| *until > now)
            })
            .flatten();
        let transport_until = runtime_profile_transport_backoff_until_from_map(
            &runtime.profile_transport_backoff_until,
            profile_name,
            route_kind,
            now,
        );
        let circuit_until = runtime
            .profile_route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
            .copied()
            .filter(|until| *until > now);
        let recovery_at = prodex_mojo_core::runtime::profile_recovery_at(
            retry_until,
            transport_until,
            circuit_until,
            now,
        )
        .expect("Mojo profile recovery planner returned invalid output");
        if let Some(next) = recovery_at {
            earliest = Some(earliest.map_or(next, |current| current.min(next)));
        }
    }
    Ok(earliest)
}

pub(crate) fn clear_runtime_recovered_profiles(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &mut BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    include_retry_backoff: bool,
) -> Result<usize> {
    let now = Local::now().timestamp();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let before = excluded_profiles.len();
    excluded_profiles.retain(|profile_name| {
        let Some(auth) = runtime_profile_cached_auth_summary_from_maps_for_selection(
            profile_name,
            &runtime.profile_usage_auth,
            &runtime.profile_probe_cache,
        ) else {
            return true;
        };
        if !auth.quota_compatible
            || runtime_profile_auth_failure_active_from_map(
                &runtime.profile_health,
                profile_name,
                now,
            )
        {
            return true;
        }
        let (quota_summary, _) = runtime_profile_quota_summary_for_route_from_state(
            &runtime,
            profile_name,
            route_kind,
            now,
        );
        if runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some() {
            return true;
        }
        let retry_until = include_retry_backoff
            .then(|| {
                runtime
                    .profile_retry_backoff_until
                    .get(profile_name)
                    .copied()
            })
            .flatten();
        let transport_until = runtime_profile_transport_backoff_until_from_map(
            &runtime.profile_transport_backoff_until,
            profile_name,
            route_kind,
            now,
        );
        let circuit_until = runtime
            .profile_route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
            .copied();
        prodex_mojo_core::runtime::profile_selection_backoff_active(
            retry_until,
            transport_until,
            circuit_until,
            now,
        )
        .expect("Mojo selection-backoff policy returned invalid output")
    });
    Ok(before.saturating_sub(excluded_profiles.len()))
}

pub(crate) fn prune_runtime_profile_retry_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    runtime.profile_retry_backoff_until.retain(|_, until| {
        prodex_mojo_core::runtime::profile_backoff_should_retain(true, *until, now)
            .expect("Mojo retry-backoff retention returned invalid output")
    });
}

pub(crate) fn prune_runtime_profile_transport_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    runtime.profile_transport_backoff_until.retain(|_, until| {
        prodex_mojo_core::runtime::profile_backoff_should_retain(true, *until, now)
            .expect("Mojo transport-backoff retention returned invalid output")
    });
}

pub(crate) fn prune_runtime_profile_route_circuits(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_route_circuit_open_until
        .retain(|key, until| {
            let health_key = runtime_profile_route_circuit_health_key(key);
            let health_score = runtime_profile_effective_health_score_from_map(
                &runtime.profile_health,
                &health_key,
                now,
            );
            prodex_mojo_core::runtime::profile_route_circuit_should_retain(
                *until,
                now,
                health_score,
            )
            .expect("Mojo route-circuit retention returned invalid output")
        });
}

pub(crate) fn prune_runtime_profile_selection_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    prune_runtime_profile_retry_backoff(runtime, now);
    prune_runtime_profile_transport_backoff(runtime, now);
    prune_runtime_profile_route_circuits(runtime, now);
}

pub(crate) fn runtime_profile_backoffs_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeProfileBackoffs {
    RuntimeProfileBackoffs {
        retry_backoff_until: runtime.profile_retry_backoff_until.clone(),
        transport_backoff_until: runtime.profile_transport_backoff_until.clone(),
        route_circuit_open_until: runtime.profile_route_circuit_open_until.clone(),
        updated_at: runtime.profile_backoff_updated_at.clone(),
    }
}

pub(crate) fn runtime_soften_persisted_backoffs_for_startup(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = prodex_runtime_store::runtime_soften_persisted_backoff_map_for_startup(
        &mut backoffs.transport_backoff_until,
        now,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

pub(crate) fn mark_runtime_profile_retry_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<()> {
    mark_runtime_profile_retry_backoff_for_delay(shared, profile_name, None)
}

pub(crate) fn mark_runtime_profile_retry_backoff_for_delay(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    retry_after: Option<std::time::Duration>,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let retry_after_seconds = retry_after
        .map(|delay| delay.as_millis().saturating_add(999) / 1_000)
        .and_then(|seconds| i64::try_from(seconds).ok())
        .unwrap_or_default();
    let plan = prodex_mojo_core::runtime::profile_retry_backoff_plan(
        retry_after_seconds,
        RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS,
        now,
    )
    .expect("Mojo retry-backoff planner returned invalid output");
    let until = plan.until;
    runtime
        .profile_retry_backoff_until
        .insert(profile_name.to_string(), until);
    runtime.profile_backoff_updated_at.insert(
        prodex_runtime_store::runtime_profile_retry_backoff_update_key(profile_name),
        Local::now().timestamp_millis(),
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        RuntimeStateMutation::ProfileRetryBackoff(profile_name.to_string()),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_retry_backoff",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("until", until.to_string()),
                runtime_proxy_log_field(
                    "retry_after_ms",
                    retry_after.map_or(0, |delay| delay.as_millis()).to_string(),
                ),
            ],
        ),
    );
    Ok(())
}

pub(crate) fn mark_runtime_profile_transport_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    let existing_remaining = runtime_profile_transport_backoff_until_from_map(
        &runtime.profile_transport_backoff_until,
        profile_name,
        route_kind,
        now,
    )
    .unwrap_or(now)
    .saturating_sub(now);
    let plan = prodex_mojo_core::runtime::profile_transport_backoff_plan(
        existing_remaining,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_MAX_SECONDS,
        now,
    )
    .expect("Mojo transport-backoff planner returned invalid output");
    let next_backoff_seconds = plan.seconds;
    let until = plan.until;
    runtime
        .profile_transport_backoff_until
        .entry(route_key.clone())
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    runtime.profile_backoff_updated_at.insert(
        prodex_runtime_store::runtime_profile_transport_backoff_update_key(&route_key),
        Local::now().timestamp_millis(),
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        RuntimeStateMutation::ProfileTransportBackoff(format!(
            "{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        )),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_transport_backoff",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("until", until.to_string()),
                runtime_proxy_log_field("seconds", next_backoff_seconds.to_string()),
                runtime_proxy_log_field("context", context),
            ],
        ),
    );
    Ok(())
}

pub(crate) fn clear_runtime_profile_transport_backoff_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> bool {
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    let mut changed = runtime
        .profile_transport_backoff_until
        .remove(&route_key)
        .is_some();
    if changed {
        runtime.profile_backoff_updated_at.insert(
            prodex_runtime_store::runtime_profile_transport_backoff_update_key(&route_key),
            Local::now().timestamp_millis(),
        );
    }
    let cleared_legacy = runtime
        .profile_transport_backoff_until
        .remove(profile_name)
        .is_some();
    if cleared_legacy {
        runtime.profile_backoff_updated_at.insert(
            prodex_runtime_store::runtime_profile_transport_backoff_update_key(profile_name),
            Local::now().timestamp_millis(),
        );
    }
    changed = cleared_legacy || changed;
    changed
}

pub(crate) fn mark_runtime_profile_retry_backoff_update(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
) {
    runtime.profile_backoff_updated_at.insert(
        prodex_runtime_store::runtime_profile_retry_backoff_update_key(profile_name),
        Local::now().timestamp_millis(),
    );
}

pub(crate) fn mark_runtime_profile_route_circuit_update(
    runtime: &mut RuntimeRotationState,
    route_key: &str,
) {
    runtime.profile_backoff_updated_at.insert(
        prodex_runtime_store::runtime_profile_route_circuit_update_key(route_key),
        Local::now().timestamp_millis(),
    );
}
