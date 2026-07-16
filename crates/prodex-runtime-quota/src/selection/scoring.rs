use super::{
    ProfileSelectionProvider, ProfileSelectionRead, RUN_SELECTION_COOLDOWN_SECONDS,
    RUN_SELECTION_HYSTERESIS_BPS, RUN_SELECTION_NEAR_OPTIMAL_BPS,
};
use chrono::Local;
use prodex_quota::{
    MainWindowSnapshot, RuntimeQuotaPressureBand, UsageResponse, find_main_window,
    remaining_percent, scale_quota_pressure_for_plan, usage_plan_capacity_pressure_scale_bps,
};
use prodex_runtime_state::RuntimeRouteKind;
use prodex_shared_types::{ReadyProfileCandidate, ReadyProfileScore, RuntimeQuotaSource};
use std::cmp::Reverse;

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
    let reserve_bias = match runtime_quota_pressure_band_for_route_at(usage, route_kind, now) {
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

pub fn required_main_window_snapshot_at(
    usage: &UsageResponse,
    label: &str,
    now: i64,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - now).max(0)
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
