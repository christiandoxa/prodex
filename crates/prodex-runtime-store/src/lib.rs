//! Runtime store merge, compaction, and small persisted cache helpers.
//!
//! Runtime state save orchestration stays in the binary crate. This crate keeps
//! reusable merge/retention primitives and the smart-context artifact JSON cache
//! boundary.

use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeContinuationStore, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeProfileUsageSnapshot, RuntimeRouteKind, RuntimeStateSaveSections,
    RuntimeStateSaveSelectedSnapshot, RuntimeStateSaveStateSection,
};
use prodex_state::{
    AppState, AppStateCompactionPolicy, ProfileEntry, ResponseProfileBinding,
    merge_profile_bindings, prune_profile_bindings,
    prune_profile_bindings_for_housekeeping_without_retention,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

mod continuations;
mod selected_snapshot;
mod smart_context_store;

pub use continuations::*;
pub use selected_snapshot::*;
pub use smart_context_store::*;

pub const RUNTIME_SCORE_RETENTION_SECONDS: i64 = if cfg!(test) { 120 } else { 14 * 24 * 60 * 60 };
pub const RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };
pub const RUNTIME_PROFILE_HEALTH_DECAY_SECONDS: i64 = if cfg!(test) { 2 } else { 60 };
pub const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
pub const RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
pub const RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 15 };
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS: i64 = 20;
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS: i64 = if cfg!(test) { 320 } else { 600 };
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS: i64 =
    if cfg!(test) { 20 } else { 60 };
pub const RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS: i64 = if cfg!(test) { 12 } else { 1_800 };
pub const RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE: u32 = 4;
pub const RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION: u32 = 1;
pub const RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_BYTES: usize = 1_024;
pub const RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_TOKENS: usize = 256;

pub fn merge_runtime_state_snapshot_for_save(
    existing: AppState,
    snapshot: &AppState,
    now: i64,
    policy: AppStateCompactionPolicy,
) -> AppState {
    prodex_state::merge_runtime_state_snapshot_with_policy(existing, snapshot, now, policy)
}

pub fn merge_runtime_state_and_continuations_for_save(
    existing_state: AppState,
    state_snapshot: &AppState,
    existing_continuations: &RuntimeContinuationStore<ResponseProfileBinding>,
    continuation_snapshot: &RuntimeContinuationStore<ResponseProfileBinding>,
    now: i64,
    state_policy: AppStateCompactionPolicy,
    continuation_policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeMergedStateAndContinuations {
    let mut state =
        merge_runtime_state_snapshot_for_save(existing_state, state_snapshot, now, state_policy);
    let continuations = merge_runtime_continuation_store(
        existing_continuations,
        continuation_snapshot,
        &state.profiles,
        now,
        continuation_policy,
    );
    state.response_profile_bindings =
        runtime_external_response_profile_bindings(&continuations.response_profile_bindings);
    state.session_profile_bindings = continuations.session_profile_bindings.clone();
    RuntimeMergedStateAndContinuations {
        state,
        continuations,
    }
}

pub fn compact_runtime_usage_snapshots<W>(
    mut snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let oldest_allowed = now.saturating_sub(RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS);
    snapshots.retain(|profile_name, snapshot| {
        profiles.contains_key(profile_name) && snapshot.checked_at >= oldest_allowed
    });
    snapshots
}

pub fn merge_runtime_usage_snapshots<W: Clone>(
    existing: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    incoming: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let mut merged = existing.clone();
    for (profile_name, snapshot) in incoming {
        let should_replace = merged
            .get(profile_name)
            .is_none_or(|current| current.checked_at <= snapshot.checked_at);
        if should_replace {
            merged.insert(profile_name.clone(), snapshot.clone());
        }
    }
    compact_runtime_usage_snapshots(merged, profiles, now)
}

pub fn compact_runtime_profile_scores(
    mut scores: BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let oldest_allowed = now.saturating_sub(RUNTIME_SCORE_RETENTION_SECONDS);
    scores.retain(|key, value| {
        profiles.contains_key(runtime_profile_score_profile_name(key))
            && value.updated_at >= oldest_allowed
    });
    scores
}

pub fn merge_runtime_profile_scores(
    existing: &BTreeMap<String, RuntimeProfileHealth>,
    incoming: &BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let mut merged = existing.clone();
    for (key, value) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| current.updated_at <= value.updated_at);
        if should_replace {
            merged.insert(key.clone(), value.clone());
        }
    }
    compact_runtime_profile_scores(merged, profiles, now)
}

pub fn runtime_profile_score_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub fn compact_runtime_profile_backoffs(
    mut backoffs: RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    backoffs
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    backoffs.transport_backoff_until.retain(|key, until| {
        runtime_profile_transport_backoff_key_matches_profiles(key, profiles) && *until > now
    });
    backoffs
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    backoffs
}

pub fn merge_runtime_profile_backoffs(
    existing: &RuntimeProfileBackoffs,
    incoming: &RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    let mut merged = existing.clone();
    for (profile_name, until) in &incoming.retry_backoff_until {
        merged
            .retry_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (profile_name, until) in &incoming.transport_backoff_until {
        merged
            .transport_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (route_profile_key, until) in &incoming.route_circuit_open_until {
        merged
            .route_circuit_open_until
            .insert(route_profile_key.clone(), *until);
    }
    compact_runtime_profile_backoffs(merged, profiles, now)
}

pub fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses",
        RuntimeRouteKind::Compact => "compact",
        RuntimeRouteKind::Websocket => "websocket",
        RuntimeRouteKind::Standard => "standard",
    }
}

pub fn runtime_route_kind_from_label(label: &str) -> Option<RuntimeRouteKind> {
    match label {
        "responses" => Some(RuntimeRouteKind::Responses),
        "compact" => Some(RuntimeRouteKind::Compact),
        "websocket" => Some(RuntimeRouteKind::Websocket),
        "standard" => Some(RuntimeRouteKind::Standard),
        _ => None,
    }
}

pub fn runtime_route_coupled_kinds(route_kind: RuntimeRouteKind) -> &'static [RuntimeRouteKind] {
    match route_kind {
        RuntimeRouteKind::Responses => &[RuntimeRouteKind::Websocket],
        RuntimeRouteKind::Websocket => &[RuntimeRouteKind::Responses],
        RuntimeRouteKind::Compact => &[RuntimeRouteKind::Standard],
        RuntimeRouteKind::Standard => &[RuntimeRouteKind::Compact],
    }
}

pub fn runtime_profile_effective_health_score(entry: &RuntimeProfileHealth, now: i64) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

pub fn runtime_profile_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

pub fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

pub fn runtime_profile_effective_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

pub fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_success__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    key.replacen("__route_circuit__", "__route_health__", 1)
}

pub fn runtime_profile_route_circuit_reopen_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit_reopen__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_global_health_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
) -> u32 {
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
}

pub fn runtime_profile_route_health_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(
        profile_health,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    )
}

pub fn runtime_profile_route_coupling_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_from_map(
                profile_health,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_bad_pairing_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            route_score
                .saturating_add(bad_pairing_score)
                .saturating_div(2)
        })
        .fold(0, u32::saturating_add)
}

pub fn runtime_profile_route_performance_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    let route_score = runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_performance_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
            .saturating_div(2)
        })
        .fold(0, u32::saturating_add);
    route_score.saturating_add(coupled_score)
}

pub fn runtime_profile_health_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_global_health_score(profile_health, profile_name, now)
        .saturating_add(runtime_profile_route_health_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_coupling_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

pub fn runtime_profile_health_sort_key(
    profile_name: &str,
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
        .saturating_add(runtime_profile_route_health_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_effective_score_from_map(
            profile_health,
            &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
            now,
            RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        ))
        .saturating_add(runtime_profile_route_coupling_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

pub fn runtime_previous_response_negative_cache_key(
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__previous_response_not_found__:{}:{}:{profile_name}",
        runtime_route_kind_label(route_kind),
        previous_response_id
    )
}

pub fn runtime_previous_response_negative_cache_failures(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_previous_response_negative_cache_key(
            previous_response_id,
            profile_name,
            route_kind,
        ),
        now,
        decay_seconds,
    )
}

pub fn runtime_previous_response_negative_cache_active(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
    decay_seconds: i64,
) -> bool {
    runtime_previous_response_negative_cache_failures(
        profile_health,
        previous_response_id,
        profile_name,
        route_kind,
        now,
        decay_seconds,
    ) > 0
}

pub fn clear_runtime_previous_response_negative_cache(
    profile_health: &mut BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
) -> bool {
    let mut changed = false;
    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Standard,
    ] {
        changed = profile_health
            .remove(&runtime_previous_response_negative_cache_key(
                previous_response_id,
                profile_name,
                route_kind,
            ))
            .is_some()
            || changed;
    }
    changed
}

pub fn runtime_profile_route_key_parts<'a>(
    key: &'a str,
    prefix: &str,
) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

pub fn runtime_profile_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_transport_backoff__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_profile_route_key_parts(key, "__route_transport_backoff__:")
}

pub fn runtime_profile_transport_backoff_profile_name(key: &str) -> &str {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(_, profile_name)| profile_name)
        .unwrap_or(key)
}

pub fn runtime_profile_transport_backoff_key_valid(
    key: &str,
    valid_profiles: &BTreeSet<String>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && valid_profiles.contains(profile_name)
        })
        .unwrap_or_else(|| valid_profiles.contains(key))
}

pub fn runtime_profile_transport_backoff_until_from_map(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    [
        transport_backoff_until.get(&route_key).copied(),
        transport_backoff_until.get(profile_name).copied(),
    ]
    .into_iter()
    .flatten()
    .filter(|until| *until > now)
    .max()
}

pub fn runtime_profile_transport_backoff_max_until(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    now: i64,
) -> Option<i64> {
    transport_backoff_until
        .iter()
        .filter(|(key, until)| {
            runtime_profile_transport_backoff_profile_name(key) == profile_name && **until > now
        })
        .map(|(_, until)| *until)
        .max()
}

pub fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
        || runtime_profile_transport_backoff_until_from_map(
            transport_backoff_until,
            profile_name,
            route_kind,
            now,
        )
        .is_some()
        || route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
            .copied()
            .is_some_and(|until| until > now)
}

pub fn runtime_profile_backoff_sort_key(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (usize, i64, i64, i64) {
    let retry_until = retry_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let transport_until = runtime_profile_transport_backoff_until_from_map(
        transport_backoff_until,
        profile_name,
        route_kind,
        now,
    );
    let circuit_until = route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now);

    match (circuit_until, transport_until, retry_until) {
        (None, None, None) => (0, 0, 0, 0),
        (Some(circuit_until), None, None) => (1, circuit_until, 0, 0),
        (None, Some(transport_until), None) => (2, transport_until, 0, 0),
        (None, None, Some(retry_until)) => (3, retry_until, 0, 0),
        (Some(circuit_until), Some(transport_until), None) => (
            4,
            circuit_until.min(transport_until),
            circuit_until.max(transport_until),
            0,
        ),
        (Some(circuit_until), None, Some(retry_until)) => (
            5,
            circuit_until.min(retry_until),
            circuit_until.max(retry_until),
            0,
        ),
        (None, Some(transport_until), Some(retry_until)) => (
            6,
            transport_until.min(retry_until),
            transport_until.max(retry_until),
            0,
        ),
        (Some(circuit_until), Some(transport_until), Some(retry_until)) => (
            7,
            circuit_until.min(transport_until.min(retry_until)),
            circuit_until.max(transport_until.max(retry_until)),
            retry_until,
        ),
    }
}

pub fn runtime_soften_persisted_backoff_map_for_startup(
    backoffs: &mut BTreeMap<String, i64>,
    now: i64,
    max_future_seconds: i64,
) -> bool {
    let max_until = now.saturating_add(max_future_seconds.max(0));
    let mut changed = false;
    backoffs.retain(|_, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub fn runtime_profile_circuit_open_seconds(score: u32, reopen_stage: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3)
                .saturating_add(reopen_stage.min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS)
}

pub fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS)
}

pub fn runtime_profile_route_circuit_probe_seconds(
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    route_profile_key: &str,
    now: i64,
) -> i64 {
    let Some((route_label, profile_name)) =
        runtime_profile_route_key_parts(route_profile_key, "__route_circuit__:")
    else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let Some(route_kind) = runtime_route_kind_from_label(route_label) else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let score = runtime_profile_effective_health_score_from_map(
        profile_scores,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    );
    runtime_profile_circuit_half_open_probe_seconds(score)
}

pub fn runtime_soften_persisted_route_circuits_for_startup(
    route_circuit_open_until: &mut BTreeMap<String, i64>,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = false;
    route_circuit_open_until.retain(|route_profile_key, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let max_until = now.saturating_add(runtime_profile_route_circuit_probe_seconds(
            profile_scores,
            route_profile_key,
            now,
        ));
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub fn runtime_soften_persisted_backoffs_for_startup(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = runtime_soften_persisted_backoff_map_for_startup(
        &mut backoffs.transport_backoff_until,
        now,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

fn runtime_profile_transport_backoff_key_matches_profiles(
    key: &str,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && profiles.contains_key(profile_name)
        })
        .unwrap_or_else(|| profiles.contains_key(key))
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
