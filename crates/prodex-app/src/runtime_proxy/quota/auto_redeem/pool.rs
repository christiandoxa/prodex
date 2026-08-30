//! Auto-redeem pool probing and candidate selection.

#[cfg(feature = "mojo-quota")]
use super::super::runtime_quota_summary_for_route;
use super::super::{
    RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT, RuntimeRotationProxyShared, RuntimeRouteKind,
    active_profile_selection_order, prune_runtime_profile_selection_backoff,
    runtime_profile_auth_failure_active_from_map, runtime_profile_inflight_soft_limit_for_shared,
    runtime_profile_inflight_sort_key, runtime_profile_name_in_selection_backoff,
    runtime_proxy_log, runtime_proxy_pressure_mode_active_for_route, runtime_route_kind_label,
    schedule_runtime_probe_refresh,
};
#[cfg(feature = "mojo-quota")]
use super::super::{
    RuntimeRotationState, runtime_profile_health_score, runtime_profile_route_circuit_open_until,
};
use crate::ProfileProviderExt;
use anyhow::Result;
use chrono::Local;
#[cfg(feature = "mojo-quota")]
use prodex_runtime_quota::runtime_quota_window_usable_for_auto_rotate;
#[cfg(feature = "mojo-quota")]
use prodex_runtime_store::runtime_profile_transport_backoff_until_from_map;
#[cfg(feature = "mojo-quota")]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub(super) fn refresh_runtime_auto_redeem_pool_missing_quota(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
    context: &str,
) -> Result<bool> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit =
        runtime_profile_inflight_soft_limit_for_shared(shared, route_kind, pressure_mode);
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let jobs = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        active_profile_selection_order(&runtime.state, &runtime.current_profile)
            .into_iter()
            .filter(|name| !excluded_profiles.contains(name))
            .filter(|name| !runtime.profile_probe_cache.contains_key(name))
            .filter_map(|name| {
                let profile = runtime.state.profiles.get(&name)?;
                if !matches!(profile.provider, crate::ProfileProvider::Openai) {
                    return None;
                }
                if runtime_profile_auth_failure_active_from_map(&runtime.profile_health, &name, now)
                {
                    return None;
                }
                if runtime_profile_name_in_selection_backoff(
                    &name,
                    &runtime.profile_retry_backoff_until,
                    &runtime.profile_transport_backoff_until,
                    &runtime.profile_route_circuit_open_until,
                    route_kind,
                    now,
                ) {
                    return None;
                }
                if runtime_profile_inflight_sort_key(&name, &profile_inflight)
                    >= inflight_soft_limit
                {
                    return None;
                }
                profile
                    .provider
                    .auth_summary(&profile.codex_home)
                    .quota_compatible
                    .then(|| (name, profile.codex_home.clone()))
            })
            .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
            .collect::<Vec<_>>()
    };

    let scheduled = !jobs.is_empty();
    if scheduled {
        runtime_proxy_log(
            shared,
            format!(
                "{context}_auto_redeem_pool_probe_scheduled route={} jobs={}",
                runtime_route_kind_label(route_kind),
                jobs.len(),
            ),
        );
        for (profile_name, codex_home) in jobs {
            schedule_runtime_probe_refresh(shared, &profile_name, &codex_home);
        }
    }
    Ok(scheduled)
}

#[cfg(feature = "mojo-quota")]
pub(super) fn runtime_auto_redeem_pool_has_weekly_remaining_profile(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    if !shared.auto_redeem_enabled {
        return Ok(None);
    }
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit =
        runtime_profile_inflight_soft_limit_for_shared(shared, route_kind, pressure_mode);
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;

    Ok(
        active_profile_selection_order(&runtime.state, &runtime.current_profile)
            .into_iter()
            .filter(|name| !excluded_profiles.contains(name))
            .find(|name| {
                runtime_auto_redeem_profile_has_weekly_remaining(
                    &runtime,
                    name,
                    route_kind,
                    now,
                    inflight_soft_limit,
                    &profile_inflight,
                )
            }),
    )
}

#[cfg(feature = "mojo-quota")]
fn runtime_auto_redeem_profile_has_weekly_remaining(
    runtime: &RuntimeRotationState,
    name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
    inflight_soft_limit: usize,
    profile_inflight: &BTreeMap<String, usize>,
) -> bool {
    if !runtime_auto_redeem_profile_is_available(
        runtime,
        name,
        route_kind,
        now,
        inflight_soft_limit,
        profile_inflight,
    ) {
        return false;
    }
    let Some(probe) = runtime.profile_probe_cache.get(name) else {
        return false;
    };
    if !probe.auth.quota_compatible {
        return false;
    }
    let Ok(usage) = probe.result.as_ref() else {
        return false;
    };
    runtime_quota_window_usable_for_auto_rotate(
        runtime_quota_summary_for_route(usage, route_kind)
            .weekly
            .status,
    )
}

#[cfg(feature = "mojo-quota")]
fn runtime_auto_redeem_profile_is_available(
    runtime: &RuntimeRotationState,
    name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
    inflight_soft_limit: usize,
    profile_inflight: &BTreeMap<String, usize>,
) -> bool {
    let Some(profile) = runtime.state.profiles.get(name) else {
        return false;
    };
    if !matches!(profile.provider, crate::ProfileProvider::Openai) {
        return false;
    }
    if runtime_profile_auth_failure_active_from_map(&runtime.profile_health, name, now) {
        return false;
    }
    if runtime_profile_transport_backoff_until_from_map(
        &runtime.profile_transport_backoff_until,
        name,
        route_kind,
        now,
    )
    .is_some()
        || runtime_profile_route_circuit_open_until(runtime, name, route_kind, now).is_some()
    {
        return false;
    }
    runtime_profile_inflight_sort_key(name, profile_inflight) < inflight_soft_limit
}

pub(crate) fn runtime_best_auto_redeem_profile_name(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    #[cfg(not(feature = "mojo-quota"))]
    {
        let _ = (shared, route_kind, excluded_profiles);
        Ok(None)
    }

    #[cfg(feature = "mojo-quota")]
    {
        let now = Local::now().timestamp();
        let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
        let inflight_soft_limit =
            runtime_profile_inflight_soft_limit_for_shared(shared, route_kind, pressure_mode);
        let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let order = active_profile_selection_order(&runtime.state, &runtime.current_profile);

        let mut candidates = Vec::new();
        for (order_index, name) in order.into_iter().enumerate() {
            if excluded_profiles.contains(&name) {
                continue;
            }
            let Some(profile) = runtime.state.profiles.get(&name) else {
                continue;
            };
            if !matches!(profile.provider, crate::ProfileProvider::Openai) {
                continue;
            }
            if runtime_profile_auth_failure_active_from_map(&runtime.profile_health, &name, now) {
                continue;
            }
            if runtime_profile_transport_backoff_until_from_map(
                &runtime.profile_transport_backoff_until,
                &name,
                route_kind,
                now,
            )
            .is_some()
                || runtime_profile_route_circuit_open_until(&runtime, &name, route_kind, now)
                    .is_some()
            {
                continue;
            }
            let inflight_count = runtime_profile_inflight_sort_key(&name, &profile_inflight);
            if inflight_count >= inflight_soft_limit {
                continue;
            }
            let Some(probe) = runtime.profile_probe_cache.get(&name) else {
                continue;
            };
            if !probe.auth.quota_compatible {
                continue;
            }
            let Some(usage) = probe.result.as_ref().ok() else {
                continue;
            };
            let quota_summary = runtime_quota_summary_for_route(usage, route_kind);
            let health_sort_key = runtime_profile_health_score(&runtime, &name, now, route_kind);
            // ponytail: fixed 256-row ABI bound; raise only with a versioned capacity review.
            if candidates.len() >= prodex_mojo_core::runtime::RUNTIME_AUTO_REDEEM_PLAN_MAX_COUNT {
                return Err(anyhow::anyhow!(
                    "auto-redeem candidate count exceeds Mojo planner bound"
                ));
            }
            candidates.push((
                name,
                prodex_mojo_core::runtime::AutoRedeemCandidateInput {
                    plan_type: usage.plan_type.as_deref(),
                    available_count: usage
                        .rate_limit_reset_credits
                        .as_ref()
                        .map(|credits| credits.available_count)
                        .unwrap_or_default(),
                    weekly_status: match quota_summary.weekly.status {
                        prodex_quota::RuntimeQuotaWindowStatus::Ready => 0,
                        prodex_quota::RuntimeQuotaWindowStatus::Thin => 1,
                        prodex_quota::RuntimeQuotaWindowStatus::Critical => 2,
                        prodex_quota::RuntimeQuotaWindowStatus::Exhausted => 3,
                        prodex_quota::RuntimeQuotaWindowStatus::Unknown => 4,
                    },
                    weekly_reset_at: quota_summary.weekly.reset_at,
                    inflight_count: i64::try_from(inflight_count)
                        .map_err(|_| anyhow::anyhow!("auto-redeem in-flight count exceeds ABI"))?,
                    health_sort_key: i64::from(health_sort_key),
                    order_index: i64::try_from(order_index)
                        .map_err(|_| anyhow::anyhow!("auto-redeem order exceeds ABI"))?,
                },
            ));
        }
        let inputs = candidates
            .iter()
            .map(|(_, input)| *input)
            .collect::<Vec<_>>();
        let selected = prodex_mojo_core::runtime::auto_redeem_plan_batch(&inputs, now)
            .map_err(|error| anyhow::anyhow!("Mojo auto-redeem planner failed: {error:?}"))?;
        Ok(selected.map(|index| candidates[index].0.clone()))
    }
}
