use super::*;

mod candidates;
mod prepare;

use candidates::{
    RuntimeResponseCandidatePass, record_runtime_response_candidates,
    select_runtime_auto_redeem_candidate, select_runtime_cold_start_candidate,
    select_runtime_response_candidate,
};
use prepare::{log_runtime_response_selection_plan, prepare_runtime_response_selection};

#[cfg(test)]
pub(crate) fn next_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let mut trace = runtime_selection_trace_builder(route_kind, None);
    next_runtime_response_candidate_for_route_with_prompt_cache_key(
        shared,
        excluded_profiles,
        route_kind,
        None,
        &mut trace,
    )
}

pub(super) fn next_runtime_response_candidate_for_route_with_prompt_cache_key(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    prompt_cache_key: Option<&str>,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<Option<String>> {
    let mut prepared = prepare_runtime_response_selection(shared, excluded_profiles, route_kind)?;
    let prompt_cache_owner = runtime_prompt_cache_bound_profile(prompt_cache_key);
    let candidate_plan = build_runtime_response_candidate_execution_plan(
        &prepared.selection_state,
        excluded_profiles,
        route_kind,
        prepared.inflight_soft_limit,
        std::mem::take(&mut prepared.ready_candidates),
        runtime_response_candidate_execution_options(
            prompt_cache_key,
            prompt_cache_owner.as_deref(),
            |name| runtime_profile_selection_jitter(shared, name, route_kind),
        ),
    );
    log_runtime_response_selection_plan(
        shared,
        excluded_profiles,
        route_kind,
        prompt_cache_owner.as_deref(),
        &prepared,
        &candidate_plan,
    );
    let candidate_record_count = record_runtime_response_candidates(trace, &candidate_plan);
    let RuntimeResponseCandidateExecutionPlan {
        ready_candidates,
        fallback_candidates,
    } = candidate_plan;

    if let Some(candidate) = select_runtime_response_candidate(
        shared,
        route_kind,
        ready_candidates,
        RuntimeResponseCandidatePass::Ready,
        trace,
    )? {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = select_runtime_response_candidate(
        shared,
        route_kind,
        fallback_candidates,
        RuntimeResponseCandidatePass::Fallback,
        trace,
    )? {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = select_runtime_auto_redeem_candidate(
        shared,
        excluded_profiles,
        route_kind,
        candidate_record_count,
        trace,
    )? {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = select_runtime_cold_start_candidate(
        shared,
        &prepared.selection_state,
        excluded_profiles,
        route_kind,
        prepared.inflight_soft_limit,
        candidate_record_count,
        trace,
    )? {
        return Ok(Some(candidate));
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_pick",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("profile", "none"),
                runtime_proxy_log_field("mode", "exhausted"),
                runtime_proxy_log_field("excluded_count", excluded_profiles.len().to_string()),
            ],
        ),
    );
    Ok(None)
}

pub(crate) fn runtime_quota_last_chance_profile_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    prompt_cache_key: Option<&str>,
) -> Result<Option<String>> {
    let mut prepared = prepare_runtime_response_selection(shared, excluded_profiles, route_kind)?;
    let prompt_cache_owner = runtime_prompt_cache_bound_profile(prompt_cache_key);
    let candidate_plan = build_runtime_response_candidate_execution_plan(
        &prepared.selection_state,
        excluded_profiles,
        route_kind,
        prepared.inflight_soft_limit,
        std::mem::take(&mut prepared.ready_candidates),
        runtime_response_candidate_execution_options(
            prompt_cache_key,
            prompt_cache_owner.as_deref(),
            |name| runtime_profile_selection_jitter(shared, name, route_kind),
        ),
    );
    log_runtime_response_selection_plan(
        shared,
        excluded_profiles,
        route_kind,
        prompt_cache_owner.as_deref(),
        &prepared,
        &candidate_plan,
    );

    for candidate in candidate_plan.fallback_candidates {
        if candidate.fallback_skip_reason().is_some()
            || !matches!(candidate.backoff_sort_key.0, 0 | 1 | 2 | 4)
            || runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate.name,
                runtime_route_kind_inflight_context(route_kind),
            )?
        {
            continue;
        }
        return Ok(Some(candidate.name));
    }
    for name in active_profile_selection_order_with_view(
        runtime_route_selection_view(&prepared.selection_state),
        &prepared.selection_state.current_profile,
    ) {
        if excluded_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = prepared.selection_state.entry(&name) else {
            continue;
        };
        if !entry.supports_codex_runtime()
            || !entry
                .cached_auth_summary
                .as_ref()
                .is_some_and(|auth| auth.quota_compatible)
            || entry
                .cached_probe_entry
                .as_ref()
                .is_some_and(|probe| probe.result.is_ok())
            || entry.cached_usage_snapshot.is_some()
            || entry.auth_failure_active
            || !matches!(entry.backoff_sort_key.0, 0 | 1 | 2 | 4)
            || runtime_profile_inflight_hard_limited_for_context(
                shared,
                &name,
                runtime_route_kind_inflight_context(route_kind),
            )?
        {
            continue;
        }
        return Ok(Some(name));
    }
    Ok(None)
}
