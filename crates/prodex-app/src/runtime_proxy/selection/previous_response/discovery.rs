use super::super::*;
use prodex_quota::{RuntimeQuotaPressureBand, RuntimeQuotaSummary};
use prodex_shared_types::RuntimeQuotaSource;
use prodex_state::ProfileEntry;
use std::path::PathBuf;

struct RuntimePreviousResponseDiskFallbackEntry {
    name: String,
    codex_home: PathBuf,
    order_index: usize,
}

enum RuntimePreviousResponseRejection<'a> {
    NegativeCache {
        response_id: &'a str,
    },
    AuthFailure,
    Quota {
        summary: RuntimeQuotaSummary,
        source: Option<RuntimeQuotaSource>,
        reason: &'static str,
    },
}

struct RuntimePreviousResponseEligibleCandidate {
    quota_summary: RuntimeQuotaSummary,
}

struct RuntimePreviousResponseDiscovery {
    selected: Option<String>,
    disk_fallback_entries: Vec<RuntimePreviousResponseDiskFallbackEntry>,
}

#[derive(Clone, Copy)]
struct RuntimePreviousResponseDiscoveryContext<'a> {
    shared: &'a RuntimeRotationProxyShared,
    runtime: &'a RuntimeRotationState,
    excluded_profiles: &'a BTreeSet<String>,
    previous_response_id: Option<&'a str>,
    route_kind: RuntimeRouteKind,
    allow_disk_auth_fallback: bool,
    now: i64,
}

#[derive(Clone, Copy)]
struct RuntimePreviousResponseCandidateLogContext<'a> {
    shared: &'a RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    name: &'a str,
    order_index: usize,
}

pub(super) fn discover_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<Option<String>> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let now = Local::now().timestamp();
    let disk_fallback_entries = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        if let Some(owner) = bound_previous_response_owner(
            &runtime,
            excluded_profiles,
            previous_response_id,
            route_kind,
            now,
        ) {
            record_runtime_previous_response_selection(trace, owner, 0, None, true);
            return Ok(Some(owner.to_string()));
        }
        let discovered = discover_cached_previous_response_candidate(
            RuntimePreviousResponseDiscoveryContext {
                shared,
                runtime: &runtime,
                excluded_profiles,
                previous_response_id,
                route_kind,
                allow_disk_auth_fallback,
                now,
            },
            trace,
        );
        if let Some(selected) = discovered.selected {
            return Ok(Some(selected));
        }
        discovered.disk_fallback_entries
    };
    select_runtime_previous_response_disk_fallback(disk_fallback_entries, trace)
}

fn bound_previous_response_owner<'a>(
    runtime: &'a RuntimeRotationState,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<&'a str> {
    let response_id = previous_response_id?;
    let owner = runtime
        .state
        .response_profile_bindings
        .get(response_id)?
        .profile_name
        .as_str();
    (!excluded_profiles.contains(owner)
        && runtime.state.profiles.contains_key(owner)
        && !runtime_previous_response_negative_cache_active(
            &runtime.profile_health,
            response_id,
            owner,
            route_kind,
            now,
        ))
    .then_some(owner)
}

fn discover_cached_previous_response_candidate(
    context: RuntimePreviousResponseDiscoveryContext<'_>,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> RuntimePreviousResponseDiscovery {
    let RuntimePreviousResponseDiscoveryContext {
        shared,
        runtime,
        excluded_profiles,
        previous_response_id,
        route_kind,
        allow_disk_auth_fallback,
        now,
    } = context;
    let mut disk_fallback_entries = Vec::new();
    for (order_index, (name, profile)) in runtime_previous_response_ordered_profiles(runtime)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(name) {
            continue;
        }
        let eligible = match runtime_previous_response_candidate_eligibility(
            runtime,
            name,
            previous_response_id,
            route_kind,
            now,
        ) {
            Ok(eligible) => eligible,
            Err(rejection) => {
                record_runtime_previous_response_rejection(
                    shared,
                    trace,
                    route_kind,
                    name,
                    order_index,
                    rejection,
                );
                continue;
            }
        };
        match runtime_profile_cached_auth_summary_from_maps_for_selection(
            name,
            &runtime.profile_usage_auth,
            &runtime.profile_probe_cache,
        ) {
            Some(summary) if summary.quota_compatible => {
                record_runtime_previous_response_selection(
                    trace,
                    name,
                    order_index,
                    Some(eligible.quota_summary),
                    false,
                );
                return RuntimePreviousResponseDiscovery {
                    selected: Some(name.to_string()),
                    disk_fallback_entries,
                };
            }
            Some(_) => record_runtime_previous_response_auth_incompatible(
                trace,
                name,
                order_index,
                eligible.quota_summary,
            ),
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
    RuntimePreviousResponseDiscovery {
        selected: None,
        disk_fallback_entries,
    }
}

fn runtime_previous_response_candidate_eligibility<'a>(
    runtime: &RuntimeRotationState,
    name: &str,
    previous_response_id: Option<&'a str>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> std::result::Result<
    RuntimePreviousResponseEligibleCandidate,
    RuntimePreviousResponseRejection<'a>,
> {
    if let Some(response_id) = previous_response_id
        && runtime_previous_response_negative_cache_active(
            &runtime.profile_health,
            response_id,
            name,
            route_kind,
            now,
        )
    {
        return Err(RuntimePreviousResponseRejection::NegativeCache { response_id });
    }
    if runtime_profile_auth_failure_active_with_auth_cache(
        &runtime.profile_health,
        &runtime.profile_usage_auth,
        name,
        now,
    ) {
        return Err(RuntimePreviousResponseRejection::AuthFailure);
    }
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route_from_state(runtime, name, route_kind, now);
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
        || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
    {
        let reason = runtime_quota_precommit_guard_reason(quota_summary, route_kind)
            .unwrap_or_else(|| runtime_quota_pressure_band_reason(quota_summary.route_band));
        return Err(RuntimePreviousResponseRejection::Quota {
            summary: quota_summary,
            source: quota_source,
            reason,
        });
    }
    Ok(RuntimePreviousResponseEligibleCandidate { quota_summary })
}

fn record_runtime_previous_response_rejection(
    shared: &RuntimeRotationProxyShared,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    route_kind: RuntimeRouteKind,
    name: &str,
    order_index: usize,
    rejection: RuntimePreviousResponseRejection<'_>,
) {
    match rejection {
        RuntimePreviousResponseRejection::NegativeCache { response_id } => {
            record_runtime_previous_response_simple_rejection(
                trace,
                name,
                order_index,
                "negative_cache",
                Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Affinity),
            );
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_affinity",
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("affinity", "previous_response_discovery"),
                        runtime_proxy_log_field("profile", name),
                        runtime_proxy_log_field("reason", "negative_cache"),
                        runtime_proxy_log_field("response_id", response_id),
                    ],
                ),
            );
        }
        RuntimePreviousResponseRejection::AuthFailure => {
            let reason =
                runtime_proxy_crate::RuntimeRouteDecisionReasonKind::AuthFailureBackoff.as_str();
            record_runtime_previous_response_simple_rejection(
                trace,
                name,
                order_index,
                reason,
                None,
            );
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
        }
        RuntimePreviousResponseRejection::Quota {
            summary,
            source,
            reason,
        } => record_runtime_previous_response_quota_rejection(
            RuntimePreviousResponseCandidateLogContext {
                shared,
                route_kind,
                name,
                order_index,
            },
            trace,
            summary,
            source,
            reason,
        ),
    }
}

fn record_runtime_previous_response_simple_rejection(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    name: &str,
    order_index: usize,
    reason: &'static str,
    stage: Option<runtime_proxy_crate::RuntimeRouteDecisionStage>,
) {
    let mut candidate = runtime_selection_trace_candidate(
        order_index,
        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
        None,
        None,
        None,
        None,
    );
    runtime_selection_trace_reject(&mut candidate, reason, stage);
    trace.record_candidate(name, candidate);
}

fn record_runtime_previous_response_quota_rejection(
    context: RuntimePreviousResponseCandidateLogContext<'_>,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    reason: &'static str,
) {
    let RuntimePreviousResponseCandidateLogContext {
        shared,
        route_kind,
        name,
        order_index,
    } = context;
    let mut candidate = runtime_selection_trace_candidate(
        order_index,
        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
        Some(summary),
        None,
        None,
        None,
    );
    runtime_selection_trace_reject(
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
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("affinity", "previous_response_discovery"),
                    runtime_proxy_log_field("profile", name),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field(
                        "quota_source",
                        runtime_selection_quota_source_label(source),
                    ),
                ],
                summary,
            ),
        ),
    );
}

fn record_runtime_previous_response_auth_incompatible(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    name: &str,
    order_index: usize,
    quota_summary: RuntimeQuotaSummary,
) {
    let mut candidate = runtime_selection_trace_candidate(
        order_index,
        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
        Some(quota_summary),
        None,
        None,
        None,
    );
    runtime_selection_trace_reject(
        &mut candidate,
        runtime_proxy_crate::RuntimeRouteDecisionReasonKind::AuthNotQuotaCompatible.as_str(),
        None,
    );
    trace.record_candidate(name, candidate);
}

fn record_runtime_previous_response_selection(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    name: &str,
    order_index: usize,
    quota_summary: Option<RuntimeQuotaSummary>,
    hard_affinity: bool,
) {
    let mut candidate = runtime_selection_trace_candidate(
        order_index,
        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
        quota_summary,
        None,
        None,
        None,
    );
    candidate.hard_affinity = hard_affinity;
    candidate.selected = true;
    trace.record_candidate(name, candidate);
    trace.record_affinity(
        runtime_proxy_crate::RuntimeRouteAffinityKind::PreviousResponse,
        Some(name),
        hard_affinity,
        runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained,
    );
}

fn select_runtime_previous_response_disk_fallback(
    entries: Vec<RuntimePreviousResponseDiskFallbackEntry>,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<Option<String>> {
    for entry in entries {
        if read_auth_summary(&entry.codex_home).quota_compatible {
            record_runtime_previous_response_selection(
                trace,
                &entry.name,
                entry.order_index,
                None,
                false,
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
