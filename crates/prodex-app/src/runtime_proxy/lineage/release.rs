use super::helpers::{
    drain_runtime_response_turn_state_lineage, runtime_live_response_turn_states_for_profile,
};
use super::*;
pub(crate) fn release_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    reason: &str,
) -> Result<bool> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    if session_id.is_none() && turn_state.is_none() {
        return Ok(false);
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        if runtime
            .session_id_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.session_id_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                now,
            );
            changed = true;
        }
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        if runtime
            .turn_state_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.turn_state_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                now,
            );
            changed = true;
        }
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("compact_lineage_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "compact_lineage_released profile={profile_name} reason={reason} session={} turn_state={}",
                session_id.unwrap_or("-"),
                turn_state.unwrap_or("-"),
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(changed)
}

pub(crate) fn clear_runtime_dead_response_bindings(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response_ids: &[String],
    reason: &str,
) -> Result<bool> {
    let response_ids = response_ids
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if response_ids.is_empty() {
        return Ok(false);
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let mut changed = false;
    let mut dead_turn_states = BTreeSet::new();
    for response_id in &response_ids {
        if runtime
            .state
            .response_profile_bindings
            .get(*response_id)
            .is_some_and(|binding| binding.profile_name != profile_name)
        {
            continue;
        }
        changed = runtime
            .state
            .response_profile_bindings
            .remove(*response_id)
            .is_some()
            || changed;
        let removed_turn_states = drain_runtime_response_turn_state_lineage(
            &mut runtime.state.response_profile_bindings,
            response_id,
            Some(profile_name),
        );
        changed = !removed_turn_states.is_empty() || changed;
        dead_turn_states.extend(removed_turn_states);
        changed = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            response_id,
            now,
        ) || changed;
    }
    let surviving_turn_states = runtime_live_response_turn_states_for_profile(
        &runtime.state.response_profile_bindings,
        profile_name,
        &dead_turn_states,
    );
    for turn_state in dead_turn_states {
        if runtime
            .turn_state_bindings
            .get(turn_state.as_str())
            .is_some_and(|binding| binding.profile_name == profile_name)
            && !surviving_turn_states.contains(turn_state.as_str())
        {
            changed = runtime
                .turn_state_bindings
                .remove(turn_state.as_str())
                .is_some()
                || changed;
            changed = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                turn_state.as_str(),
                now,
            ) || changed;
        }
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("dead_response_binding_clear:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "response_bindings_cleared profile={profile_name} count={} first={:?} reason={reason}",
                response_ids.len(),
                response_ids.first().copied(),
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn clear_runtime_stale_previous_response_binding(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    if runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_none_or(|binding| binding.profile_name != profile_name)
    {
        drop(runtime);
        return Ok(false);
    }

    runtime
        .state
        .response_profile_bindings
        .remove(previous_response_id);
    let _ = clear_runtime_response_turn_state_lineage(
        &mut runtime.state.response_profile_bindings,
        previous_response_id,
    );
    let now = Local::now().timestamp();
    let _ = runtime_mark_continuation_status_dead(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("previous_response_binding_clear:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "previous_response_binding_cleared profile={profile_name} response_id={previous_response_id}"
        ),
    );
    Ok(true)
}

pub(super) fn release_runtime_affinity_bindings(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
    now: i64,
) -> bool {
    let mut changed = false;
    let release_session_affinity = previous_response_id.is_none() && turn_state.is_none();

    if let Some(previous_response_id) = previous_response_id
        && runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime
            .state
            .response_profile_bindings
            .remove(previous_response_id);
        let _ = clear_runtime_response_turn_state_lineage(
            &mut runtime.state.response_profile_bindings,
            previous_response_id,
        );
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
        changed = true;
    }

    // Dropping previous_response or turn_state affinity should not also erase an existing
    // session lineage. Fresh fallback may still need that session owner to preserve compact
    // context or to reapply soft session affinity on the next selection pass.
    if release_session_affinity
        && let Some(session_id) = session_id
        && runtime
            .session_id_bindings
            .get(session_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.session_id_bindings.remove(session_id);
        runtime.state.session_profile_bindings.remove(session_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
        changed = true;
    }

    changed
}

pub(crate) fn release_runtime_quota_blocked_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<bool> {
    release_runtime_profile_affinity(
        shared,
        profile_name,
        previous_response_id,
        turn_state,
        session_id,
        "quota",
    )
}

pub(crate) fn release_runtime_auth_failed_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<bool> {
    release_runtime_profile_affinity(
        shared,
        profile_name,
        previous_response_id,
        turn_state,
        session_id,
        "auth_failed",
    )
}

pub(super) fn release_runtime_profile_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
    reason: &str,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let changed = release_runtime_affinity_bindings(
        &mut runtime,
        profile_name,
        previous_response_id,
        turn_state,
        session_id,
        now,
    );

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("{reason}_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "{reason}_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

pub(crate) fn release_runtime_previous_response_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let previous_response_failures = note_runtime_previous_response_not_found(
        shared,
        profile_name,
        previous_response_id,
        route_kind,
    )?;
    if previous_response_failures < RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD {
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_deferred profile={profile_name} route={} previous_response_id={:?} failures={previous_response_failures}",
                runtime_route_kind_label(route_kind),
                previous_response_id,
            ),
        );
        return Ok(false);
    }
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let changed = release_runtime_affinity_bindings(
        &mut runtime,
        profile_name,
        previous_response_id,
        turn_state,
        session_id,
        now,
    );

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("previous_response_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}
