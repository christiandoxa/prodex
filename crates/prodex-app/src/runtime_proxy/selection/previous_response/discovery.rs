use super::super::*;
use prodex_quota::{RuntimeQuotaPressureBand, RuntimeQuotaSummary};
use prodex_state::ProfileEntry;
use std::path::PathBuf;

struct RuntimePreviousResponseDiskFallbackEntry {
    name: String,
    codex_home: PathBuf,
    order_index: usize,
}

struct RuntimePreviousResponseDiscovery {
    selected: Option<String>,
    disk_fallback_entries: Vec<RuntimePreviousResponseDiskFallbackEntry>,
}

enum RuntimeBoundPreviousResponseOwner<'a> {
    Unbound,
    Usable(&'a str),
    Unusable,
}

#[derive(Clone, Copy)]
struct RuntimePreviousResponseDiscoveryContext<'a> {
    runtime: &'a RuntimeRotationState,
    excluded_profiles: &'a BTreeSet<String>,
    previous_response_id: Option<&'a str>,
    route_kind: RuntimeRouteKind,
    allow_disk_auth_fallback: bool,
    now: i64,
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
        match bound_previous_response_owner(
            &runtime,
            excluded_profiles,
            previous_response_id,
            route_kind,
            now,
        )? {
            RuntimeBoundPreviousResponseOwner::Usable(owner) => {
                record_runtime_previous_response_selection(trace, owner, 0, None, true);
                return Ok(Some(owner.to_string()));
            }
            RuntimeBoundPreviousResponseOwner::Unusable => return Ok(None),
            RuntimeBoundPreviousResponseOwner::Unbound => {}
        }
        let discovered = discover_cached_previous_response_candidate(
            RuntimePreviousResponseDiscoveryContext {
                runtime: &runtime,
                excluded_profiles,
                previous_response_id,
                route_kind,
                allow_disk_auth_fallback,
                now,
            },
            trace,
        )?;
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
) -> Result<RuntimeBoundPreviousResponseOwner<'a>> {
    let id_present = previous_response_id.is_some();
    let identity =
        previous_response_id.and_then(prodex_runtime_state::RuntimeHardBindingIdentity::response);
    let owner = if let Some(identity) = identity.as_ref() {
        prodex_runtime_store::runtime_hard_binding_owner(
            identity,
            &runtime.state.response_profile_bindings,
            &runtime.turn_state_bindings,
            &runtime.session_id_bindings,
            &runtime.state.session_profile_bindings,
            &runtime.state.profiles,
        )
    } else {
        prodex_runtime_state::RuntimeHardBindingOwner::Unbound
    };
    let response_id = previous_response_id.unwrap_or_default();
    let owned_profile = match &owner {
        prodex_runtime_state::RuntimeHardBindingOwner::Owned(owner) => Some(owner.as_str()),
        _ => None,
    };
    let binding = owned_profile.and_then(|owner| {
        runtime
            .state
            .response_profile_bindings
            .get(response_id)
            .filter(|binding| binding.profile_name == owner)
    });
    let identity_matches = binding.is_none_or(|binding| {
        binding
            .binding_identity
            .as_ref()
            .is_none_or(|binding_identity| {
                runtime_profile_binding_identity(runtime, owned_profile.unwrap_or_default())
                    .is_some_and(|current_identity| current_identity == *binding_identity)
            })
    });
    let owner_kind = match &owner {
        prodex_runtime_state::RuntimeHardBindingOwner::Unbound => {
            prodex_mojo_core::runtime::RuntimeContinuationOwnerKind::Unbound
        }
        prodex_runtime_state::RuntimeHardBindingOwner::Owned(_) => {
            prodex_mojo_core::runtime::RuntimeContinuationOwnerKind::Owned
        }
        prodex_runtime_state::RuntimeHardBindingOwner::Unavailable(_) => {
            prodex_mojo_core::runtime::RuntimeContinuationOwnerKind::Unavailable
        }
        prodex_runtime_state::RuntimeHardBindingOwner::Conflict => {
            prodex_mojo_core::runtime::RuntimeContinuationOwnerKind::Conflict
        }
    };
    let action = prodex_mojo_core::runtime::runtime_previous_response_owner_plan(
        prodex_mojo_core::runtime::RuntimePreviousResponseOwnerInput {
            id_present,
            id_valid: identity.is_some(),
            owner_kind,
            excluded: owned_profile.is_some_and(|owner| excluded_profiles.contains(owner)),
            auth_failure: owned_profile.is_some_and(|owner| {
                runtime_profile_auth_failure_active_from_map(&runtime.profile_health, owner, now)
            }),
            negative_cache: owned_profile.is_some_and(|owner| {
                runtime_previous_response_negative_cache_active(
                    &runtime.profile_health,
                    response_id,
                    owner,
                    route_kind,
                    now,
                )
            }),
            binding_present: binding.is_some(),
            identity_matches,
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo previous-response owner planning failed: {error:?}"))?;
    let owner = match action {
        prodex_mojo_core::runtime::RuntimePreviousResponseOwnerAction::Unbound => {
            RuntimeBoundPreviousResponseOwner::Unbound
        }
        prodex_mojo_core::runtime::RuntimePreviousResponseOwnerAction::Usable => {
            let binding = binding.ok_or_else(|| {
                anyhow::anyhow!("Mojo accepted previous-response owner without a binding")
            })?;
            RuntimeBoundPreviousResponseOwner::Usable(binding.profile_name.as_str())
        }
        prodex_mojo_core::runtime::RuntimePreviousResponseOwnerAction::Unusable
        | prodex_mojo_core::runtime::RuntimePreviousResponseOwnerAction::Conflict => {
            RuntimeBoundPreviousResponseOwner::Unusable
        }
    };
    Ok(owner)
}

fn discover_cached_previous_response_candidate(
    context: RuntimePreviousResponseDiscoveryContext<'_>,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<RuntimePreviousResponseDiscovery> {
    let RuntimePreviousResponseDiscoveryContext {
        runtime,
        excluded_profiles,
        previous_response_id,
        route_kind,
        allow_disk_auth_fallback,
        now,
    } = context;
    let mut disk_fallback_entries = Vec::new();
    for (order_index, (_, name, profile)) in runtime_previous_response_ordered_profiles(runtime)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(name) {
            continue;
        }
        let negative_cache = previous_response_id.is_some_and(|response_id| {
            runtime_previous_response_negative_cache_active(
                &runtime.profile_health,
                response_id,
                name,
                route_kind,
                now,
            )
        });
        let auth_failure =
            runtime_profile_auth_failure_active_from_map(&runtime.profile_health, name, now);
        let (quota_summary, _) =
            runtime_profile_quota_summary_for_route_from_state(runtime, name, route_kind, now);
        let quota_reason =
            runtime_quota_precommit_guard_reason(quota_summary, route_kind).or_else(|| {
                (quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted)
                    .then(|| runtime_quota_pressure_band_reason(quota_summary.route_band))
            });
        let cached_summary = runtime_profile_cached_auth_summary_from_maps_for_selection(
            name,
            &runtime.profile_usage_auth,
            &runtime.profile_probe_cache,
        );
        let action = prodex_mojo_core::runtime::runtime_previous_response_candidate_plan(
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateInput {
                negative_cache,
                auth_failure,
                quota_exhausted: quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted,
                quota_guard: quota_reason.is_some(),
                cached_auth_present: cached_summary.is_some(),
                cached_auth_compatible: cached_summary
                    .as_ref()
                    .is_some_and(|summary| summary.quota_compatible),
                allow_disk_fallback: allow_disk_auth_fallback,
            },
        )
        .map_err(|error| {
            anyhow::anyhow!("Mojo previous-response candidate planning failed: {error:?}")
        })?;
        match action {
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateAction::RejectNegativeCache => {
                record_runtime_previous_response_simple_rejection(
                    trace,
                    name,
                    order_index,
                    "negative_cache",
                    Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Affinity),
                );
            }
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateAction::RejectAuth => {
                let reason =
                    runtime_proxy_crate::RuntimeRouteDecisionReasonKind::AuthFailureBackoff.as_str();
                record_runtime_previous_response_simple_rejection(
                    trace,
                    name,
                    order_index,
                    reason,
                    None,
                );
            }
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateAction::RejectQuota => {
                record_runtime_previous_response_quota_rejection(
                    trace,
                    name,
                    order_index,
                    quota_summary,
                    quota_reason.unwrap_or_else(|| {
                        runtime_quota_pressure_band_reason(quota_summary.route_band)
                    }),
                );
            }
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateAction::SelectCached => {
                if cached_summary.is_none() {
                    return Err(anyhow::anyhow!(
                        "Mojo selected cached previous-response auth without a cache"
                    ));
                }
                record_runtime_previous_response_selection(
                    trace,
                    name,
                    order_index,
                    Some(quota_summary),
                    false,
                );
                return Ok(RuntimePreviousResponseDiscovery {
                    selected: Some(name.to_string()),
                    disk_fallback_entries,
                });
            }
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateAction::DiskFallback => {
                disk_fallback_entries.push(RuntimePreviousResponseDiskFallbackEntry {
                    name: name.to_string(),
                    codex_home: profile.codex_home.clone(),
                    order_index,
                });
            }
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateAction::Skip => {
                if cached_summary
                    .as_ref()
                    .is_some_and(|summary| !summary.quota_compatible)
                {
                    record_runtime_previous_response_auth_incompatible(
                        trace,
                        name,
                        order_index,
                        quota_summary,
                    );
                }
            }
        }
    }
    Ok(RuntimePreviousResponseDiscovery {
        selected: None,
        disk_fallback_entries,
    })
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
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    name: &str,
    order_index: usize,
    summary: RuntimeQuotaSummary,
    reason: &'static str,
) {
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
        let auth = read_auth_summary(&entry.codex_home);
        let action = prodex_mojo_core::runtime::runtime_previous_response_candidate_plan(
            prodex_mojo_core::runtime::RuntimePreviousResponseCandidateInput {
                negative_cache: false,
                auth_failure: false,
                quota_exhausted: false,
                quota_guard: false,
                cached_auth_present: true,
                cached_auth_compatible: auth.quota_compatible,
                allow_disk_fallback: false,
            },
        )
        .map_err(|error| {
            anyhow::anyhow!("Mojo previous-response disk fallback planning failed: {error:?}")
        })?;
        if action == prodex_mojo_core::runtime::RuntimePreviousResponseCandidateAction::SelectCached
        {
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
) -> Vec<(usize, &str, &ProfileEntry)> {
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
}
