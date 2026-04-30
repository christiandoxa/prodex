use super::*;

pub(crate) use prodex_state::{
    merge_last_run_selection, merge_profile_bindings,
    prune_profile_bindings_for_housekeeping_without_retention,
};

fn app_state_compaction_policy() -> prodex_state::AppStateCompactionPolicy {
    prodex_state::AppStateCompactionPolicy {
        last_run_retention_seconds: APP_STATE_LAST_RUN_RETENTION_SECONDS,
        session_binding_retention_seconds: APP_STATE_SESSION_BINDING_RETENTION_SECONDS,
        session_binding_limit: SESSION_ID_PROFILE_BINDING_LIMIT,
    }
}

pub(crate) fn compact_app_state(state: AppState, now: i64) -> AppState {
    prodex_state::compact_app_state_with_policy(state, now, app_state_compaction_policy())
}

#[allow(dead_code)]
pub(crate) fn prune_last_run_selection(
    selections: &mut BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) {
    prodex_state::prune_last_run_selection_with_retention(
        selections,
        profiles,
        now,
        APP_STATE_LAST_RUN_RETENTION_SECONDS,
    );
}

#[allow(dead_code)]
pub(crate) fn prune_profile_bindings_for_housekeeping(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    retention_seconds: i64,
    max_entries: usize,
) {
    prodex_state::prune_profile_bindings_for_housekeeping(
        bindings,
        profiles,
        now,
        retention_seconds,
        max_entries,
    );
}

pub(crate) fn runtime_continuation_binding_lifecycle_rank(
    state: RuntimeContinuationBindingLifecycle,
) -> u8 {
    match state {
        RuntimeContinuationBindingLifecycle::Dead => 0,
        RuntimeContinuationBindingLifecycle::Suspect => 1,
        RuntimeContinuationBindingLifecycle::Warm => 2,
        RuntimeContinuationBindingLifecycle::Verified => 3,
    }
}

pub(crate) fn runtime_continuation_status_evidence_sort_key(
    status: &RuntimeContinuationBindingStatus,
) -> (u8, u32, u32, u32, u8, i64, i64, i64) {
    (
        runtime_continuation_binding_lifecycle_rank(status.state),
        status.confidence.min(RUNTIME_CONTINUATION_CONFIDENCE_MAX),
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

pub(crate) fn runtime_continuation_status_is_more_evidenced(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_evidence_sort_key(candidate)
        > runtime_continuation_status_evidence_sort_key(current)
}

pub(crate) fn runtime_continuation_status_should_replace(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
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
        runtime_continuation_status_is_terminal(candidate),
        runtime_continuation_status_is_terminal(current),
    ) {
        (true, false) => return true,
        (false, true) => return false,
        _ => {}
    }

    runtime_continuation_status_is_more_evidenced(candidate, current)
}

pub(crate) fn runtime_continuation_status_last_event_at(
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

pub(crate) fn runtime_continuation_status_is_terminal(
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Dead
        || status.not_found_streak >= RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
        || (status.state == RuntimeContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

pub(crate) fn runtime_continuation_status_is_stale_verified(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Verified
        && runtime_continuation_status_last_event_at(status).is_some_and(|last| {
            now.saturating_sub(last) >= RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS
        })
}

pub(crate) fn runtime_age_stale_verified_continuation_status(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let Some(status) = runtime_continuation_status_map_mut(statuses, kind).get_mut(key) else {
        return false;
    };
    if !runtime_continuation_status_is_stale_verified(status, now) {
        return false;
    }
    status.state = RuntimeContinuationBindingLifecycle::Warm;
    true
}

pub(crate) fn runtime_continuation_status_should_retain_with_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
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
            status.not_found_streak < RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
                && status.confidence > 0
                && status.last_not_found_at.is_some_and(|last| {
                    now.saturating_sub(last) < RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
                })
        }
    }
}

pub(crate) fn runtime_continuation_status_should_retain_without_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => status
            .last_not_found_at
            .or(status.last_touched_at)
            .is_some_and(|last| now.saturating_sub(last) < RUNTIME_CONTINUATION_DEAD_GRACE_SECONDS),
        _ => runtime_continuation_status_should_retain_with_binding(status, now),
    }
}

pub(crate) fn runtime_continuation_status_dead_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    (status.state == RuntimeContinuationBindingLifecycle::Dead)
        .then(|| status.last_not_found_at.or(status.last_touched_at))
        .flatten()
}

pub(crate) fn runtime_continuation_dead_status_shadowed_by_binding(
    binding: &ResponseProfileBinding,
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_dead_at(status).is_some_and(|dead_at| binding.bound_at > dead_at)
}

pub(crate) fn merge_runtime_continuation_status_map(
    existing: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    incoming: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    live_bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, RuntimeContinuationBindingStatus> {
    let now = Local::now().timestamp();
    let mut merged = existing.clone();
    for (key, status) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| runtime_continuation_status_should_replace(status, current));
        if should_replace {
            merged.insert(key.clone(), status.clone());
        }
    }
    merged.retain(|key, status| {
        live_bindings.contains_key(key)
            || runtime_continuation_status_should_retain_without_binding(status, now)
    });
    merged
}

pub(crate) fn merge_runtime_continuation_statuses(
    existing: &RuntimeContinuationStatuses,
    incoming: &RuntimeContinuationStatuses,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> RuntimeContinuationStatuses {
    RuntimeContinuationStatuses {
        response: merge_runtime_continuation_status_map(
            &existing.response,
            &incoming.response,
            response_bindings,
        ),
        turn_state: merge_runtime_continuation_status_map(
            &existing.turn_state,
            &incoming.turn_state,
            turn_state_bindings,
        ),
        session_id: merge_runtime_continuation_status_map(
            &existing.session_id,
            &incoming.session_id,
            session_id_bindings,
        ),
    }
}

pub(crate) fn compact_runtime_continuation_statuses(
    statuses: RuntimeContinuationStatuses,
    continuations: &RuntimeContinuationStore,
) -> RuntimeContinuationStatuses {
    let now = Local::now().timestamp();
    let mut merged = merge_runtime_continuation_statuses(
        &RuntimeContinuationStatuses::default(),
        &statuses,
        &continuations.response_profile_bindings,
        &continuations.turn_state_bindings,
        &continuations.session_id_bindings,
    );
    merged.response.retain(|key, status| {
        if let Some(binding) = continuations.response_profile_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged.turn_state.retain(|key, status| {
        if let Some(binding) = continuations.turn_state_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged.session_id.retain(|key, status| {
        if let Some(binding) = continuations.session_id_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    prune_runtime_continuation_status_map(
        &mut merged.response,
        &continuations.response_profile_bindings,
        RUNTIME_CONTINUATION_RESPONSE_STATUS_LIMIT,
    );
    prune_runtime_continuation_status_map(
        &mut merged.turn_state,
        &continuations.turn_state_bindings,
        RUNTIME_CONTINUATION_TURN_STATE_STATUS_LIMIT,
    );
    prune_runtime_continuation_status_map(
        &mut merged.session_id,
        &continuations.session_id_bindings,
        RUNTIME_CONTINUATION_SESSION_ID_STATUS_LIMIT,
    );
    merged
}

pub(crate) fn runtime_continuation_status_retention_sort_key(
    key: &str,
    status: &RuntimeContinuationBindingStatus,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> (u8, u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let evidence = runtime_continuation_status_evidence_sort_key(status);
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

pub(crate) fn prune_runtime_continuation_status_map(
    statuses: &mut BTreeMap<String, RuntimeContinuationBindingStatus>,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
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
                runtime_continuation_status_retention_sort_key(key, status, bindings),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (key, _) in coldest.into_iter().take(excess) {
        statuses.remove(&key);
    }
}

pub(crate) fn runtime_continuation_binding_should_retain(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
) -> bool {
    match status {
        Some(status) if runtime_continuation_dead_status_shadowed_by_binding(binding, status) => {
            true
        }
        Some(status) if runtime_continuation_status_is_terminal(status) => false,
        Some(status) => runtime_continuation_status_should_retain_with_binding(status, now),
        None => binding.bound_at <= now,
    }
}

pub(crate) fn runtime_continuation_binding_retention_sort_key(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
) -> (u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let evidence = status
        .map(runtime_continuation_status_evidence_sort_key)
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

pub(crate) fn prune_runtime_continuation_response_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    statuses: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    max_entries: usize,
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
                runtime_continuation_binding_retention_sort_key(binding, statuses.get(response_id)),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (response_id, _) in coldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

pub(crate) fn merge_runtime_state_snapshot(existing: AppState, snapshot: &AppState) -> AppState {
    prodex_state::merge_runtime_state_snapshot_with_policy(
        existing,
        snapshot,
        Local::now().timestamp(),
        app_state_compaction_policy(),
    )
}
