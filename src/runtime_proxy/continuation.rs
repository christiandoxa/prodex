use super::*;

pub(crate) fn runtime_binding_touch_should_persist(bound_at: i64, now: i64) -> bool {
    // These timestamps are stored with second precision. Require strictly more
    // than the interval so a boundary-crossing lookup does not persist nearly a
    // second early.
    now.saturating_sub(bound_at) > RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

pub(crate) fn runtime_continuation_status_map(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &statuses.response,
        RuntimeContinuationBindingKind::TurnState => &statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &statuses.session_id,
    }
}

pub(crate) fn runtime_continuation_status_map_mut(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &mut BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &mut statuses.response,
        RuntimeContinuationBindingKind::TurnState => &mut statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &mut statuses.session_id,
    }
}

pub(crate) fn runtime_continuation_next_event_at(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> i64 {
    runtime_continuation_status_last_event_at(status)
        .filter(|last| *last >= now)
        .map_or(now, |last| last.saturating_add(1))
}

pub(crate) fn runtime_continuation_status_touches(
    status: &mut RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.last_touched_at = Some(event_at);
    if status.state == RuntimeContinuationBindingLifecycle::Suspect {
        if status.last_not_found_at.is_some_and(|last| {
            event_at.saturating_sub(last) >= RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
        }) {
            status.state = RuntimeContinuationBindingLifecycle::Warm;
            status.not_found_streak = 0;
            status.last_not_found_at = None;
        }
        status.confidence = status
            .confidence
            .saturating_add(RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS)
            .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    } else if status.state != RuntimeContinuationBindingLifecycle::Dead {
        status.confidence = status
            .confidence
            .saturating_add(RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS)
            .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    }
    *status != previous
}

pub(crate) fn runtime_mark_continuation_status_touched(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    runtime_continuation_status_touches(status, now)
}

pub(crate) fn runtime_continuation_status_should_refresh_verified(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route: Option<RuntimeRouteKind>,
) -> bool {
    let Some(status) = runtime_continuation_status_map(statuses, kind).get(key) else {
        return true;
    };

    if status.state != RuntimeContinuationBindingLifecycle::Verified {
        return true;
    }

    let verified_route_label = verified_route.map(runtime_route_kind_label);
    if status.last_verified_route.as_deref() != verified_route_label {
        return true;
    }

    status
        .last_verified_at
        .is_none_or(|last_verified_at| runtime_binding_touch_should_persist(last_verified_at, now))
}

pub(crate) fn runtime_continuation_status_should_persist_touch(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let Some(status) = runtime_continuation_status_map(statuses, kind).get(key) else {
        return true;
    };

    if status.state == RuntimeContinuationBindingLifecycle::Suspect
        && status.last_not_found_at.is_some_and(|last_not_found_at| {
            now.saturating_sub(last_not_found_at) >= RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
        })
    {
        return true;
    }

    status
        .last_touched_at
        .is_none_or(|last_touched_at| runtime_binding_touch_should_persist(last_touched_at, now))
}

pub(crate) fn runtime_mark_continuation_status_verified(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route: Option<RuntimeRouteKind>,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.state = RuntimeContinuationBindingLifecycle::Verified;
    status.last_touched_at = Some(event_at);
    status.last_verified_at = Some(event_at);
    status.last_verified_route =
        verified_route.map(|route_kind| runtime_route_kind_label(route_kind).to_string());
    status.last_not_found_at = None;
    status.not_found_streak = 0;
    status.success_count = status.success_count.saturating_add(1);
    status.failure_count = 0;
    status.confidence = status
        .confidence
        .saturating_add(RUNTIME_CONTINUATION_VERIFIED_CONFIDENCE_BONUS)
        .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    *status != previous
}

pub(crate) fn runtime_mark_continuation_status_suspect(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
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
        .saturating_sub(RUNTIME_CONTINUATION_SUSPECT_CONFIDENCE_PENALTY);
    if previous_confidence == 0 {
        status.confidence = 1;
    }
    status.state = if status.not_found_streak >= RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
        || (previous_confidence > 0 && status.confidence == 0)
    {
        RuntimeContinuationBindingLifecycle::Dead
    } else {
        RuntimeContinuationBindingLifecycle::Suspect
    };
    *status != previous
}

pub(crate) fn runtime_mark_continuation_status_dead(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
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
        .max(RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT);
    status.failure_count = status.failure_count.saturating_add(1);
    *status != previous
}

pub(crate) fn runtime_continuation_status_recently_suspect(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    runtime_continuation_status_map(statuses, kind)
        .get(key)
        .is_some_and(|status| {
            status.state == RuntimeContinuationBindingLifecycle::Suspect
                && !runtime_continuation_status_is_terminal(status)
                && status.last_not_found_at.is_some_and(|last| {
                    now.saturating_sub(last) < RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
                })
        })
}

pub(crate) fn runtime_continuation_status_label(
    status: &RuntimeContinuationBindingStatus,
) -> &'static str {
    match status.state {
        RuntimeContinuationBindingLifecycle::Warm => "warm",
        RuntimeContinuationBindingLifecycle::Verified => "verified",
        RuntimeContinuationBindingLifecycle::Suspect => "suspect",
        RuntimeContinuationBindingLifecycle::Dead => "dead",
    }
}

pub(crate) fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    format!("{RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX}{session_id}")
}

pub(crate) fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    format!("{RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX}{turn_state}")
}

pub(crate) fn runtime_response_turn_state_lineage_key(
    response_id: &str,
    turn_state: &str,
) -> String {
    format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{response_id}:{turn_state}",
        response_id.len()
    )
}

pub(crate) fn runtime_is_response_turn_state_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)
}

pub(crate) fn runtime_response_turn_state_lineage_parts(key: &str) -> Option<(&str, &str)> {
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

pub(crate) fn runtime_is_compact_session_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX)
}

pub(crate) fn runtime_external_response_profile_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_response_turn_state_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub(crate) fn runtime_external_session_id_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_compact_session_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub(crate) fn runtime_dead_continuation_status_shadowed_by_live_binding(
    status: Option<&RuntimeContinuationBindingStatus>,
    binding: Option<&ResponseProfileBinding>,
) -> bool {
    matches!(
        (binding, status),
        (Some(binding), Some(status))
            if runtime_continuation_status_is_terminal(status)
                && runtime_continuation_dead_status_shadowed_by_binding(binding, status)
    )
}

pub(crate) fn runtime_touch_compact_lineage_binding(
    shared: &RuntimeRotationProxyShared,
    runtime: &mut RuntimeRotationState,
    key: &str,
    reason: &str,
    session_binding: bool,
) -> Option<String> {
    let now = Local::now().timestamp();
    let status_kind = if session_binding {
        RuntimeContinuationBindingKind::SessionId
    } else {
        RuntimeContinuationBindingKind::TurnState
    };
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        status_kind,
        key,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_stale key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        schedule_runtime_binding_touch_save(shared, runtime, &format!("continuation_stale:{key}"));
        return None;
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        status_kind,
        key,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_recent_suspect key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        return None;
    }
    let (profile_name, dead_shadowed_by_binding) = {
        let bindings = if session_binding {
            &runtime.session_id_bindings
        } else {
            &runtime.turn_state_bindings
        };
        let binding = bindings
            .get(key)
            .filter(|binding| runtime.state.profiles.contains_key(&binding.profile_name));
        (
            binding.map(|binding| binding.profile_name.clone()),
            runtime_dead_continuation_status_shadowed_by_live_binding(
                runtime_continuation_status_map(&runtime.continuation_statuses, status_kind)
                    .get(key),
                binding,
            ),
        )
    };
    if runtime_continuation_status_map(&runtime.continuation_statuses, status_kind)
        .get(key)
        .is_some_and(runtime_continuation_status_is_terminal)
        && !dead_shadowed_by_binding
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_dead key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        return None;
    }
    let bindings = if session_binding {
        &mut runtime.session_id_bindings
    } else {
        &mut runtime.turn_state_bindings
    };
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = bindings.get_mut(key)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            persist_touch = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            status_kind,
            key,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            status_kind,
            key,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(shared, runtime, reason);
    }
    profile_name
}

fn runtime_compact_followup_bound_profile_raw(
    shared: &RuntimeRotationProxyShared,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<Option<(String, &'static str)>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    if let Some(turn_state) = turn_state.map(str::trim).filter(|value| !value.is_empty()) {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        if let Some(profile_name) = runtime_touch_compact_lineage_binding(
            shared,
            &mut runtime,
            &key,
            &format!("compact_turn_state_touch:{turn_state}"),
            false,
        ) {
            return Ok(Some((profile_name, "turn_state")));
        }
    }
    if let Some(session_id) = session_id.map(str::trim).filter(|value| !value.is_empty()) {
        let key = runtime_compact_session_lineage_key(session_id);
        if let Some(profile_name) = runtime_touch_compact_lineage_binding(
            shared,
            &mut runtime,
            &key,
            &format!("compact_session_touch:{session_id}"),
            true,
        ) {
            return Ok(Some((profile_name, "session_id")));
        }
    }
    Ok(None)
}

pub(crate) fn runtime_compact_route_followup_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<Option<(String, &'static str)>> {
    runtime_compact_followup_bound_profile_raw(shared, turn_state, session_id)
}

pub(crate) fn runtime_compact_turn_state_followup_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: Option<&str>,
) -> Result<Option<String>> {
    Ok(
        runtime_compact_followup_bound_profile_raw(shared, turn_state, None)?
            .map(|(profile_name, _)| profile_name),
    )
}

pub(crate) fn runtime_compact_session_followup_bound_profile(
    shared: &RuntimeRotationProxyShared,
    session_id: Option<&RuntimeExplicitSessionId>,
) -> Result<Option<String>> {
    Ok(runtime_compact_followup_bound_profile_raw(
        shared,
        None,
        session_id.map(|value| value.as_str()),
    )?
    .map(|(profile_name, _)| profile_name))
}

pub(crate) fn runtime_previous_response_negative_cache_key(
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

pub(crate) fn runtime_previous_response_negative_cache_failures(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> u32 {
    runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_previous_response_negative_cache_key(
            previous_response_id,
            profile_name,
            route_kind,
        ),
        now,
        RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    )
}

pub(crate) fn runtime_previous_response_negative_cache_active(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_previous_response_negative_cache_failures(
        profile_health,
        previous_response_id,
        profile_name,
        route_kind,
        now,
    ) > 0
}

pub(crate) fn clear_runtime_previous_response_negative_cache(
    runtime: &mut RuntimeRotationState,
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
        changed = runtime
            .profile_health
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

pub(crate) fn note_runtime_previous_response_not_found(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<u32> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(0);
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_previous_response_negative_cache_key(
        previous_response_id,
        profile_name,
        route_kind,
    );
    let next_failures = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    )
    .saturating_add(1)
    .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_failures,
            updated_at: now,
        },
    );
    let _ = runtime_mark_continuation_status_suspect(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    );
    runtime_proxy_log(
        shared,
        format!(
            "previous_response_negative_cache profile={profile_name} route={} response_id={} failures={next_failures}",
            runtime_route_kind_label(route_kind),
            previous_response_id,
        ),
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "previous_response_negative_cache:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    if next_failures >= RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD {
        let _ = bump_runtime_profile_bad_pairing_score(
            shared,
            profile_name,
            route_kind,
            1,
            "previous_response_not_found",
        );
    }
    Ok(next_failures)
}
