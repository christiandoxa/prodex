use super::*;

pub(crate) fn runtime_has_route_eligible_quota_fallback(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let fallback_profiles = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_profile_selection_catalog(&runtime)
            .entries
            .into_iter()
            .filter(|profile| {
                profile.name != profile_name && !excluded_profiles.contains(&profile.name)
            })
            .map(|profile| {
                let cached_auth_summary =
                    runtime_profile_cached_auth_summary_from_maps_for_selection(
                        &profile.name,
                        &runtime.profile_usage_auth,
                        &runtime.profile_probe_cache,
                    );
                let auth_failure_active = runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    &profile.name,
                    now,
                );
                let in_selection_backoff = runtime_profile_name_in_selection_backoff(
                    &profile.name,
                    &runtime.profile_retry_backoff_until,
                    &runtime.profile_transport_backoff_until,
                    &runtime.profile_route_circuit_open_until,
                    route_kind,
                    now,
                );
                let inflight_count =
                    runtime_profile_inflight_sort_key(&profile.name, &runtime.profile_inflight);

                (
                    profile.name,
                    profile.codex_home,
                    cached_auth_summary,
                    auth_failure_active,
                    in_selection_backoff,
                    inflight_count,
                )
            })
            .collect::<Vec<_>>()
    };

    for (
        _candidate_name,
        codex_home,
        cached_auth_summary,
        auth_failure_active,
        in_selection_backoff,
        inflight_count,
    ) in fallback_profiles
    {
        if auth_failure_active || in_selection_backoff || inflight_count >= inflight_soft_limit {
            continue;
        }

        let quota_compatible = cached_auth_summary
            .or_else(|| allow_disk_auth_fallback.then(|| read_auth_summary(&codex_home)))
            .is_some_and(|summary| summary.quota_compatible);
        if quota_compatible {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn refresh_runtime_profile_quota_inline(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<()> {
    let Some(codex_home) = runtime_profile_codex_home(shared, profile_name)? else {
        return Ok(());
    };
    runtime_proxy_log(shared, format!("{context}_start profile={profile_name}"));
    run_runtime_probe_jobs_inline(
        shared,
        vec![(profile_name.to_string(), codex_home)],
        context,
    );
    Ok(())
}

pub(crate) fn ensure_runtime_profile_precommit_quota_ready(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let (mut quota_summary, mut quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    if runtime_quota_summary_requires_precommit_live_probe(quota_summary, quota_source, route_kind)
    {
        refresh_runtime_profile_quota_inline(shared, profile_name, context)?;
        (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    }
    Ok((quota_summary, quota_source))
}

pub(crate) struct RuntimePrecommitQuotaGateRequest<'a> {
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) profile_name: &'a str,
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) has_continuation_context: bool,
    pub(crate) reprobe_context: &'a str,
}

pub(crate) enum RuntimePrecommitQuotaGateDecision {
    Proceed,
    Block {
        reason: RuntimePrecommitQuotaBlockReason,
        summary: RuntimeQuotaSummary,
        source: Option<RuntimeQuotaSource>,
    },
}

pub(crate) fn runtime_precommit_quota_gate(
    request: RuntimePrecommitQuotaGateRequest<'_>,
) -> Result<RuntimePrecommitQuotaGateDecision> {
    let RuntimePrecommitQuotaGateRequest {
        shared,
        profile_name,
        route_kind,
        has_continuation_context,
        reprobe_context,
    } = request;

    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    match prodex_runtime_quota::runtime_precommit_quota_gate_initial_decision(
        initial_quota_summary,
        initial_quota_source,
        route_kind,
        has_continuation_context,
        runtime_proxy_responses_quota_critical_floor_percent(),
    ) {
        runtime_proxy_crate::RuntimeProxyPrecommitQuotaGateInitialDecision::Block { reason } => {
            return Ok(RuntimePrecommitQuotaGateDecision::Block {
                reason,
                summary: initial_quota_summary,
                source: initial_quota_source,
            });
        }
        runtime_proxy_crate::RuntimeProxyPrecommitQuotaGateInitialDecision::Continue
        | runtime_proxy_crate::RuntimeProxyPrecommitQuotaGateInitialDecision::RefreshRequired => {}
    }

    let has_alternative_quota_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        profile_name,
        &BTreeSet::new(),
        route_kind,
    )?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        route_kind,
        reprobe_context,
    )?;

    match prodex_runtime_quota::runtime_precommit_quota_gate_final_decision(
        quota_summary,
        quota_source,
        route_kind,
        has_alternative_quota_profile,
        runtime_proxy_responses_quota_critical_floor_percent(),
    ) {
        runtime_proxy_crate::RuntimeProxyPrecommitQuotaGateFinalDecision::Proceed => {}
        runtime_proxy_crate::RuntimeProxyPrecommitQuotaGateFinalDecision::Block { reason } => {
            return Ok(RuntimePrecommitQuotaGateDecision::Block {
                reason,
                summary: quota_summary,
                source: quota_source,
            });
        }
    }

    Ok(RuntimePrecommitQuotaGateDecision::Proceed)
}
