use super::{
    RuntimeRotationProxyShared, RuntimeRotationState, read_auth_summary,
    runtime_previous_response_negative_cache_active,
    runtime_profile_auth_failure_active_with_auth_cache,
    runtime_profile_cached_auth_summary_from_maps_for_selection,
    runtime_profile_quota_summary_for_route_from_state, runtime_proxy_log,
    runtime_proxy_sync_probe_pressure_mode_active_for_route, runtime_quota_precommit_guard_reason,
    runtime_quota_pressure_band_reason, runtime_route_kind_label,
    runtime_selection_log_fields_with_quota, runtime_selection_quota_source_label,
};
use anyhow::Result;
use chrono::Local;
use prodex_quota::RuntimeQuotaPressureBand;
use prodex_runtime_state::RuntimeRouteKind;
use prodex_state::ProfileEntry;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[cfg(any(test, feature = "bench-support"))]
pub(crate) fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let mut trace = super::runtime_selection_trace_builder(route_kind, None);
    next_runtime_previous_response_candidate_with_trace(
        shared,
        excluded_profiles,
        previous_response_id,
        route_kind,
        &mut trace,
    )
}

pub(super) fn next_runtime_previous_response_candidate_with_trace(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<Option<String>> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let now = Local::now().timestamp();
    struct RuntimePreviousResponseDiskFallbackEntry {
        name: String,
        codex_home: PathBuf,
        order_index: usize,
    }

    let disk_fallback_entries = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        if let Some(owner) = previous_response_id.and_then(|response_id| {
            runtime
                .state
                .response_profile_bindings
                .get(response_id)
                .map(|binding| binding.profile_name.as_str())
        }) && !excluded_profiles.contains(owner)
            && runtime.state.profiles.contains_key(owner)
            && !previous_response_id.is_some_and(|response_id| {
                runtime_previous_response_negative_cache_active(
                    &runtime.profile_health,
                    response_id,
                    owner,
                    route_kind,
                    now,
                )
            })
        {
            let mut candidate = super::runtime_selection_trace_candidate(
                0,
                runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
                None,
                None,
                None,
                None,
            );
            candidate.hard_affinity = true;
            candidate.selected = true;
            trace.record_candidate(owner, candidate);
            trace.record_affinity(
                runtime_proxy_crate::RuntimeRouteAffinityKind::PreviousResponse,
                Some(owner),
                true,
                runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained,
            );
            return Ok(Some(owner.to_string()));
        }
        let mut disk_fallback_entries = Vec::new();
        for (order_index, (name, profile)) in runtime_previous_response_ordered_profiles(&runtime)
            .into_iter()
            .enumerate()
        {
            if excluded_profiles.contains(name) {
                continue;
            }
            if let Some(previous_response_id) = previous_response_id
                && runtime_previous_response_negative_cache_active(
                    &runtime.profile_health,
                    previous_response_id,
                    name,
                    route_kind,
                    now,
                )
            {
                let mut candidate = super::runtime_selection_trace_candidate(
                    order_index,
                    runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
                    None,
                    None,
                    None,
                    None,
                );
                super::runtime_selection_trace_reject(
                    &mut candidate,
                    "negative_cache",
                    Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Affinity),
                );
                trace.record_candidate(name, candidate);
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_affinity",
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("affinity", "previous_response_discovery"),
                            runtime_proxy_log_field("profile", name),
                            runtime_proxy_log_field("reason", "negative_cache"),
                            runtime_proxy_log_field("response_id", previous_response_id),
                        ],
                    ),
                );
                continue;
            }
            if runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                name,
                now,
            ) {
                let mut candidate = super::runtime_selection_trace_candidate(
                    order_index,
                    runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
                    None,
                    None,
                    None,
                    None,
                );
                super::runtime_selection_trace_reject(
                    &mut candidate,
                    runtime_proxy_crate::RuntimeRouteDecisionReasonKind::AuthFailureBackoff
                        .as_str(),
                    None,
                );
                trace.record_candidate(name, candidate);
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_affinity",
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("affinity", "previous_response_discovery"),
                            runtime_proxy_log_field("profile", name),
                            runtime_proxy_log_field("reason", "auth_failure_backoff"),
                        ],
                    ),
                );
                continue;
            }
            let (quota_summary, quota_source) =
                runtime_profile_quota_summary_for_route_from_state(&runtime, name, route_kind, now);
            if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
                || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
            {
                let reason = runtime_quota_precommit_guard_reason(quota_summary, route_kind)
                    .unwrap_or_else(|| {
                        runtime_quota_pressure_band_reason(quota_summary.route_band)
                    });
                let mut candidate = super::runtime_selection_trace_candidate(
                    order_index,
                    runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
                    Some(quota_summary),
                    None,
                    None,
                    None,
                );
                super::runtime_selection_trace_reject(
                    &mut candidate,
                    reason,
                    Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Quota),
                );
                trace.record_candidate(name, candidate);
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_affinity",
                        runtime_selection_log_fields_with_quota(
                            [
                                runtime_proxy_log_field(
                                    "route",
                                    runtime_route_kind_label(route_kind),
                                ),
                                runtime_proxy_log_field("affinity", "previous_response_discovery"),
                                runtime_proxy_log_field("profile", name),
                                runtime_proxy_log_field("reason", reason),
                                runtime_proxy_log_field(
                                    "quota_source",
                                    runtime_selection_quota_source_label(quota_source),
                                ),
                            ],
                            quota_summary,
                        ),
                    ),
                );
                continue;
            }
            match runtime_profile_cached_auth_summary_from_maps_for_selection(
                name,
                &runtime.profile_usage_auth,
                &runtime.profile_probe_cache,
            ) {
                Some(summary) if summary.quota_compatible => {
                    let mut candidate = super::runtime_selection_trace_candidate(
                        order_index,
                        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
                        Some(quota_summary),
                        None,
                        None,
                        None,
                    );
                    candidate.selected = true;
                    trace.record_candidate(name, candidate);
                    trace.record_affinity(
                        runtime_proxy_crate::RuntimeRouteAffinityKind::PreviousResponse,
                        Some(name),
                        false,
                        runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained,
                    );
                    return Ok(Some(name.to_string()));
                }
                Some(_) => {
                    let mut candidate = super::runtime_selection_trace_candidate(
                        order_index,
                        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
                        Some(quota_summary),
                        None,
                        None,
                        None,
                    );
                    super::runtime_selection_trace_reject(
                        &mut candidate,
                        runtime_proxy_crate::RuntimeRouteDecisionReasonKind::AuthNotQuotaCompatible
                            .as_str(),
                        None,
                    );
                    trace.record_candidate(name, candidate);
                }
                None if allow_disk_auth_fallback => {
                    disk_fallback_entries.push(RuntimePreviousResponseDiskFallbackEntry {
                        name: name.to_string(),
                        codex_home: profile.codex_home.clone(),
                        order_index,
                    });
                }
                None => {}
            }
        }
        disk_fallback_entries
    };

    for entry in disk_fallback_entries {
        if read_auth_summary(&entry.codex_home).quota_compatible {
            let mut candidate = super::runtime_selection_trace_candidate(
                entry.order_index,
                runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
                None,
                None,
                None,
                None,
            );
            candidate.selected = true;
            trace.record_candidate(&entry.name, candidate);
            trace.record_affinity(
                runtime_proxy_crate::RuntimeRouteAffinityKind::PreviousResponse,
                Some(&entry.name),
                false,
                runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained,
            );
            return Ok(Some(entry.name));
        }
    }
    Ok(None)
}

fn runtime_previous_response_ordered_profiles(
    runtime: &RuntimeRotationState,
) -> Vec<(&str, &ProfileEntry)> {
    let mut ordered = runtime
        .state
        .profiles
        .iter()
        .enumerate()
        .map(|(index, (name, profile))| (index, name.as_str(), profile))
        .collect::<Vec<_>>();
    let current_index = ordered
        .iter()
        .position(|(_, name, _)| *name == runtime.current_profile);
    let profile_count = ordered.len();
    ordered.sort_by_key(|(index, _, profile)| {
        let rotation_index = current_index.map_or(*index, |current_index| {
            if *index >= current_index {
                index - current_index
            } else {
                profile_count - current_index + index
            }
        });
        (profile.provider.runtime_pool_priority(), rotation_index)
    });
    ordered
        .into_iter()
        .map(|(_, name, profile)| (name, profile))
        .collect()
}
