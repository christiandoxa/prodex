use super::*;

pub(crate) trait ProfileSelectionProvider {
    fn runtime_pool_priority(&self) -> usize;
}

impl ProfileSelectionProvider for ProfileEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider.runtime_pool_priority()
    }
}

pub(crate) trait ProfileSelectionRead: Copy {
    type Profile: ProfileSelectionProvider;

    fn profile_names(&self) -> Vec<String>;
    fn profile_entry(&self, name: &str) -> Option<&Self::Profile>;
    fn last_run_selected_at(&self, name: &str) -> Option<i64>;
}

pub(crate) struct ProfileSelectionView<'a, P> {
    pub(crate) profiles: &'a BTreeMap<String, P>,
    pub(crate) last_run_selected_at: &'a BTreeMap<String, i64>,
}

impl<P> Copy for ProfileSelectionView<'_, P> {}

impl<P> Clone for ProfileSelectionView<'_, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: ProfileSelectionProvider> ProfileSelectionRead for ProfileSelectionView<'_, P> {
    type Profile = P;

    fn profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.profiles.get(name)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.last_run_selected_at.get(name).copied()
    }
}

pub(crate) fn app_state_profile_selection_view(
    state: &AppState,
) -> ProfileSelectionView<'_, ProfileEntry> {
    ProfileSelectionView {
        profiles: &state.profiles,
        last_run_selected_at: &state.last_run_selected_at,
    }
}

pub(crate) fn collect_run_profile_reports(
    state: &AppState,
    profile_names: Vec<String>,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Vec<RunProfileProbeReport> {
    let jobs = profile_names
        .into_iter()
        .enumerate()
        .filter_map(|(order_index, name)| {
            let profile = state.profiles.get(&name)?;
            Some(RunProfileProbeJob {
                name,
                order_index,
                provider: profile.provider.clone(),
                codex_home: profile.codex_home.clone(),
            })
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = job.provider.auth_summary(&job.codex_home);
        let result = if auth.quota_compatible {
            fetch_usage_with_proxy_policy(&job.codex_home, base_url.as_deref(), upstream_no_proxy)
                .map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };

        RunProfileProbeReport {
            name: job.name,
            order_index: job.order_index,
            auth,
            result,
        }
    })
}

pub(crate) fn probe_run_profile(
    state: &AppState,
    profile_name: &str,
    order_index: usize,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Result<RunProfileProbeReport> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let auth = profile.provider.auth_summary(&profile.codex_home);
    let result = if auth.quota_compatible {
        fetch_usage_with_proxy_policy(&profile.codex_home, base_url, upstream_no_proxy)
            .map_err(|err| err.to_string())
    } else {
        Err("auth mode is not quota-compatible".to_string())
    };

    Ok(RunProfileProbeReport {
        name: profile_name.to_string(),
        order_index,
        auth,
        result,
    })
}

pub(crate) fn run_profile_probe_is_ready(
    report: &RunProfileProbeReport,
    include_code_review: bool,
) -> bool {
    match report.result.as_ref() {
        Ok(usage) => collect_blocked_limits(usage, include_code_review).is_empty(),
        Err(_) => false,
    }
}

pub(crate) fn run_preflight_reports_with_current_first(
    state: &AppState,
    current_profile: &str,
    current_report: RunProfileProbeReport,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Vec<RunProfileProbeReport> {
    let mut reports = Vec::with_capacity(state.profiles.len());
    reports.push(current_report);
    reports.extend(
        collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
            upstream_no_proxy,
        )
        .into_iter()
        .map(|mut report| {
            report.order_index += 1;
            report
        }),
    );
    reports
}

pub(crate) fn ready_profile_candidates(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    state: &AppState,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
) -> Vec<ReadyProfileCandidate> {
    ready_profile_candidates_with_view(
        reports,
        include_code_review,
        preferred_profile,
        app_state_profile_selection_view(state),
        persisted_usage_snapshots,
    )
}

pub(crate) fn ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    selection: S,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
) -> Vec<ReadyProfileCandidate> {
    let candidates = reports
        .iter()
        .filter_map(|report| {
            if !report.auth.quota_compatible {
                return None;
            }

            let (usage, quota_source) = match report.result.as_ref() {
                Ok(usage) => (usage.clone(), RuntimeQuotaSource::LiveProbe),
                Err(_) => {
                    let snapshot = persisted_usage_snapshots
                        .and_then(|snapshots| snapshots.get(&report.name))?;
                    let now = Local::now().timestamp();
                    if !runtime_usage_snapshot_is_usable(snapshot, now) {
                        return None;
                    }
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        RuntimeQuotaSource::PersistedSnapshot,
                    )
                }
            };
            if !collect_blocked_limits(&usage, include_code_review).is_empty() {
                return None;
            }

            Some(ReadyProfileCandidate {
                name: report.name.clone(),
                usage,
                order_index: report.order_index,
                preferred: preferred_profile == Some(report.name.as_str()),
                provider_priority: selection
                    .profile_entry(&report.name)
                    .map(ProfileSelectionProvider::runtime_pool_priority)
                    .unwrap_or(usize::MAX),
                quota_source,
            })
        })
        .collect::<Vec<_>>();

    schedule_ready_profile_candidates_with_view(candidates, selection, preferred_profile)
}

#[allow(dead_code)]
pub(crate) fn schedule_ready_profile_candidates(
    candidates: Vec<ReadyProfileCandidate>,
    state: &AppState,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    schedule_ready_profile_candidates_with_view(
        candidates,
        app_state_profile_selection_view(state),
        preferred_profile,
    )
}

pub(crate) fn schedule_ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    mut candidates: Vec<ReadyProfileCandidate>,
    selection: S,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let now = Local::now().timestamp();
    let best_provider_priority = candidates
        .iter()
        .map(|candidate| candidate.provider_priority)
        .min()
        .unwrap_or(usize::MAX);
    let best_total_pressure = candidates
        .iter()
        .filter(|candidate| candidate.provider_priority == best_provider_priority)
        .map(|candidate| ready_profile_score(candidate).total_pressure)
        .min()
        .unwrap_or(i64::MAX);

    candidates.sort_by_key(|candidate| {
        ready_profile_runtime_sort_key_with_view(
            candidate,
            selection,
            best_provider_priority,
            best_total_pressure,
            now,
        )
    });

    if let Some(preferred_name) = preferred_profile
        && let Some(preferred_index) = candidates.iter().position(|candidate| {
            candidate.name == preferred_name
                && !profile_in_run_selection_cooldown_with_view(selection, &candidate.name, now)
        })
    {
        let preferred_score = ready_profile_score(&candidates[preferred_index]).total_pressure;
        let selected_score = ready_profile_score(&candidates[0]).total_pressure;

        if preferred_index > 0
            && candidates[preferred_index].provider_priority == candidates[0].provider_priority
            && score_within_bps(
                preferred_score,
                selected_score,
                RUN_SELECTION_HYSTERESIS_BPS,
            )
        {
            let preferred_candidate = candidates.remove(preferred_index);
            candidates.insert(0, preferred_candidate);
        }
    }

    candidates
}

pub(crate) type ReadyProfileSortKey = (
    usize,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
    usize,
    usize,
    usize,
);

pub(crate) type ReadyProfileRuntimeSortKey = (usize, usize, usize, i64, ReadyProfileSortKey);

#[allow(dead_code)]
pub(crate) fn ready_profile_runtime_sort_key(
    candidate: &ReadyProfileCandidate,
    state: &AppState,
    best_provider_priority: usize,
    best_total_pressure: i64,
    now: i64,
) -> ReadyProfileRuntimeSortKey {
    ready_profile_runtime_sort_key_with_view(
        candidate,
        app_state_profile_selection_view(state),
        best_provider_priority,
        best_total_pressure,
        now,
    )
}

pub(crate) fn ready_profile_runtime_sort_key_with_view<S: ProfileSelectionRead>(
    candidate: &ReadyProfileCandidate,
    selection: S,
    best_provider_priority: usize,
    best_total_pressure: i64,
    now: i64,
) -> ReadyProfileRuntimeSortKey {
    let score = ready_profile_score(candidate);
    let near_optimal = candidate.provider_priority == best_provider_priority
        && score_within_bps(
            score.total_pressure,
            best_total_pressure,
            RUN_SELECTION_NEAR_OPTIMAL_BPS,
        );
    let recently_used = near_optimal
        && profile_in_run_selection_cooldown_with_view(selection, &candidate.name, now);
    let last_selected_at = if near_optimal {
        selection
            .last_run_selected_at(&candidate.name)
            .unwrap_or(i64::MIN)
    } else {
        i64::MIN
    };

    (
        candidate.provider_priority,
        if near_optimal { 0usize } else { 1usize },
        if recently_used { 1usize } else { 0usize },
        last_selected_at,
        ready_profile_sort_key(candidate),
    )
}

pub(crate) fn ready_profile_sort_key(candidate: &ReadyProfileCandidate) -> ReadyProfileSortKey {
    let score = ready_profile_score(candidate);

    (
        candidate.provider_priority,
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
        runtime_quota_source_sort_key(RuntimeRouteKind::Responses, candidate.quota_source),
        if candidate.preferred { 0usize } else { 1usize },
        candidate.order_index,
    )
}

pub(crate) fn ready_profile_score(candidate: &ReadyProfileCandidate) -> ReadyProfileScore {
    ready_profile_score_for_route(&candidate.usage, RuntimeRouteKind::Responses)
}

pub(crate) fn ready_profile_score_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot(usage, "weekly");
    let five_hour = required_main_window_snapshot(usage, "5h");

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias = match runtime_quota_pressure_band_for_route(usage, route_kind) {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 250_000,
        RuntimeQuotaPressureBand::Critical => 1_000_000,
        RuntimeQuotaPressureBand::Exhausted | RuntimeQuotaPressureBand::Unknown => i64::MAX / 4,
    };

    ReadyProfileScore {
        total_pressure: reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: weekly_remaining.min(five_hour_remaining),
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
        five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
    }
}

#[allow(dead_code)]
pub(crate) fn profile_in_run_selection_cooldown(
    state: &AppState,
    profile_name: &str,
    now: i64,
) -> bool {
    profile_in_run_selection_cooldown_with_view(
        app_state_profile_selection_view(state),
        profile_name,
        now,
    )
}

pub(crate) fn profile_in_run_selection_cooldown_with_view<S: ProfileSelectionRead>(
    selection: S,
    profile_name: &str,
    now: i64,
) -> bool {
    let Some(last_selected_at) = selection.last_run_selected_at(profile_name) else {
        return false;
    };

    now.saturating_sub(last_selected_at) < RUN_SELECTION_COOLDOWN_SECONDS
}

pub(crate) fn score_within_bps(candidate_score: i64, best_score: i64, bps: i64) -> bool {
    if candidate_score <= best_score {
        return true;
    }

    let lhs = i128::from(candidate_score).saturating_mul(10_000);
    let rhs = i128::from(best_score).saturating_mul(i128::from(10_000 + bps));
    lhs <= rhs
}

pub(crate) fn required_main_window_snapshot(
    usage: &UsageResponse,
    label: &str,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - Local::now().timestamp()).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

pub(crate) fn active_profile_selection_order(
    state: &AppState,
    current_profile: &str,
) -> Vec<String> {
    active_profile_selection_order_with_view(
        app_state_profile_selection_view(state),
        current_profile,
    )
}

pub(crate) fn active_profile_selection_order_with_view<S: ProfileSelectionRead>(
    selection: S,
    current_profile: &str,
) -> Vec<String> {
    provider_aware_profile_order_with_view(
        selection,
        std::iter::once(current_profile.to_string())
            .chain(profile_rotation_order_with_view(selection, current_profile)),
    )
}

pub(crate) fn map_parallel<I, O, F>(inputs: Vec<I>, func: F) -> Vec<O>
where
    I: Send,
    O: Send,
    F: Fn(I) -> O + Sync,
{
    if inputs.len() <= 1 {
        return inputs.into_iter().map(func).collect();
    }

    thread::scope(|scope| {
        let func = &func;
        let mut handles = Vec::with_capacity(inputs.len());
        for input in inputs {
            handles.push(scope.spawn(move || func(input)));
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel worker panicked"))
            .collect()
    })
}

pub(crate) fn find_ready_profiles(
    state: &AppState,
    current_profile: &str,
    base_url: Option<&str>,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> Vec<String> {
    ready_profile_candidates(
        &collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
            upstream_no_proxy,
        ),
        include_code_review,
        None,
        state,
        None,
    )
    .into_iter()
    .map(|candidate| candidate.name)
    .collect()
}

pub(crate) fn profile_rotation_order(state: &AppState, current_profile: &str) -> Vec<String> {
    profile_rotation_order_with_view(app_state_profile_selection_view(state), current_profile)
}

pub(crate) fn profile_rotation_order_with_view<S: ProfileSelectionRead>(
    selection: S,
    current_profile: &str,
) -> Vec<String> {
    let names = selection.profile_names();
    let Some(index) = names.iter().position(|name| name == current_profile) else {
        return provider_aware_profile_order_with_view(
            selection,
            names.into_iter().filter(|name| name != current_profile),
        );
    };

    provider_aware_profile_order_with_view(
        selection,
        names
            .iter()
            .skip(index + 1)
            .chain(names.iter().take(index))
            .cloned(),
    )
}

#[allow(dead_code)]
fn provider_aware_profile_order<I>(state: &AppState, names: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    provider_aware_profile_order_with_view(app_state_profile_selection_view(state), names)
}

fn provider_aware_profile_order_with_view<S: ProfileSelectionRead, I>(
    selection: S,
    names: I,
) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut ordered = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let provider_priority = selection
                .profile_entry(&name)
                .map(ProfileSelectionProvider::runtime_pool_priority)
                .unwrap_or(usize::MAX);
            (provider_priority, index, name)
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(provider_priority, index, _)| (*provider_priority, *index));
    ordered.into_iter().map(|(_, _, name)| name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct VectorSelectionView<'a> {
        entries: &'a [VectorSelectionEntry],
    }

    #[derive(Debug)]
    struct VectorSelectionEntry {
        name: &'static str,
        provider_priority: usize,
        last_run_selected_at: Option<i64>,
    }

    impl ProfileSelectionProvider for VectorSelectionEntry {
        fn runtime_pool_priority(&self) -> usize {
            self.provider_priority
        }
    }

    impl ProfileSelectionRead for VectorSelectionView<'_> {
        type Profile = VectorSelectionEntry;

        fn profile_names(&self) -> Vec<String> {
            self.entries
                .iter()
                .map(|entry| entry.name.to_string())
                .collect()
        }

        fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
            self.entries.iter().find(|entry| entry.name == name)
        }

        fn last_run_selected_at(&self, name: &str) -> Option<i64> {
            self.profile_entry(name)
                .and_then(|entry| entry.last_run_selected_at)
        }
    }

    fn test_usage(remaining: i64) -> UsageResponse {
        let now = Local::now().timestamp();
        let window = |remaining_percent| UsageWindow {
            used_percent: Some(100 - remaining_percent),
            reset_at: Some(now + 3_600),
            limit_window_seconds: Some(3_600),
        };
        UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(window(remaining)),
                secondary_window: Some(window(remaining)),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        }
    }

    #[test]
    fn profile_rotation_order_supports_custom_selection_view() {
        let selection = VectorSelectionView {
            entries: &[
                VectorSelectionEntry {
                    name: "bravo",
                    provider_priority: 0,
                    last_run_selected_at: None,
                },
                VectorSelectionEntry {
                    name: "alpha",
                    provider_priority: 1,
                    last_run_selected_at: None,
                },
                VectorSelectionEntry {
                    name: "charlie",
                    provider_priority: 0,
                    last_run_selected_at: None,
                },
            ],
        };

        assert_eq!(
            active_profile_selection_order_with_view(selection, "bravo"),
            vec![
                "bravo".to_string(),
                "charlie".to_string(),
                "alpha".to_string(),
            ]
        );
    }

    #[test]
    fn schedule_ready_profile_candidates_supports_custom_selection_view() {
        let now = Local::now().timestamp();
        let selection = VectorSelectionView {
            entries: &[
                VectorSelectionEntry {
                    name: "alpha",
                    provider_priority: 0,
                    last_run_selected_at: Some(now),
                },
                VectorSelectionEntry {
                    name: "bravo",
                    provider_priority: 0,
                    last_run_selected_at: Some(now - RUN_SELECTION_COOLDOWN_SECONDS - 5),
                },
            ],
        };
        let candidates = vec![
            ReadyProfileCandidate {
                name: "alpha".to_string(),
                usage: test_usage(80),
                order_index: 0,
                preferred: false,
                provider_priority: 0,
                quota_source: RuntimeQuotaSource::LiveProbe,
            },
            ReadyProfileCandidate {
                name: "bravo".to_string(),
                usage: test_usage(80),
                order_index: 1,
                preferred: false,
                provider_priority: 0,
                quota_source: RuntimeQuotaSource::LiveProbe,
            },
        ];

        let scheduled = schedule_ready_profile_candidates_with_view(candidates, selection, None);

        assert_eq!(
            scheduled
                .into_iter()
                .map(|candidate| candidate.name)
                .collect::<Vec<_>>(),
            vec!["bravo".to_string(), "alpha".to_string()]
        );
    }
}
