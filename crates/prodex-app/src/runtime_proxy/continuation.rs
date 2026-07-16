use super::*;

pub(crate) use prodex_runtime_store::RuntimeContinuationBindingKind;

pub(crate) fn runtime_binding_touch_should_persist(bound_at: i64, now: i64) -> bool {
    prodex_runtime_store::runtime_binding_touch_should_persist(
        bound_at,
        now,
        RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS,
    )
}

fn runtime_continuation_policy() -> prodex_runtime_store::RuntimeContinuationStatusPolicy {
    prodex_runtime_store::RuntimeContinuationStatusPolicy {
        touch_persist_interval_seconds: RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS,
        suspect_grace_seconds: RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS,
        suspect_not_found_streak_limit: RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT,
        confidence_max: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
        verified_confidence_bonus: RUNTIME_CONTINUATION_VERIFIED_CONFIDENCE_BONUS,
        touch_confidence_bonus: RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS,
        suspect_confidence_penalty: RUNTIME_CONTINUATION_SUSPECT_CONFIDENCE_PENALTY,
    }
}

pub(crate) fn runtime_continuation_status_map(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &BTreeMap<String, RuntimeContinuationBindingStatus> {
    prodex_runtime_store::runtime_continuation_status_map(statuses, kind)
}

pub(crate) fn runtime_mark_continuation_status_touched(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    prodex_runtime_store::runtime_mark_continuation_status_touched(
        statuses,
        kind,
        key,
        now,
        runtime_continuation_policy(),
    )
}

pub(crate) fn runtime_continuation_status_should_refresh_verified(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route: Option<RuntimeRouteKind>,
) -> bool {
    prodex_runtime_store::runtime_continuation_status_should_refresh_verified(
        runtime_continuation_status_map(statuses, kind).get(key),
        now,
        verified_route.map(runtime_route_kind_label),
        runtime_continuation_policy(),
    )
}

pub(crate) fn runtime_continuation_status_should_persist_touch(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    prodex_runtime_store::runtime_continuation_status_should_persist_touch(
        runtime_continuation_status_map(statuses, kind).get(key),
        now,
        runtime_continuation_policy(),
    )
}

pub(crate) fn runtime_mark_continuation_status_verified(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route: Option<RuntimeRouteKind>,
) -> bool {
    prodex_runtime_store::runtime_mark_continuation_status_verified(
        statuses,
        kind,
        key,
        now,
        verified_route.map(runtime_route_kind_label),
        runtime_continuation_policy(),
    )
}

pub(crate) fn runtime_mark_continuation_status_suspect(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    prodex_runtime_store::runtime_mark_continuation_status_suspect(
        statuses,
        kind,
        key,
        now,
        runtime_continuation_policy(),
    )
}

pub(crate) fn runtime_mark_continuation_status_dead(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    prodex_runtime_store::runtime_mark_continuation_status_dead(
        statuses,
        kind,
        key,
        now,
        runtime_continuation_policy(),
    )
}

pub(crate) fn runtime_continuation_status_recently_suspect(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    prodex_runtime_store::runtime_continuation_status_recently_suspect(
        runtime_continuation_status_map(statuses, kind).get(key),
        now,
        runtime_continuation_policy(),
    )
}

pub(crate) fn runtime_continuation_status_label(
    status: &RuntimeContinuationBindingStatus,
) -> &'static str {
    prodex_runtime_store::runtime_continuation_status_label(status)
}

pub(crate) fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    prodex_runtime_store::runtime_compact_session_lineage_key(session_id)
}

pub(crate) fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    prodex_runtime_store::runtime_compact_turn_state_lineage_key(turn_state)
}

pub(crate) fn runtime_response_turn_state_lineage_key(
    response_id: &str,
    turn_state: &str,
) -> String {
    prodex_runtime_store::runtime_response_turn_state_lineage_key(response_id, turn_state)
}

pub(crate) fn runtime_external_response_profile_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    prodex_runtime_store::runtime_external_response_profile_bindings(bindings)
}

pub(crate) fn runtime_external_session_id_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    prodex_runtime_store::runtime_external_session_id_bindings(bindings)
}

pub(crate) fn runtime_dead_continuation_status_shadowed_by_live_binding(
    status: Option<&RuntimeContinuationBindingStatus>,
    binding: Option<&ResponseProfileBinding>,
) -> bool {
    matches!(
        (binding, status),
        (Some(binding), Some(status))
            if runtime_continuation_status_is_terminal(status)
                && prodex_runtime_store::runtime_continuation_dead_status_shadowed_by_binding(
                    binding,
                    status,
                )
    )
}

pub(crate) fn runtime_touch_compact_lineage_binding(
    shared: &RuntimeRotationProxyShared,
    runtime: &mut RuntimeRotationState,
    key: &str,
    mutation: RuntimeStateMutation,
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
        schedule_runtime_binding_touch_save(
            shared,
            runtime,
            RuntimeStateMutation::ContinuationStale(key.to_string()),
        );
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
        schedule_runtime_binding_touch_save(shared, runtime, mutation);
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
            RuntimeStateMutation::CompactTurnStateTouch(turn_state.to_string()),
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
            RuntimeStateMutation::CompactSessionTouch(session_id.to_string()),
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
    prodex_runtime_store::runtime_previous_response_negative_cache_key(
        previous_response_id,
        profile_name,
        route_kind,
    )
}

pub(crate) fn runtime_previous_response_negative_cache_active(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    prodex_runtime_store::runtime_previous_response_negative_cache_active(
        profile_health,
        previous_response_id,
        profile_name,
        route_kind,
        now,
        RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    )
}

pub(crate) fn clear_runtime_previous_response_negative_cache(
    runtime: &mut RuntimeRotationState,
    previous_response_id: &str,
    profile_name: &str,
) -> bool {
    prodex_runtime_store::clear_runtime_previous_response_negative_cache(
        &mut runtime.profile_health,
        previous_response_id,
        profile_name,
    )
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
        RuntimeStateMutation::PreviousResponseNegativeCache(format!(
            "{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        )),
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
