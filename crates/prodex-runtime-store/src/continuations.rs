use super::*;

pub const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
pub const RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX: &str = "__compact_turn_state__:";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationStatusPolicy {
    pub touch_persist_interval_seconds: i64,
    pub suspect_grace_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub confidence_max: u32,
    pub verified_confidence_bonus: u32,
    pub touch_confidence_bonus: u32,
    pub suspect_confidence_penalty: u32,
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

pub fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    format!("{RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX}{session_id}")
}

pub fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    format!("{RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX}{turn_state}")
}

pub fn runtime_response_turn_state_lineage_key(response_id: &str, turn_state: &str) -> String {
    format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{response_id}:{turn_state}",
        response_id.len()
    )
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

pub fn runtime_continuation_status_map(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &statuses.response,
        RuntimeContinuationBindingKind::TurnState => &statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &statuses.session_id,
    }
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

pub fn runtime_binding_touch_should_persist(
    bound_at: i64,
    now: i64,
    touch_persist_interval_seconds: i64,
) -> bool {
    // Timestamps use second precision. Require strictly more than the interval
    // so a boundary-crossing lookup does not persist nearly a second early.
    now.saturating_sub(bound_at) > touch_persist_interval_seconds
}

pub fn runtime_continuation_status_is_terminal_for_status_policy(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Dead
        || status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (status.state == RuntimeContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

pub fn runtime_continuation_next_event_at(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> i64 {
    runtime_continuation_status_last_event_at(status)
        .filter(|last| *last >= now)
        .map_or(now, |last| last.saturating_add(1))
}

pub fn runtime_continuation_status_touches(
    status: &mut RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.last_touched_at = Some(event_at);
    if status.state == RuntimeContinuationBindingLifecycle::Suspect {
        if status
            .last_not_found_at
            .is_some_and(|last| event_at.saturating_sub(last) >= policy.suspect_grace_seconds)
        {
            status.state = RuntimeContinuationBindingLifecycle::Warm;
            status.not_found_streak = 0;
            status.last_not_found_at = None;
        }
        status.confidence = status
            .confidence
            .saturating_add(policy.touch_confidence_bonus)
            .min(policy.confidence_max);
    } else if status.state != RuntimeContinuationBindingLifecycle::Dead {
        status.confidence = status
            .confidence
            .saturating_add(policy.touch_confidence_bonus)
            .min(policy.confidence_max);
    }
    *status != previous
}

pub fn runtime_continuation_status_should_refresh_verified(
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    verified_route_label: Option<&str>,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let Some(status) = status else {
        return true;
    };

    if status.state != RuntimeContinuationBindingLifecycle::Verified {
        return true;
    }

    if status.last_verified_route.as_deref() != verified_route_label {
        return true;
    }

    status.last_verified_at.is_none_or(|last_verified_at| {
        runtime_binding_touch_should_persist(
            last_verified_at,
            now,
            policy.touch_persist_interval_seconds,
        )
    })
}

pub fn runtime_continuation_status_should_persist_touch(
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let Some(status) = status else {
        return true;
    };

    if status.state == RuntimeContinuationBindingLifecycle::Suspect
        && status.last_not_found_at.is_some_and(|last_not_found_at| {
            now.saturating_sub(last_not_found_at) >= policy.suspect_grace_seconds
        })
    {
        return true;
    }

    status.last_touched_at.is_none_or(|last_touched_at| {
        runtime_binding_touch_should_persist(
            last_touched_at,
            now,
            policy.touch_persist_interval_seconds,
        )
    })
}

pub fn runtime_mark_continuation_status_touched(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    runtime_continuation_status_touches(status, now, policy)
}

pub fn runtime_mark_continuation_status_verified(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route_label: Option<&str>,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.state = RuntimeContinuationBindingLifecycle::Verified;
    status.last_touched_at = Some(event_at);
    status.last_verified_at = Some(event_at);
    status.last_verified_route = verified_route_label.map(str::to_string);
    status.last_not_found_at = None;
    status.not_found_streak = 0;
    status.success_count = status.success_count.saturating_add(1);
    status.failure_count = 0;
    status.confidence = status
        .confidence
        .saturating_add(policy.verified_confidence_bonus)
        .min(policy.confidence_max);
    *status != previous
}

pub fn runtime_mark_continuation_status_suspect(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.not_found_streak = status.not_found_streak.saturating_add(1);
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.failure_count = status.failure_count.saturating_add(1);
    let previous_confidence = status.confidence;
    status.confidence = status
        .confidence
        .saturating_sub(policy.suspect_confidence_penalty);
    if previous_confidence == 0 {
        status.confidence = 1;
    }
    status.state = if status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (previous_confidence > 0 && status.confidence == 0)
    {
        RuntimeContinuationBindingLifecycle::Dead
    } else {
        RuntimeContinuationBindingLifecycle::Suspect
    };
    *status != previous
}

pub fn runtime_mark_continuation_status_dead(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.state = RuntimeContinuationBindingLifecycle::Dead;
    status.confidence = 0;
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.not_found_streak = status
        .not_found_streak
        .max(policy.suspect_not_found_streak_limit);
    status.failure_count = status.failure_count.saturating_add(1);
    *status != previous
}

pub fn runtime_continuation_status_recently_suspect(
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    status.is_some_and(|status| {
        status.state == RuntimeContinuationBindingLifecycle::Suspect
            && !runtime_continuation_status_is_terminal_for_status_policy(status, policy)
            && status
                .last_not_found_at
                .is_some_and(|last| now.saturating_sub(last) < policy.suspect_grace_seconds)
    })
}

pub fn runtime_continuation_status_label(
    status: &RuntimeContinuationBindingStatus,
) -> &'static str {
    match status.state {
        RuntimeContinuationBindingLifecycle::Warm => "warm",
        RuntimeContinuationBindingLifecycle::Verified => "verified",
        RuntimeContinuationBindingLifecycle::Suspect => "suspect",
        RuntimeContinuationBindingLifecycle::Dead => "dead",
    }
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
