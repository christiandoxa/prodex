use super::*;

#[derive(Debug, Clone)]
pub(crate) struct RuntimePlannedProbeRefresh {
    pub(crate) name: String,
    pub(crate) codex_home: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeResponseProbePlan {
    pub(crate) reports: Vec<RunProfileProbeReport>,
    pub(crate) ready_candidates: Vec<ReadyProfileCandidate>,
    pub(crate) stale_probe_refreshes: Vec<RuntimePlannedProbeRefresh>,
    pub(crate) cold_start_probe_jobs: Vec<RunProfileProbeJob>,
    pub(crate) sync_probe_jobs: Vec<RunProfileProbeJob>,
    pub(crate) should_sync_probe_cold_start: bool,
    pub(crate) sync_probe_skip_jobs_count: Option<usize>,
    pub(crate) sync_probe_skip_profiles_count: Option<usize>,
}

pub(crate) fn build_runtime_response_probe_plan(
    selection_state: &RuntimeRouteSelectionCatalog,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    allow_disk_auth_fallback: bool,
    sync_probe_pressure_mode: bool,
    inflight_soft_limit: usize,
    now: i64,
) -> RuntimeResponseProbePlan {
    let mut reports = Vec::new();
    let mut stale_probe_refreshes = Vec::new();
    let mut cold_start_probe_jobs = Vec::new();

    for (order_index, name) in active_profile_selection_order_with_view(
        runtime_route_selection_view(selection_state),
        &selection_state.current_profile,
    )
    .into_iter()
    .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = selection_state.entry(&name) else {
            continue;
        };
        if let Some(probe_entry) = entry.cached_probe_entry.as_ref() {
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth: probe_entry.auth.clone(),
                result: probe_entry.result.clone(),
            });
            if runtime_profile_probe_cache_freshness(probe_entry, now)
                != RuntimeProbeCacheFreshness::Fresh
                && entry.supports_codex_runtime()
            {
                stale_probe_refreshes.push(RuntimePlannedProbeRefresh {
                    name,
                    codex_home: entry.profile.codex_home.clone(),
                });
            }
        } else {
            let auth = entry
                .cached_auth_summary
                .clone()
                .or_else(|| {
                    allow_disk_auth_fallback.then(|| read_auth_summary(&entry.profile.codex_home))
                })
                .unwrap_or_else(runtime_profile_uncached_auth_summary_for_selection);
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth,
                result: Err("runtime quota snapshot unavailable".to_string()),
            });
            cold_start_probe_jobs.push(RunProfileProbeJob {
                name,
                order_index,
                provider: entry.profile.provider.clone(),
                codex_home: entry.profile.codex_home.clone(),
            });
        }
    }

    let cached_usage_snapshots = selection_state.persisted_usage_snapshots();

    cold_start_probe_jobs.sort_by_key(|job| {
        let quota_summary = cached_usage_snapshots
            .get(&job.name)
            .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
            .map(|snapshot| runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now))
            .unwrap_or(RuntimeQuotaSummary {
                five_hour: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                weekly: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                route_band: RuntimeQuotaPressureBand::Unknown,
            });
        (
            runtime_quota_pressure_sort_key_for_route_from_summary(quota_summary),
            job.order_index,
        )
    });

    let request_probe_jobs = cold_start_probe_jobs
        .iter()
        .filter(|job| {
            !cached_usage_snapshots
                .get(&job.name)
                .is_some_and(|snapshot| {
                    runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
                })
        })
        .cloned()
        .collect::<Vec<_>>();

    reports.sort_by_key(|report| report.order_index);
    let ready_candidates =
        runtime_response_ready_candidates(selection_state, &reports, &cached_usage_snapshots);
    let best_candidate_order_index = runtime_response_best_candidate_order_index(
        selection_state,
        excluded_profiles,
        &ready_candidates,
        inflight_soft_limit,
    );
    let sync_probe_jobs = request_probe_jobs
        .iter()
        .filter(|job| {
            ready_candidates.is_empty()
                || best_candidate_order_index.is_none()
                || best_candidate_order_index
                    .is_some_and(|best_order_index| job.order_index < best_order_index)
        })
        .cloned()
        .collect::<Vec<_>>();
    let should_sync_probe_cold_start =
        !sync_probe_pressure_mode && !request_probe_jobs.is_empty() && !sync_probe_jobs.is_empty();
    let sync_probe_skip_jobs_count = (sync_probe_pressure_mode && !request_probe_jobs.is_empty())
        .then_some(request_probe_jobs.len());
    let sync_probe_skip_profiles_count = (sync_probe_pressure_mode
        && !cold_start_probe_jobs.is_empty())
    .then_some(cold_start_probe_jobs.len());
    RuntimeResponseProbePlan {
        reports,
        ready_candidates,
        stale_probe_refreshes,
        cold_start_probe_jobs,
        sync_probe_jobs,
        should_sync_probe_cold_start,
        sync_probe_skip_jobs_count,
        sync_probe_skip_profiles_count,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeResponseCandidateExecutionPlan {
    pub(crate) ready_candidates: Vec<RuntimeResponsePlannedCandidate>,
    pub(crate) fallback_candidates: Vec<RuntimeResponsePlannedCandidate>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeResponsePlannedCandidate {
    pub(crate) name: String,
    pub(crate) order_index: usize,
    pub(crate) inflight_count: usize,
    pub(crate) inflight_soft_limit: usize,
    pub(crate) health_sort_key: u32,
    pub(crate) backoff_sort_key: (usize, i64, i64, i64),
    pub(crate) quota_source: RuntimeQuotaSource,
    pub(crate) quota_summary: RuntimeQuotaSummary,
    pub(crate) auth_failure_active: bool,
    pub(crate) quota_guard_reason: Option<&'static str>,
    pub(crate) inflight_soft_limited: bool,
}

impl RuntimeResponsePlannedCandidate {
    pub(crate) fn ready_skip_reason(&self) -> Option<&'static str> {
        runtime_proxy_crate::runtime_response_candidate_ready_skip_reason(
            self.auth_failure_active,
            self.quota_guard_reason,
            self.inflight_soft_limited,
        )
    }

    pub(crate) fn fallback_skip_reason(&self) -> Option<&'static str> {
        runtime_proxy_crate::runtime_response_candidate_fallback_skip_reason(
            self.auth_failure_active,
            self.quota_guard_reason,
        )
    }
}

pub(crate) struct RuntimeResponseCandidateExecutionOptions<'a, F>
where
    F: Fn(&str) -> u64,
{
    pub(crate) prompt_cache_key: Option<&'a str>,
    pub(crate) prompt_cache_owner_profile: Option<&'a str>,
    pub(crate) jitter_for: F,
}

pub(crate) fn runtime_response_candidate_execution_options<'a, F>(
    prompt_cache_key: Option<&'a str>,
    prompt_cache_owner_profile: Option<&'a str>,
    jitter_for: F,
) -> RuntimeResponseCandidateExecutionOptions<'a, F>
where
    F: Fn(&str) -> u64,
{
    RuntimeResponseCandidateExecutionOptions {
        prompt_cache_key,
        prompt_cache_owner_profile,
        jitter_for,
    }
}

pub(crate) use runtime_proxy_crate::{
    RuntimeOptimisticCurrentCandidateDecision, RuntimeOptimisticCurrentCandidateSkipReason,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeOptimisticCurrentCandidateSelectionInput<'a> {
    pub(crate) current_profile: &'a str,
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) auth_failure_active: bool,
    pub(crate) in_selection_backoff: bool,
    pub(crate) circuit_open: bool,
    pub(crate) health_score: u32,
    pub(crate) performance_score: u32,
    pub(crate) current_profile_quota_compatible: bool,
    pub(crate) has_alternative_quota_compatible_profile: bool,
    pub(crate) quota_summary: RuntimeQuotaSummary,
    pub(crate) quota_source: Option<RuntimeQuotaSource>,
    pub(crate) inflight_count: usize,
    pub(crate) inflight_soft_limit: usize,
    pub(crate) prompt_cache_key: Option<&'a str>,
    pub(crate) prompt_cache_owner_profile: Option<&'a str>,
}

pub(crate) fn runtime_optimistic_current_candidate_decision(
    input: RuntimeOptimisticCurrentCandidateSelectionInput<'_>,
) -> RuntimeOptimisticCurrentCandidateDecision {
    runtime_proxy_crate::runtime_optimistic_current_candidate_decision(
        runtime_proxy_crate::RuntimeOptimisticCurrentCandidateInput {
            current_profile: input.current_profile,
            route_kind: prodex_runtime_quota::runtime_route_kind_to_proxy(input.route_kind),
            auth_failure_active: input.auth_failure_active,
            in_selection_backoff: input.in_selection_backoff,
            circuit_open: input.circuit_open,
            health_score: input.health_score,
            performance_score: input.performance_score,
            current_profile_quota_compatible: input.current_profile_quota_compatible,
            has_alternative_quota_compatible_profile: input
                .has_alternative_quota_compatible_profile,
            quota_summary: prodex_runtime_quota::runtime_selection_quota_summary_to_proxy(
                input.quota_summary,
            ),
            quota_source: input
                .quota_source
                .map(prodex_runtime_quota::runtime_quota_source_to_proxy),
            inflight_count: input.inflight_count,
            inflight_soft_limit: input.inflight_soft_limit,
            prompt_cache_key: input.prompt_cache_key,
            prompt_cache_owner_profile: input.prompt_cache_owner_profile,
        },
    )
}

pub(crate) fn build_runtime_response_candidate_execution_plan<F>(
    selection_state: &RuntimeRouteSelectionCatalog,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    inflight_soft_limit: usize,
    ready_profile_candidates: Vec<ReadyProfileCandidate>,
    options: RuntimeResponseCandidateExecutionOptions<'_, F>,
) -> RuntimeResponseCandidateExecutionPlan
where
    F: Fn(&str) -> u64,
{
    let RuntimeResponseCandidateExecutionOptions {
        prompt_cache_key,
        prompt_cache_owner_profile,
        jitter_for,
    } = options;
    let mut candidate_quota_summaries = BTreeMap::new();
    let candidate_inputs = ready_profile_candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| !excluded_profiles.contains(&candidate.name))
        .filter_map(|(order_index, candidate)| {
            let entry = selection_state.entry(&candidate.name)?;
            let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
            candidate_quota_summaries.insert((candidate.name.clone(), order_index), quota_summary);
            Some(runtime_proxy_crate::RuntimeResponseCandidatePlanInput {
                name: candidate.name.clone(),
                order_index,
                inflight_count: entry.inflight_count,
                health_sort_key: entry.health_sort_key,
                backoff_sort_key: entry.backoff_sort_key,
                quota_source: prodex_runtime_quota::runtime_quota_source_to_proxy(
                    candidate.quota_source,
                ),
                quota_summary: prodex_runtime_quota::runtime_selection_quota_summary_to_proxy(
                    quota_summary,
                ),
                auth_failure_active: entry.auth_failure_active,
                provider_priority: candidate.provider_priority,
                quota_sort_key:
                    prodex_runtime_quota::runtime_response_quota_pressure_sort_key_to_proxy(
                        runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
                    ),
                in_selection_backoff: entry.in_selection_backoff,
                jitter: jitter_for(&candidate.name),
            })
        })
        .collect::<Vec<_>>();
    let proxy_plan = runtime_proxy_crate::build_runtime_response_candidate_execution_plan(
        candidate_inputs,
        excluded_profiles,
        runtime_proxy_crate::runtime_response_candidate_plan_options(
            prodex_runtime_quota::runtime_route_kind_to_proxy(route_kind),
            inflight_soft_limit,
            prompt_cache_key,
            prompt_cache_owner_profile,
            runtime_proxy_responses_quota_critical_floor_percent(),
        ),
    );

    RuntimeResponseCandidateExecutionPlan {
        ready_candidates: proxy_plan
            .ready_candidates
            .into_iter()
            .map(|candidate| {
                runtime_response_planned_candidate_from_proxy(candidate, &candidate_quota_summaries)
            })
            .collect(),
        fallback_candidates: proxy_plan
            .fallback_candidates
            .into_iter()
            .map(|candidate| {
                runtime_response_planned_candidate_from_proxy(candidate, &candidate_quota_summaries)
            })
            .collect(),
    }
}

#[cfg(test)]
pub(crate) fn runtime_prompt_cache_affinity_sort_key(
    prompt_cache_key: Option<&str>,
    profile_name: &str,
) -> u64 {
    runtime_proxy_crate::runtime_prompt_cache_affinity_sort_key(prompt_cache_key, profile_name)
}

fn runtime_response_planned_candidate_from_proxy(
    candidate: runtime_proxy_crate::RuntimeResponsePlannedCandidate,
    quota_summaries: &BTreeMap<(String, usize), RuntimeQuotaSummary>,
) -> RuntimeResponsePlannedCandidate {
    RuntimeResponsePlannedCandidate {
        quota_summary: quota_summaries
            .get(&(candidate.name.clone(), candidate.order_index))
            .copied()
            .unwrap_or_else(|| {
                prodex_runtime_quota::runtime_selection_quota_summary_from_proxy(
                    candidate.quota_summary,
                )
            }),
        name: candidate.name,
        order_index: candidate.order_index,
        inflight_count: candidate.inflight_count,
        inflight_soft_limit: candidate.inflight_soft_limit,
        health_sort_key: candidate.health_sort_key,
        backoff_sort_key: candidate.backoff_sort_key,
        quota_source: prodex_runtime_quota::runtime_quota_source_from_proxy(candidate.quota_source),
        auth_failure_active: candidate.auth_failure_active,
        quota_guard_reason: candidate.quota_guard_reason,
        inflight_soft_limited: candidate.inflight_soft_limited,
    }
}

fn runtime_response_ready_candidates(
    selection_state: &RuntimeRouteSelectionCatalog,
    reports: &[RunProfileProbeReport],
    cached_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) -> Vec<ReadyProfileCandidate> {
    ready_profile_candidates_with_view(
        reports,
        selection_state.include_code_review,
        Some(selection_state.current_profile.as_str()),
        runtime_route_selection_view(selection_state),
        Some(cached_usage_snapshots),
    )
}

fn runtime_response_best_candidate_order_index(
    selection_state: &RuntimeRouteSelectionCatalog,
    excluded_profiles: &BTreeSet<String>,
    candidates: &[ReadyProfileCandidate],
    inflight_soft_limit: usize,
) -> Option<usize> {
    candidates
        .iter()
        .filter(|candidate| !excluded_profiles.contains(&candidate.name))
        .filter(|candidate| {
            selection_state.entry(&candidate.name).is_some_and(|entry| {
                !entry.in_selection_backoff
                    && !entry.auth_failure_active
                    && entry.inflight_count < inflight_soft_limit
            })
        })
        .map(|candidate| candidate.order_index)
        .min()
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/selection_plan.rs"]
mod tests;
