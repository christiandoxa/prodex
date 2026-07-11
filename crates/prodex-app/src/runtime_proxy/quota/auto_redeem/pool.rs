//! Auto-redeem pool probing and candidate selection.

use super::super::{
    RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT, RuntimeRotationProxyShared, RuntimeRouteKind,
    active_profile_selection_order, prune_runtime_profile_selection_backoff,
    run_runtime_probe_jobs_inline, runtime_profile_auth_failure_active_with_auth_cache,
    runtime_profile_health_score, runtime_profile_inflight_soft_limit,
    runtime_profile_inflight_sort_key, runtime_profile_name_in_selection_backoff,
    runtime_profile_route_circuit_open_until, runtime_profile_transport_backoff_until_from_map,
    runtime_proxy_log, runtime_proxy_pressure_mode_active_for_route,
    runtime_quota_summary_for_route, runtime_route_kind_label,
};
use super::summary::{
    runtime_auto_redeem_quota_summary_has_weekly_remaining,
    runtime_auto_redeem_quota_summary_warrants_credit,
    runtime_auto_redeem_weekly_exhausted_reset_at,
};
use crate::ProfileProviderExt;
use anyhow::Result;
use chrono::Local;
use std::collections::BTreeSet;

fn runtime_auto_redeem_plan_priority(plan_type: Option<&str>) -> usize {
    let normalized = plan_type
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_'))
        .collect::<String>();

    match normalized.as_str() {
        "plus" => 0,
        "free" | "basic" => 1,
        "" => 2,
        "prolite" | "pro" | "pro5x" | "5x" | "pro20x" | "pro20" | "20x" | "ultra" | "max"
        | "team" | "business" | "enterprise" => 3,
        _ => 2,
    }
}

pub(super) fn refresh_runtime_auto_redeem_pool_missing_quota(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
    context: &str,
) -> Result<()> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
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
                if runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    &name,
                    now,
                ) {
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
                if runtime_profile_inflight_sort_key(&name, &runtime.profile_inflight)
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

    if !jobs.is_empty() {
        runtime_proxy_log(
            shared,
            format!(
                "{context}_auto_redeem_pool_probe_start route={} jobs={}",
                runtime_route_kind_label(route_kind),
                jobs.len(),
            ),
        );
        run_runtime_probe_jobs_inline(shared, jobs, &format!("{context}_auto_redeem_pool_probe"));
    }
    Ok(())
}

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
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;

    Ok(
        active_profile_selection_order(&runtime.state, &runtime.current_profile)
            .into_iter()
            .filter(|name| !excluded_profiles.contains(name))
            .find(|name| {
                let Some(profile) = runtime.state.profiles.get(name) else {
                    return false;
                };
                if !matches!(profile.provider, crate::ProfileProvider::Openai) {
                    return false;
                }
                if runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    name,
                    now,
                ) {
                    return false;
                }
                if runtime_profile_transport_backoff_until_from_map(
                    &runtime.profile_transport_backoff_until,
                    name,
                    route_kind,
                    now,
                )
                .is_some()
                    || runtime_profile_route_circuit_open_until(&runtime, name, route_kind, now)
                        .is_some()
                {
                    return false;
                }
                if runtime_profile_inflight_sort_key(name, &runtime.profile_inflight)
                    >= inflight_soft_limit
                {
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
                runtime_auto_redeem_quota_summary_has_weekly_remaining(
                    runtime_quota_summary_for_route(usage, route_kind),
                )
            }),
    )
}

pub(crate) fn runtime_best_auto_redeem_profile_name(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let order = active_profile_selection_order(&runtime.state, &runtime.current_profile);

    Ok(order
        .into_iter()
        .enumerate()
        .filter(|(_, name)| !excluded_profiles.contains(name))
        .filter_map(|(order_index, name)| {
            let profile = runtime.state.profiles.get(&name)?;
            if !matches!(profile.provider, crate::ProfileProvider::Openai) {
                return None;
            }
            if runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                &name,
                now,
            ) {
                return None;
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
                return None;
            }
            let inflight_count =
                runtime_profile_inflight_sort_key(&name, &runtime.profile_inflight);
            if inflight_count >= inflight_soft_limit {
                return None;
            }
            let probe = runtime.profile_probe_cache.get(&name)?;
            if !probe.auth.quota_compatible {
                return None;
            }
            let usage = probe.result.as_ref().ok()?;
            let available_count = usage
                .rate_limit_reset_credits
                .as_ref()
                .map(|credits| credits.available_count)
                .unwrap_or_default();
            if available_count <= 0 {
                return None;
            }
            let quota_summary = runtime_quota_summary_for_route(usage, route_kind);
            if !runtime_auto_redeem_quota_summary_warrants_credit(quota_summary, now) {
                return None;
            }
            let reset_sort_key = runtime_auto_redeem_weekly_exhausted_reset_at(quota_summary)
                .unwrap_or_default()
                .saturating_neg();
            let health_sort_key = runtime_profile_health_score(&runtime, &name, now, route_kind);
            Some((
                (
                    runtime_auto_redeem_plan_priority(usage.plan_type.as_deref()),
                    reset_sort_key,
                    inflight_count,
                    health_sort_key,
                    order_index,
                    name.clone(),
                ),
                name,
            ))
        })
        .min_by_key(|(sort_key, _)| sort_key.clone())
        .map(|(_, name)| name))
}
