use crate::info::{
    RuntimeProfileUsageSnapshot, required_main_window_snapshot_at,
    runtime_usage_snapshot_is_usable, usage_from_runtime_usage_snapshot,
};
use chrono::Local;
use prodex_quota::{RuntimeQuotaPressureBand, UsageResponse, collect_blocked_limits};
use prodex_runtime_state::RuntimeRouteKind;
use prodex_shared_types::{
    ReadyProfileCandidate, ReadyProfileScore, RunProfileProbeReport, RuntimeQuotaSource,
};
use prodex_state::ProfileEntry;
use std::cmp::Reverse;
use std::collections::BTreeMap;

pub const RUN_SELECTION_NEAR_OPTIMAL_BPS: i64 = 1_000;
pub const RUN_SELECTION_HYSTERESIS_BPS: i64 = 500;
pub const RUN_SELECTION_COOLDOWN_SECONDS: i64 = 15 * 60;

pub trait ProfileSelectionProvider {
    fn runtime_pool_priority(&self) -> usize;
}

impl ProfileSelectionProvider for ProfileEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider.runtime_pool_priority()
    }
}

pub trait ProfileSelectionRead: Copy {
    type Profile: ProfileSelectionProvider;

    fn profile_names(&self) -> Vec<String>;
    fn profile_entry(&self, name: &str) -> Option<&Self::Profile>;
    fn last_run_selected_at(&self, name: &str) -> Option<i64>;
}

pub struct ProfileSelectionView<'a, P> {
    pub profiles: &'a BTreeMap<String, P>,
    pub last_run_selected_at: &'a BTreeMap<String, i64>,
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

pub fn ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    selection: S,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    stale_grace_seconds: i64,
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
                    if !runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds) {
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

pub fn schedule_ready_profile_candidates_with_view<S: ProfileSelectionRead>(
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

pub type ReadyProfileSortKey = (
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

pub type ReadyProfileRuntimeSortKey = (usize, usize, usize, i64, ReadyProfileSortKey);

pub fn ready_profile_runtime_sort_key_with_view<S: ProfileSelectionRead>(
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

pub fn ready_profile_sort_key(candidate: &ReadyProfileCandidate) -> ReadyProfileSortKey {
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

pub fn ready_profile_score(candidate: &ReadyProfileCandidate) -> ReadyProfileScore {
    ready_profile_score_for_route(&candidate.usage, RuntimeRouteKind::Responses)
}

pub fn ready_profile_score_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> ReadyProfileScore {
    ready_profile_score_for_route_at(usage, route_kind, Local::now().timestamp())
}

pub fn ready_profile_score_for_route_at(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot_at(usage, "weekly", now);
    let five_hour = required_main_window_snapshot_at(usage, "5h", now);

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias = match runtime_quota_pressure_band_for_route_at(usage, route_kind, now) {
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

pub fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    runtime_quota_pressure_band_for_route_at(usage, route_kind, Local::now().timestamp())
}

pub fn runtime_quota_pressure_band_for_route_at(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeQuotaPressureBand {
    let Some(weekly) = required_main_window_snapshot_at(usage, "weekly", now) else {
        return RuntimeQuotaPressureBand::Unknown;
    };
    let Some(five_hour) = required_main_window_snapshot_at(usage, "5h", now) else {
        return RuntimeQuotaPressureBand::Unknown;
    };

    let weekly_remaining = weekly.remaining_percent;
    let five_hour_remaining = five_hour.remaining_percent;
    if weekly_remaining == 0 || five_hour_remaining == 0 {
        return RuntimeQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };

    if weekly_remaining <= critical_weekly || five_hour_remaining <= critical_five_hour {
        RuntimeQuotaPressureBand::Critical
    } else if weekly_remaining <= thin_weekly || five_hour_remaining <= thin_five_hour {
        RuntimeQuotaPressureBand::Thin
    } else {
        RuntimeQuotaPressureBand::Healthy
    }
}

pub fn runtime_quota_source_sort_key(
    route_kind: RuntimeRouteKind,
    source: RuntimeQuotaSource,
) -> usize {
    match (route_kind, source) {
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::LiveProbe,
        ) => 0,
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::PersistedSnapshot,
        ) => 1,
        _ => 0,
    }
}

pub fn profile_in_run_selection_cooldown_with_view<S: ProfileSelectionRead>(
    selection: S,
    profile_name: &str,
    now: i64,
) -> bool {
    let Some(last_selected_at) = selection.last_run_selected_at(profile_name) else {
        return false;
    };

    now.saturating_sub(last_selected_at) < RUN_SELECTION_COOLDOWN_SECONDS
}

pub fn score_within_bps(candidate_score: i64, best_score: i64, bps: i64) -> bool {
    if candidate_score <= best_score {
        return true;
    }

    let lhs = i128::from(candidate_score).saturating_mul(10_000);
    let rhs = i128::from(best_score).saturating_mul(i128::from(10_000 + bps));
    lhs <= rhs
}

pub fn active_profile_selection_order_with_view<S: ProfileSelectionRead>(
    selection: S,
    current_profile: &str,
) -> Vec<String> {
    provider_aware_profile_order_with_view(
        selection,
        std::iter::once(current_profile.to_string())
            .chain(profile_rotation_order_with_view(selection, current_profile)),
    )
}

pub fn profile_rotation_order_with_view<S: ProfileSelectionRead>(
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

pub fn provider_aware_profile_order_with_view<S: ProfileSelectionRead, I>(
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
    use prodex_quota::{UsageWindow, WindowPair};

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
