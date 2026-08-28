#[cfg(not(feature = "mojo"))]
use super::RUN_SELECTION_HYSTERESIS_BPS;
use super::{
    ProfileSelectionProvider, ProfileSelectionRead, RUN_SELECTION_COOLDOWN_SECONDS,
    RUN_SELECTION_NEAR_OPTIMAL_BPS,
};
use chrono::Local;
#[cfg(feature = "mojo")]
use prodex_mojo_core::runtime::{
    ProfileScheduleInput as MojoProfileScheduleInput, ProfileScoreInput as MojoProfileScoreInput,
    QuotaScoreInput as MojoQuotaScoreInput,
};
pub use prodex_quota::required_main_window_snapshot_at;
use prodex_quota::{
    RuntimeQuotaPressureBand, UsageResponse, scale_quota_pressure_for_plan,
    usage_plan_capacity_pressure_scale_bps,
};
use prodex_runtime_state::RuntimeRouteKind;
use prodex_shared_types::{ReadyProfileCandidate, ReadyProfileScore, RuntimeQuotaSource};
use std::cmp::Reverse;

pub fn schedule_ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    candidates: Vec<ReadyProfileCandidate>,
    selection: S,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let now = Local::now().timestamp();

    #[cfg(feature = "mojo")]
    {
        let inputs = candidates
            .iter()
            .map(|candidate| {
                let weekly = required_main_window_snapshot_at(&candidate.usage, "weekly", now);
                let five_hour = required_main_window_snapshot_at(&candidate.usage, "5h", now);
                let score = MojoProfileScoreInput {
                    weekly_pressure: weekly.map_or(i64::MAX, |window| window.pressure_score),
                    five_hour_pressure: five_hour.map_or(i64::MAX, |window| window.pressure_score),
                    scale_bps: usage_plan_capacity_pressure_scale_bps(&candidate.usage),
                    weekly_remaining: weekly.map_or(0, |window| window.remaining_percent),
                    five_hour_remaining: five_hour.map_or(0, |window| window.remaining_percent),
                    windows_complete: weekly.is_some() && five_hour.is_some(),
                    weekly_weight: 10,
                };
                MojoProfileScheduleInput {
                    score,
                    provider_priority: i64::try_from(candidate.provider_priority)
                        .expect("profile priority fits ABI"),
                    in_selection_cooldown: profile_in_run_selection_cooldown_with_view(
                        selection,
                        &candidate.name,
                        now,
                    ),
                    last_selected_at: selection
                        .last_run_selected_at(&candidate.name)
                        .unwrap_or(i64::MIN),
                    weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
                    five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
                    quota_source: i64::try_from(runtime_quota_source_sort_key(
                        RuntimeRouteKind::Responses,
                        candidate.quota_source,
                    ))
                    .expect("quota source sort key fits ABI"),
                    preferred: candidate.preferred,
                    affinity_preferred: preferred_profile == Some(candidate.name.as_str()),
                    order_index: i64::try_from(candidate.order_index)
                        .expect("profile order index fits ABI"),
                }
            })
            .collect::<Vec<_>>();
        let order = prodex_mojo_core::runtime::profile_schedule_batch(&inputs)
            .expect("Mojo runtime profile schedule returned invalid output");
        let mut slots = candidates.into_iter().map(Some).collect::<Vec<_>>();
        order
            .into_iter()
            .map(|index| {
                slots[index]
                    .take()
                    .expect("Mojo profile schedule index is unique")
            })
            .collect()
    }

    #[cfg(not(feature = "mojo"))]
    {
        let scores =
            ready_profile_scores_for_candidates(&candidates, RuntimeRouteKind::Responses, now);
        let mut scored_candidates = candidates.into_iter().zip(scores).collect::<Vec<_>>();
        let best_provider_priority = scored_candidates
            .iter()
            .map(|(candidate, _)| candidate.provider_priority)
            .min()
            .unwrap_or(usize::MAX);
        let best_total_pressure = scored_candidates
            .iter()
            .filter(|(candidate, _)| candidate.provider_priority == best_provider_priority)
            .map(|(_, score)| score.total_pressure)
            .min()
            .unwrap_or(i64::MAX);

        scored_candidates.sort_by_key(|(candidate, score)| {
            ready_profile_runtime_sort_key_from_score(
                candidate,
                selection,
                best_provider_priority,
                best_total_pressure,
                now,
                *score,
            )
        });

        if let Some(preferred_name) = preferred_profile
            && let Some(preferred_index) = scored_candidates.iter().position(|(candidate, _)| {
                candidate.name == preferred_name
                    && !profile_in_run_selection_cooldown_with_view(selection, &candidate.name, now)
            })
        {
            let preferred_score = scored_candidates[preferred_index].1.total_pressure;
            let selected_score = scored_candidates[0].1.total_pressure;

            if preferred_index > 0
                && scored_candidates[preferred_index].0.provider_priority
                    == scored_candidates[0].0.provider_priority
                && score_within_bps(
                    preferred_score,
                    selected_score,
                    RUN_SELECTION_HYSTERESIS_BPS,
                )
            {
                let preferred_candidate = scored_candidates.remove(preferred_index);
                scored_candidates.insert(0, preferred_candidate);
            }
        }

        scored_candidates
            .into_iter()
            .map(|(candidate, _)| candidate)
            .collect()
    }
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
    ready_profile_runtime_sort_key_from_score(
        candidate,
        selection,
        best_provider_priority,
        best_total_pressure,
        now,
        score,
    )
}

fn ready_profile_runtime_sort_key_from_score<S: ProfileSelectionRead>(
    candidate: &ReadyProfileCandidate,
    selection: S,
    best_provider_priority: usize,
    best_total_pressure: i64,
    now: i64,
    score: ReadyProfileScore,
) -> ReadyProfileRuntimeSortKey {
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
        ready_profile_sort_key_from_score(candidate, score),
    )
}

pub fn ready_profile_sort_key(candidate: &ReadyProfileCandidate) -> ReadyProfileSortKey {
    let score = ready_profile_score(candidate);
    ready_profile_sort_key_from_score(candidate, score)
}

fn ready_profile_sort_key_from_score(
    candidate: &ReadyProfileCandidate,
    score: ReadyProfileScore,
) -> ReadyProfileSortKey {
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

#[cfg(not(feature = "mojo"))]
fn ready_profile_scores_for_candidates(
    candidates: &[ReadyProfileCandidate],
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Vec<ReadyProfileScore> {
    candidates
        .iter()
        .map(|candidate| ready_profile_score_for_route_at(&candidate.usage, route_kind, now))
        .collect()
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
    #[cfg(feature = "mojo")]
    {
        return ready_profile_score_for_route_at_mojo(usage, route_kind, now);
    }

    #[cfg(not(feature = "mojo"))]
    ready_profile_score_for_route_at_rust(usage, route_kind, now)
}

#[cfg(feature = "mojo")]
fn ready_profile_score_for_route_at_mojo(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot_at(usage, "weekly", now);
    let five_hour = required_main_window_snapshot_at(usage, "5h", now);

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let plan_pressure_scale_bps = usage_plan_capacity_pressure_scale_bps(usage);
    let scaled_weekly_pressure =
        scale_quota_pressure_for_plan(weekly_pressure, plan_pressure_scale_bps);
    let scaled_five_hour_pressure =
        scale_quota_pressure_for_plan(five_hour_pressure, plan_pressure_scale_bps);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let reserve_bias = match runtime_quota_pressure_band_for_route_at(usage, route_kind, now) {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 250_000,
        RuntimeQuotaPressureBand::Critical => 1_000_000,
        RuntimeQuotaPressureBand::Exhausted | RuntimeQuotaPressureBand::Unknown => i64::MAX / 4,
    };

    let score = prodex_mojo_core::runtime::quota_score_batch(
        &[MojoQuotaScoreInput {
            weekly_pressure: scaled_weekly_pressure,
            five_hour_pressure: scaled_five_hour_pressure,
            weekly_remaining,
            five_hour_remaining,
            weekly_has_value: weekly.is_some(),
            five_hour_has_value: five_hour.is_some(),
            weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
            five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
        }],
        match route_kind {
            RuntimeRouteKind::Responses => 0,
            RuntimeRouteKind::Compact => 1,
            RuntimeRouteKind::Websocket => 2,
            RuntimeRouteKind::Standard => 3,
        },
    )
    .expect("Mojo runtime quota score returned invalid output")
    .into_iter()
    .next()
    .expect("Mojo runtime quota score returned no row");
    debug_assert_eq!(
        score.pressure_band,
        match reserve_bias {
            0 => 0,
            250_000 => 1,
            1_000_000 => 2,
            _ => 3,
        }
    );
    ReadyProfileScore {
        total_pressure: score.total_pressure,
        weekly_pressure: score.weekly_pressure,
        five_hour_pressure: score.five_hour_pressure,
        reserve_floor: score.reserve_floor,
        weekly_remaining: score.weekly_remaining,
        five_hour_remaining: score.five_hour_remaining,
        weekly_reset_at: score.weekly_reset_at,
        five_hour_reset_at: score.five_hour_reset_at,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn ready_profile_score_for_route_at_rust(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot_at(usage, "weekly", now);
    let five_hour = required_main_window_snapshot_at(usage, "5h", now);

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let plan_pressure_scale_bps = usage_plan_capacity_pressure_scale_bps(usage);
    let scaled_weekly_pressure =
        scale_quota_pressure_for_plan(weekly_pressure, plan_pressure_scale_bps);
    let scaled_five_hour_pressure =
        scale_quota_pressure_for_plan(five_hour_pressure, plan_pressure_scale_bps);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias = match runtime_quota_pressure_band_for_route_at_rust(usage, route_kind, now) {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 250_000,
        RuntimeQuotaPressureBand::Critical => 1_000_000,
        RuntimeQuotaPressureBand::Exhausted | RuntimeQuotaPressureBand::Unknown => i64::MAX / 4,
    };

    ReadyProfileScore {
        total_pressure: reserve_bias
            .saturating_add(scaled_weekly_pressure.saturating_mul(weekly_weight))
            .saturating_add(scaled_five_hour_pressure),
        weekly_pressure: scaled_weekly_pressure,
        five_hour_pressure: scaled_five_hour_pressure,
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
    #[cfg(feature = "mojo")]
    {
        let Some(weekly) = required_main_window_snapshot_at(usage, "weekly", now) else {
            return RuntimeQuotaPressureBand::Unknown;
        };
        let Some(five_hour) = required_main_window_snapshot_at(usage, "5h", now) else {
            return RuntimeQuotaPressureBand::Unknown;
        };
        return match prodex_mojo_core::runtime::pressure_band_for_route(
            Some((five_hour.remaining_percent, 1)),
            Some((weekly.remaining_percent, 1)),
            match route_kind {
                RuntimeRouteKind::Responses => 0,
                RuntimeRouteKind::Compact => 1,
                RuntimeRouteKind::Websocket => 2,
                RuntimeRouteKind::Standard => 3,
            },
        )
        .expect("Mojo runtime quota pressure band returned invalid output")
        {
            0 => RuntimeQuotaPressureBand::Healthy,
            1 => RuntimeQuotaPressureBand::Thin,
            2 => RuntimeQuotaPressureBand::Critical,
            3 => RuntimeQuotaPressureBand::Exhausted,
            _ => RuntimeQuotaPressureBand::Unknown,
        };
    }

    #[cfg(not(feature = "mojo"))]
    runtime_quota_pressure_band_for_route_at_rust(usage, route_kind, now)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_quota_pressure_band_for_route_at_rust(
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
