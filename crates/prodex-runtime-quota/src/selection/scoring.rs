use super::{ProfileSelectionProvider, ProfileSelectionRead, RUN_SELECTION_COOLDOWN_SECONDS};
use chrono::Local;
use prodex_mojo_core::runtime::{
    ProfileScheduleInput as MojoProfileScheduleInput, ProfileScoreInput as MojoProfileScoreInput,
    QuotaRouteScoreInput as MojoQuotaRouteScoreInput,
};
pub use prodex_quota::required_main_window_snapshot_at;
use prodex_quota::{
    RuntimeQuotaPressureBand, UsageResponse, usage_plan_capacity_pressure_scale_bps,
};
use prodex_runtime_state::RuntimeRouteKind;
use prodex_shared_types::{ReadyProfileCandidate, ReadyProfileScore, RuntimeQuotaSource};

pub fn schedule_ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    candidates: Vec<ReadyProfileCandidate>,
    selection: S,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    schedule_ready_profile_candidates_with_view_for_model(
        candidates,
        selection,
        preferred_profile,
        None,
    )
}

pub fn schedule_ready_profile_candidates_with_view_for_model<S: ProfileSelectionRead>(
    candidates: Vec<ReadyProfileCandidate>,
    selection: S,
    preferred_profile: Option<&str>,
    requested_model: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    schedule_ready_profile_candidates_with_view_for_model_at(
        candidates,
        selection,
        preferred_profile,
        requested_model,
        Local::now().timestamp(),
    )
}

fn schedule_ready_profile_candidates_with_view_for_model_at<S: ProfileSelectionRead>(
    candidates: Vec<ReadyProfileCandidate>,
    selection: S,
    preferred_profile: Option<&str>,
    requested_model: Option<&str>,
    now: i64,
) -> Vec<ReadyProfileCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let inputs = candidates
        .iter()
        .map(|candidate| {
            let weekly =
                ready_profile_window_snapshot_at(&candidate.usage, "weekly", requested_model, now);
            let five_hour =
                ready_profile_window_snapshot_at(&candidate.usage, "5h", requested_model, now);
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

fn ready_profile_window_snapshot_at(
    usage: &UsageResponse,
    label: &str,
    requested_model: Option<&str>,
    now: i64,
) -> Option<prodex_quota::MainWindowSnapshot> {
    let pair = match requested_model {
        Some(model) => prodex_quota::openai_quota_runtime_window_pair_for_model(usage, Some(model)),
        None => prodex_quota::openai_quota_runtime_window_pair(usage),
    }?;
    prodex_quota::required_window_snapshot_for_pair_at(pair, label, now)
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
    ready_profile_score_for_route_at_mojo(usage, route_kind, now)
}

fn ready_profile_score_for_route_at_mojo(
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

    let score = prodex_mojo_core::runtime::quota_route_score_batch(
        &[MojoQuotaRouteScoreInput {
            weekly_pressure,
            five_hour_pressure,
            scale_bps: usage_plan_capacity_pressure_scale_bps(usage),
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
    match prodex_mojo_core::runtime::pressure_band_for_route(
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

pub fn active_profile_selection_order_with_view<S: ProfileSelectionRead>(
    selection: S,
    current_profile: &str,
) -> Vec<String> {
    let names = selection.profile_names();
    profile_selection_order_with_mojo(selection, names, current_profile, true)
}

pub fn profile_rotation_order_with_view<S: ProfileSelectionRead>(
    selection: S,
    current_profile: &str,
) -> Vec<String> {
    let names = selection.profile_names();
    profile_selection_order_with_mojo(selection, names, current_profile, false)
}

fn profile_selection_order_with_mojo<S: ProfileSelectionRead>(
    selection: S,
    names: Vec<String>,
    current_profile: &str,
    include_current: bool,
) -> Vec<String> {
    let priorities = names
        .iter()
        .map(|name| {
            selection
                .profile_entry(name)
                .map(ProfileSelectionProvider::runtime_pool_priority)
                .unwrap_or(usize::MAX)
        })
        .collect::<Vec<_>>();
    let current_index = names.iter().position(|name| name == current_profile);
    let order = prodex_mojo_core::runtime::profile_selection_order_batch(
        &priorities,
        current_index,
        include_current,
    )
    .expect("Mojo profile rotation order returned invalid output");
    order
        .into_iter()
        .map(|index| {
            if index == names.len() {
                current_profile.to_string()
            } else {
                names[index].clone()
            }
        })
        .collect()
}

#[path = "scoring/profile_order.rs"]
mod profile_order;
pub use profile_order::provider_aware_profile_order_with_view;
