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
    provider_priority: usize,
    quota_sort_key: RuntimeQuotaPressureSortKey,
    in_selection_backoff: bool,
    jitter: u64,
}

impl RuntimeResponsePlannedCandidate {
    pub(crate) fn ready_skip_reason(&self) -> Option<&'static str> {
        self.common_skip_reason().or_else(|| {
            self.inflight_soft_limited
                .then_some("profile_inflight_soft_limit")
        })
    }

    pub(crate) fn fallback_skip_reason(&self) -> Option<&'static str> {
        self.common_skip_reason()
    }

    fn common_skip_reason(&self) -> Option<&'static str> {
        if self.auth_failure_active {
            Some("auth_failure_backoff")
        } else {
            self.quota_guard_reason
        }
    }
}

pub(crate) fn build_runtime_response_candidate_execution_plan<F>(
    selection_state: &RuntimeRouteSelectionCatalog,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    inflight_soft_limit: usize,
    ready_profile_candidates: Vec<ReadyProfileCandidate>,
    jitter_for: F,
) -> RuntimeResponseCandidateExecutionPlan
where
    F: Fn(&str) -> u64,
{
    let available_candidates = ready_profile_candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| !excluded_profiles.contains(&candidate.name))
        .filter_map(|(order_index, candidate)| {
            let entry = selection_state.entry(&candidate.name)?;
            let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
            Some(RuntimeResponsePlannedCandidate {
                name: candidate.name.clone(),
                order_index,
                inflight_count: entry.inflight_count,
                inflight_soft_limit,
                health_sort_key: entry.health_sort_key,
                backoff_sort_key: entry.backoff_sort_key,
                quota_source: candidate.quota_source,
                quota_summary,
                auth_failure_active: entry.auth_failure_active,
                quota_guard_reason: matches!(
                    route_kind,
                    RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
                )
                .then(|| runtime_quota_precommit_guard_reason(quota_summary, route_kind))
                .flatten(),
                inflight_soft_limited: entry.inflight_count >= inflight_soft_limit,
                provider_priority: candidate.provider_priority,
                quota_sort_key: runtime_quota_pressure_sort_key_for_route(
                    &candidate.usage,
                    route_kind,
                ),
                in_selection_backoff: entry.in_selection_backoff,
                jitter: jitter_for(&candidate.name),
            })
        })
        .collect::<Vec<_>>();

    let mut ready_candidates = available_candidates
        .iter()
        .filter(|candidate| !candidate.in_selection_backoff)
        .cloned()
        .collect::<Vec<_>>();
    ready_candidates.sort_by_key(|candidate| {
        (
            candidate.provider_priority,
            candidate.quota_sort_key,
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            candidate.inflight_count,
            candidate.health_sort_key,
            candidate.order_index,
            candidate.jitter,
        )
    });

    let mut fallback_candidates = available_candidates;
    fallback_candidates.sort_by_key(|candidate| {
        (
            candidate.backoff_sort_key,
            candidate.provider_priority,
            candidate.quota_sort_key,
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            candidate.inflight_count,
            candidate.health_sort_key,
            candidate.order_index,
            candidate.jitter,
        )
    });

    RuntimeResponseCandidateExecutionPlan {
        ready_candidates,
        fallback_candidates,
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
mod tests {
    use super::*;

    #[test]
    fn probe_plan_marks_pressure_skip_for_request_probe_jobs() {
        let now = Local::now().timestamp();
        let state = RuntimeRouteSelectionCatalog {
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            entries: vec![
                selection_entry(
                    "main",
                    SelectionEntryFixture {
                        cached_probe_entry: Some(RuntimeProfileProbeCacheEntry {
                            checked_at: now,
                            auth: quota_compatible_auth(),
                            result: Ok(test_usage_with_main_windows(80, 300, 80, 86_400)),
                        }),
                        auth_failure_active: true,
                        ..SelectionEntryFixture::default()
                    },
                ),
                selection_entry("second", SelectionEntryFixture::default()),
            ],
        };

        let plan = build_runtime_response_probe_plan(
            &state,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
            false,
            true,
            1,
            now,
        );

        assert_eq!(
            plan.sync_probe_jobs
                .iter()
                .map(|job| job.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second"]
        );
        assert!(!plan.should_sync_probe_cold_start);
        assert_eq!(plan.sync_probe_skip_jobs_count, Some(1));
        assert_eq!(plan.sync_probe_skip_profiles_count, Some(1));
    }

    #[test]
    fn candidate_plan_separates_ready_and_fallback_attempts() {
        let state = RuntimeRouteSelectionCatalog {
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            entries: vec![
                selection_entry(
                    "main",
                    SelectionEntryFixture {
                        backoff_sort_key: (2, 0, 0, 0),
                        inflight_count: 3,
                        health_sort_key: 2,
                        ..SelectionEntryFixture::default()
                    },
                ),
                selection_entry(
                    "second",
                    SelectionEntryFixture {
                        in_selection_backoff: true,
                        backoff_sort_key: (1, 0, 0, 0),
                        ..SelectionEntryFixture::default()
                    },
                ),
            ],
        };
        let reports = vec![
            RunProfileProbeReport {
                name: "main".to_string(),
                order_index: 0,
                auth: quota_compatible_auth(),
                result: Ok(test_usage_with_main_windows(90, 3_600, 95, 86_400)),
            },
            RunProfileProbeReport {
                name: "second".to_string(),
                order_index: 1,
                auth: quota_compatible_auth(),
                result: Ok(test_usage_with_main_windows(90, 3_600, 95, 86_400)),
            },
        ];

        let ready_candidates =
            runtime_response_ready_candidates(&state, &reports, &BTreeMap::new());
        let plan = build_runtime_response_candidate_execution_plan(
            &state,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
            3,
            ready_candidates,
            |_| 0,
        );

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["main"]
        );
        assert_eq!(
            plan.ready_candidates[0].ready_skip_reason(),
            Some("profile_inflight_soft_limit")
        );
        assert_eq!(
            plan.fallback_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second", "main"]
        );
        assert_eq!(plan.fallback_candidates[0].fallback_skip_reason(), None);
    }

    #[test]
    fn candidate_plan_reuses_supplied_ready_candidates_without_rebuilding() {
        let state = RuntimeRouteSelectionCatalog {
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            entries: vec![
                selection_entry(
                    "main",
                    SelectionEntryFixture {
                        cached_probe_entry: Some(RuntimeProfileProbeCacheEntry {
                            checked_at: 0,
                            auth: quota_compatible_auth(),
                            result: Ok(test_usage_with_unbounded_main_windows(90, 95)),
                        }),
                        ..SelectionEntryFixture::default()
                    },
                ),
                selection_entry("second", SelectionEntryFixture::default()),
            ],
        };
        // State-derived report planning would find `main`; execution planning must preserve this
        // already-prepared ready list instead.
        let ready_candidates = vec![ReadyProfileCandidate {
            name: "second".to_string(),
            usage: test_usage_with_unbounded_main_windows(74, 82),
            order_index: 41,
            preferred: false,
            provider_priority: 7,
            quota_source: RuntimeQuotaSource::PersistedSnapshot,
        }];

        let plan = build_runtime_response_candidate_execution_plan(
            &state,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
            3,
            ready_candidates,
            |_| 0,
        );

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second"]
        );
        assert_eq!(plan.ready_candidates[0].order_index, 0);
        assert_eq!(plan.ready_candidates[0].provider_priority, 7);
        assert_eq!(
            plan.ready_candidates[0].quota_source,
            RuntimeQuotaSource::PersistedSnapshot
        );
        assert_eq!(
            plan.ready_candidates[0]
                .quota_summary
                .five_hour
                .remaining_percent,
            74
        );
        assert_eq!(
            plan.fallback_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second"]
        );
    }

    #[derive(Default)]
    struct SelectionEntryFixture {
        cached_probe_entry: Option<RuntimeProfileProbeCacheEntry>,
        cached_usage_snapshot: Option<RuntimeProfileUsageSnapshot>,
        auth_failure_active: bool,
        in_selection_backoff: bool,
        backoff_sort_key: (usize, i64, i64, i64),
        inflight_count: usize,
        health_sort_key: u32,
    }

    fn selection_entry(name: &str, fixture: SelectionEntryFixture) -> RuntimeRouteSelectionEntry {
        RuntimeRouteSelectionEntry {
            profile: RuntimeSelectionProfileEntry {
                name: name.to_string(),
                codex_home: PathBuf::from(format!("/tmp/{name}")),
                provider: ProfileProvider::Openai,
                last_run_selected_at: None,
            },
            cached_auth_summary: Some(quota_compatible_auth()),
            cached_probe_entry: fixture.cached_probe_entry,
            cached_usage_snapshot: fixture.cached_usage_snapshot,
            auth_failure_active: fixture.auth_failure_active,
            in_selection_backoff: fixture.in_selection_backoff,
            backoff_sort_key: fixture.backoff_sort_key,
            inflight_count: fixture.inflight_count,
            health_sort_key: fixture.health_sort_key,
        }
    }

    fn quota_compatible_auth() -> AuthSummary {
        AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        }
    }

    fn test_usage_with_main_windows(
        five_hour_remaining: i64,
        five_hour_reset_offset_seconds: i64,
        weekly_remaining: i64,
        weekly_reset_offset_seconds: i64,
    ) -> UsageResponse {
        let now = Local::now().timestamp();
        UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                    reset_at: Some(now + five_hour_reset_offset_seconds),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                    reset_at: Some(now + weekly_reset_offset_seconds),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        }
    }

    fn test_usage_with_unbounded_main_windows(
        five_hour_remaining: i64,
        weekly_remaining: i64,
    ) -> UsageResponse {
        UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                    reset_at: Some(i64::MAX),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                    reset_at: Some(i64::MAX),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        }
    }
}
