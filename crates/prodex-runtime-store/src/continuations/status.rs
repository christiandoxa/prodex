use super::*;

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
