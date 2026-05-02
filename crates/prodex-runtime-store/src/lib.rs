//! Runtime store merge and compaction helpers.
//!
//! This crate owns side-effect-free state pruning and merge rules only. File
//! reads, atomic writes, and generation fences stay in the binary crate.

use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeContinuationStore, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeProfileUsageSnapshot, RuntimeRouteKind,
};
use prodex_state::{
    AppState, ProfileEntry, ResponseProfileBinding, merge_profile_bindings, prune_profile_bindings,
    prune_profile_bindings_for_housekeeping_without_retention,
};
use std::collections::{BTreeMap, BTreeSet};

pub const RUNTIME_SCORE_RETENTION_SECONDS: i64 = if cfg!(test) { 120 } else { 14 * 24 * 60 * 60 };
pub const RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };

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

fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
pub const RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX: &str = "__response_turn_state__:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationCompactionPolicy {
    pub response_binding_limit: usize,
    pub turn_state_binding_limit: usize,
    pub session_id_binding_limit: usize,
    pub response_status_limit: usize,
    pub turn_state_status_limit: usize,
    pub session_id_status_limit: usize,
    pub suspect_grace_seconds: i64,
    pub dead_grace_seconds: i64,
    pub verified_stale_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub confidence_max: u32,
}

impl Default for RuntimeContinuationCompactionPolicy {
    fn default() -> Self {
        Self {
            response_binding_limit: 16_384,
            turn_state_binding_limit: 2_048,
            session_id_binding_limit: 2_048,
            response_status_limit: 16_384,
            turn_state_status_limit: 2_048,
            session_id_status_limit: 2_048,
            suspect_grace_seconds: 120,
            dead_grace_seconds: 900,
            verified_stale_seconds: 1_800,
            suspect_not_found_streak_limit: 2,
            confidence_max: 8,
        }
    }
}

pub fn runtime_continuation_store_from_app_state(
    state: &AppState,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    RuntimeContinuationStore {
        response_profile_bindings: runtime_external_response_profile_bindings(
            &state.response_profile_bindings,
        ),
        session_profile_bindings: state.session_profile_bindings.clone(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: runtime_external_session_id_bindings(&state.session_profile_bindings),
        statuses: RuntimeContinuationStatuses::default(),
    }
}

pub fn runtime_is_response_turn_state_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)
}

pub fn runtime_response_turn_state_lineage_parts(key: &str) -> Option<(&str, &str)> {
    let suffix = key.strip_prefix(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)?;
    let (response_len, rest) = suffix.split_once(':')?;
    let response_len = response_len.parse::<usize>().ok()?;
    let response_and_sep = rest.get(..response_len.saturating_add(1))?;
    if response_and_sep.as_bytes().get(response_len).copied() != Some(b':') {
        return None;
    }
    let response_id = response_and_sep.get(..response_len)?;
    let turn_state = rest.get(response_len.saturating_add(1)..)?;
    (!response_id.is_empty() && !turn_state.is_empty()).then_some((response_id, turn_state))
}

pub fn runtime_is_compact_session_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX)
}

pub fn runtime_external_response_profile_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_response_turn_state_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub fn runtime_external_session_id_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_compact_session_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub fn runtime_continuation_status_map_mut(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &mut BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &mut statuses.response,
        RuntimeContinuationBindingKind::TurnState => &mut statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &mut statuses.session_id,
    }
}

pub fn runtime_continuation_binding_lifecycle_rank(
    state: RuntimeContinuationBindingLifecycle,
) -> u8 {
    match state {
        RuntimeContinuationBindingLifecycle::Dead => 0,
        RuntimeContinuationBindingLifecycle::Suspect => 1,
        RuntimeContinuationBindingLifecycle::Warm => 2,
        RuntimeContinuationBindingLifecycle::Verified => 3,
    }
}

pub fn runtime_continuation_status_evidence_sort_key(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u32, u32, u32, u8, i64, i64, i64) {
    (
        runtime_continuation_binding_lifecycle_rank(status.state),
        status.confidence.min(policy.confidence_max),
        status.success_count,
        u32::MAX.saturating_sub(status.not_found_streak),
        if status.last_verified_route.is_some() {
            1
        } else {
            0
        },
        status.last_verified_at.unwrap_or(i64::MIN),
        status.last_touched_at.unwrap_or(i64::MIN),
        status.last_not_found_at.unwrap_or(i64::MIN),
    )
}

pub fn runtime_continuation_status_is_more_evidenced(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    runtime_continuation_status_evidence_sort_key(candidate, policy)
        > runtime_continuation_status_evidence_sort_key(current, policy)
}

pub fn runtime_continuation_status_should_replace(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match (
        runtime_continuation_status_last_event_at(candidate),
        runtime_continuation_status_last_event_at(current),
    ) {
        (Some(candidate_at), Some(current_at)) if candidate_at != current_at => {
            return candidate_at > current_at;
        }
        (Some(_), None) => return true,
        (None, Some(_)) => return false,
        _ => {}
    }

    match (
        runtime_continuation_status_is_terminal(candidate, policy),
        runtime_continuation_status_is_terminal(current, policy),
    ) {
        (true, false) => return true,
        (false, true) => return false,
        _ => {}
    }

    runtime_continuation_status_is_more_evidenced(candidate, current, policy)
}

pub fn runtime_continuation_status_last_event_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    [
        status.last_not_found_at,
        status.last_verified_at,
        status.last_touched_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

pub fn runtime_continuation_status_is_terminal(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Dead
        || status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (status.state == RuntimeContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

pub fn runtime_continuation_status_is_stale_verified(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Verified
        && runtime_continuation_status_last_event_at(status)
            .is_some_and(|last| now.saturating_sub(last) >= policy.verified_stale_seconds)
}

pub fn runtime_age_stale_verified_continuation_status(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    let Some(status) = runtime_continuation_status_map_mut(statuses, kind).get_mut(key) else {
        return false;
    };
    if !runtime_continuation_status_is_stale_verified(status, now, policy) {
        return false;
    }
    status.state = RuntimeContinuationBindingLifecycle::Warm;
    true
}

pub fn runtime_continuation_status_should_retain_with_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => false,
        RuntimeContinuationBindingLifecycle::Verified
        | RuntimeContinuationBindingLifecycle::Warm => {
            status.confidence > 0
                || status.success_count > 0
                || status.last_verified_at.is_some()
                || status.last_touched_at.is_some()
        }
        RuntimeContinuationBindingLifecycle::Suspect => {
            status.not_found_streak < policy.suspect_not_found_streak_limit
                && status.confidence > 0
                && status
                    .last_not_found_at
                    .is_some_and(|last| now.saturating_sub(last) < policy.suspect_grace_seconds)
        }
    }
}

pub fn runtime_continuation_status_should_retain_without_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => status
            .last_not_found_at
            .or(status.last_touched_at)
            .is_some_and(|last| now.saturating_sub(last) < policy.dead_grace_seconds),
        _ => runtime_continuation_status_should_retain_with_binding(status, now, policy),
    }
}

pub fn runtime_continuation_status_dead_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    (status.state == RuntimeContinuationBindingLifecycle::Dead)
        .then(|| status.last_not_found_at.or(status.last_touched_at))
        .flatten()
}

pub fn runtime_continuation_dead_status_shadowed_by_binding(
    binding: &ResponseProfileBinding,
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_dead_at(status).is_some_and(|dead_at| binding.bound_at > dead_at)
}

pub fn merge_runtime_continuation_status_map(
    existing: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    incoming: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    live_bindings: &BTreeMap<String, ResponseProfileBinding>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> BTreeMap<String, RuntimeContinuationBindingStatus> {
    let mut merged = existing.clone();
    for (key, status) in incoming {
        let should_replace = merged.get(key).is_none_or(|current| {
            runtime_continuation_status_should_replace(status, current, policy)
        });
        if should_replace {
            merged.insert(key.clone(), status.clone());
        }
    }
    merged.retain(|key, status| {
        live_bindings.contains_key(key)
            || runtime_continuation_status_should_retain_without_binding(status, now, policy)
    });
    merged
}

pub fn merge_runtime_continuation_statuses(
    existing: &RuntimeContinuationStatuses,
    incoming: &RuntimeContinuationStatuses,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStatuses {
    RuntimeContinuationStatuses {
        response: merge_runtime_continuation_status_map(
            &existing.response,
            &incoming.response,
            response_bindings,
            now,
            policy,
        ),
        turn_state: merge_runtime_continuation_status_map(
            &existing.turn_state,
            &incoming.turn_state,
            turn_state_bindings,
            now,
            policy,
        ),
        session_id: merge_runtime_continuation_status_map(
            &existing.session_id,
            &incoming.session_id,
            session_id_bindings,
            now,
            policy,
        ),
    }
}

pub fn compact_runtime_continuation_statuses(
    statuses: RuntimeContinuationStatuses,
    continuations: &RuntimeContinuationStore<ResponseProfileBinding>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStatuses {
    let mut merged = merge_runtime_continuation_statuses(
        &RuntimeContinuationStatuses::default(),
        &statuses,
        &continuations.response_profile_bindings,
        &continuations.turn_state_bindings,
        &continuations.session_id_bindings,
        now,
        policy,
    );
    merged.response.retain(|key, status| {
        if let Some(binding) = continuations.response_profile_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now, policy)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now, policy)
        }
    });
    merged.turn_state.retain(|key, status| {
        if let Some(binding) = continuations.turn_state_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now, policy)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now, policy)
        }
    });
    merged.session_id.retain(|key, status| {
        if let Some(binding) = continuations.session_id_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now, policy)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now, policy)
        }
    });
    prune_runtime_continuation_status_map(
        &mut merged.response,
        &continuations.response_profile_bindings,
        policy.response_status_limit,
        policy,
    );
    prune_runtime_continuation_status_map(
        &mut merged.turn_state,
        &continuations.turn_state_bindings,
        policy.turn_state_status_limit,
        policy,
    );
    prune_runtime_continuation_status_map(
        &mut merged.session_id,
        &continuations.session_id_bindings,
        policy.session_id_status_limit,
        policy,
    );
    merged
}

pub fn runtime_continuation_status_retention_sort_key(
    key: &str,
    status: &RuntimeContinuationBindingStatus,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let evidence = runtime_continuation_status_evidence_sort_key(status, policy);
    (
        if bindings.contains_key(key) { 1 } else { 0 },
        evidence.0,
        evidence.1,
        evidence.2,
        evidence.3,
        evidence.4,
        evidence.5,
        evidence.6,
        evidence.7,
        bindings
            .get(key)
            .map(|binding| binding.bound_at)
            .unwrap_or(i64::MIN),
    )
}

pub fn prune_runtime_continuation_status_map(
    statuses: &mut BTreeMap<String, RuntimeContinuationBindingStatus>,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
    policy: RuntimeContinuationCompactionPolicy,
) {
    if statuses.len() <= max_entries {
        return;
    }

    let excess = statuses.len() - max_entries;
    let mut coldest = statuses
        .iter()
        .map(|(key, status)| {
            (
                key.clone(),
                runtime_continuation_status_retention_sort_key(key, status, bindings, policy),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (key, _) in coldest.into_iter().take(excess) {
        statuses.remove(&key);
    }
}

pub fn runtime_continuation_binding_should_retain(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match status {
        Some(status) if runtime_continuation_dead_status_shadowed_by_binding(binding, status) => {
            true
        }
        Some(status) if runtime_continuation_status_is_terminal(status, policy) => false,
        Some(status) => runtime_continuation_status_should_retain_with_binding(status, now, policy),
        None => binding.bound_at <= now,
    }
}

pub fn runtime_continuation_binding_retention_sort_key(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let evidence = status
        .map(|status| runtime_continuation_status_evidence_sort_key(status, policy))
        .unwrap_or((0, 0, 0, 0, 0, i64::MIN, i64::MIN, i64::MIN));
    (
        evidence.0,
        evidence.1,
        evidence.2,
        evidence.3,
        evidence.4,
        evidence.5,
        evidence.6,
        evidence.7,
        binding.bound_at,
    )
}

pub fn prune_runtime_continuation_response_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    statuses: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    max_entries: usize,
    policy: RuntimeContinuationCompactionPolicy,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut coldest = bindings
        .iter()
        .map(|(response_id, binding)| {
            (
                response_id.clone(),
                runtime_continuation_binding_retention_sort_key(
                    binding,
                    statuses.get(response_id),
                    policy,
                ),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (response_id, _) in coldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

pub fn compact_runtime_continuation_store(
    mut continuations: RuntimeContinuationStore<ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.response_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.turn_state_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_id_bindings,
        profiles,
    );
    continuations
        .response_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.response.get(key),
                now,
                policy,
            )
        });
    let response_turn_state_keys = continuations
        .response_profile_bindings
        .keys()
        .filter(|key| runtime_is_response_turn_state_lineage_key(key))
        .cloned()
        .collect::<Vec<_>>();
    for key in response_turn_state_keys {
        let Some((response_id, _)) = runtime_response_turn_state_lineage_parts(&key) else {
            continuations.response_profile_bindings.remove(&key);
            continue;
        };
        if continuations
            .response_profile_bindings
            .get(response_id)
            .is_none_or(|binding| !profiles.contains_key(&binding.profile_name))
        {
            continuations.response_profile_bindings.remove(&key);
        }
    }
    continuations.turn_state_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(
            binding,
            continuations.statuses.turn_state.get(key),
            now,
            policy,
        )
    });
    continuations
        .session_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.session_id.get(key),
                now,
                policy,
            )
        });
    continuations.session_id_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(
            binding,
            continuations.statuses.session_id.get(key),
            now,
            policy,
        )
    });
    prune_runtime_continuation_response_bindings(
        &mut continuations.response_profile_bindings,
        &continuations.statuses.response,
        policy.response_binding_limit,
        policy,
    );
    prune_profile_bindings(
        &mut continuations.turn_state_bindings,
        policy.turn_state_binding_limit,
    );
    prune_profile_bindings(
        &mut continuations.session_profile_bindings,
        policy.session_id_binding_limit,
    );
    prune_profile_bindings(
        &mut continuations.session_id_bindings,
        policy.session_id_binding_limit,
    );
    let statuses = std::mem::take(&mut continuations.statuses);
    continuations.statuses =
        compact_runtime_continuation_statuses(statuses, &continuations, now, policy);
    continuations
}

pub fn merge_runtime_continuation_store(
    existing: &RuntimeContinuationStore<ResponseProfileBinding>,
    incoming: &RuntimeContinuationStore<ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    let response_profile_bindings = merge_profile_bindings(
        &existing.response_profile_bindings,
        &incoming.response_profile_bindings,
        profiles,
    );
    let turn_state_bindings = merge_profile_bindings(
        &existing.turn_state_bindings,
        &incoming.turn_state_bindings,
        profiles,
    );
    let session_id_bindings = merge_profile_bindings(
        &existing.session_id_bindings,
        &incoming.session_id_bindings,
        profiles,
    );
    let statuses = merge_runtime_continuation_statuses(
        &existing.statuses,
        &incoming.statuses,
        &response_profile_bindings,
        &turn_state_bindings,
        &session_id_bindings,
        now,
        policy,
    );
    compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            session_profile_bindings: merge_profile_bindings(
                &existing.session_profile_bindings,
                &incoming.session_profile_bindings,
                profiles,
            ),
            turn_state_bindings: turn_state_bindings.clone(),
            session_id_bindings: session_id_bindings.clone(),
            statuses,
        },
        profiles,
        now,
        policy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_runtime_state::{RuntimeProfileUsageSnapshot, RuntimeQuotaWindowStatus};
    use prodex_state::{ProfileProvider, ResponseProfileBinding};
    use std::path::PathBuf;

    fn profile() -> ProfileEntry {
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/profile"),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        }
    }

    #[test]
    fn usage_snapshot_compaction_prunes_missing_and_expired_profiles() {
        let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
        let snapshots = BTreeMap::from([
            (
                "alpha".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: 200,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 100,
                    five_hour_reset_at: 0,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 100,
                    weekly_reset_at: 0,
                },
            ),
            (
                "missing".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: 200,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 100,
                    five_hour_reset_at: 0,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 100,
                    weekly_reset_at: 0,
                },
            ),
        ]);

        let compacted = compact_runtime_usage_snapshots(snapshots, &profiles, 250);

        assert_eq!(compacted.len(), 1);
        assert!(compacted.contains_key("alpha"));
    }

    #[test]
    fn profile_score_compaction_supports_route_scoped_keys() {
        let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
        let scores = BTreeMap::from([
            (
                "__route_health__:responses:alpha".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: 200,
                },
            ),
            (
                "__route_health__:responses:missing".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: 200,
                },
            ),
        ]);

        let compacted = compact_runtime_profile_scores(scores, &profiles, 250);

        assert_eq!(
            compacted.keys().cloned().collect::<Vec<_>>(),
            vec!["__route_health__:responses:alpha".to_string()]
        );
    }

    #[test]
    fn backoff_compaction_keeps_valid_route_keys_and_future_backoffs() {
        let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
        let backoffs = RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([
                ("alpha".to_string(), 300),
                ("expired".to_string(), 100),
            ]),
            transport_backoff_until: BTreeMap::from([
                (
                    "__route_transport_backoff__:responses:alpha".to_string(),
                    300,
                ),
                ("__route_transport_backoff__:unknown:alpha".to_string(), 300),
                ("missing".to_string(), 300),
            ]),
            route_circuit_open_until: BTreeMap::from([
                ("__route_circuit__:responses:alpha".to_string(), 300),
                ("__route_circuit__:responses:missing".to_string(), 300),
            ]),
        };

        let compacted = compact_runtime_profile_backoffs(backoffs, &profiles, 200);

        assert_eq!(
            compacted
                .retry_backoff_until
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["alpha".to_string()]
        );
        assert_eq!(
            compacted
                .transport_backoff_until
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["__route_transport_backoff__:responses:alpha".to_string()]
        );
        assert_eq!(
            compacted
                .route_circuit_open_until
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["__route_circuit__:responses:alpha".to_string()]
        );
    }

    #[test]
    fn transport_backoff_helpers_prefer_route_scoped_future_values() {
        let profile_names = BTreeSet::from(["alpha".to_string()]);
        let route_key = runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Responses);
        let backoffs = BTreeMap::from([
            ("alpha".to_string(), 250),
            (route_key.clone(), 300),
            (
                runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Compact),
                100,
            ),
        ]);

        assert!(runtime_profile_transport_backoff_key_valid(
            &route_key,
            &profile_names
        ));
        assert_eq!(
            runtime_profile_transport_backoff_until_from_map(
                &backoffs,
                "alpha",
                RuntimeRouteKind::Responses,
                200,
            ),
            Some(300)
        );
        assert_eq!(
            runtime_profile_transport_backoff_max_until(&backoffs, "alpha", 200),
            Some(300)
        );
    }

    #[test]
    fn continuation_store_compaction_removes_orphan_lineage_bindings() {
        let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
        let lineage_key = format!(
            "{}{}:{}:{}",
            RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX,
            "missing-parent".len(),
            "missing-parent",
            "turn"
        );
        let keep_lineage_key = format!(
            "{}{}:{}:{}",
            RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX,
            "resp".len(),
            "resp",
            "turn"
        );
        let continuations = RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([
                (
                    "resp".to_string(),
                    ResponseProfileBinding {
                        profile_name: "alpha".to_string(),
                        bound_at: 100,
                    },
                ),
                (
                    keep_lineage_key.clone(),
                    ResponseProfileBinding {
                        profile_name: "alpha".to_string(),
                        bound_at: 100,
                    },
                ),
                (
                    lineage_key.clone(),
                    ResponseProfileBinding {
                        profile_name: "alpha".to_string(),
                        bound_at: 100,
                    },
                ),
                (
                    "missing-profile".to_string(),
                    ResponseProfileBinding {
                        profile_name: "missing".to_string(),
                        bound_at: 100,
                    },
                ),
            ]),
            ..RuntimeContinuationStore::default()
        };

        let compacted = compact_runtime_continuation_store(
            continuations,
            &profiles,
            200,
            RuntimeContinuationCompactionPolicy::default(),
        );

        assert!(compacted.response_profile_bindings.contains_key("resp"));
        assert!(
            compacted
                .response_profile_bindings
                .contains_key(&keep_lineage_key)
        );
        assert!(
            !compacted
                .response_profile_bindings
                .contains_key(&lineage_key)
        );
        assert!(
            !compacted
                .response_profile_bindings
                .contains_key("missing-profile")
        );
    }
}
