use super::*;

pub(crate) const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;
pub(crate) const RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS: i64 = 20;
pub(crate) const RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS: i64 = if cfg!(test) { 320 } else { 600 };
pub(crate) const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;
pub(crate) const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS: i64 =
    if cfg!(test) { 20 } else { 60 };
pub(crate) const RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS: i64 =
    if cfg!(test) { 12 } else { 1_800 };
pub(crate) const RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE: u32 = 4;

pub(crate) fn runtime_profile_circuit_open_seconds(score: u32, reopen_stage: u32) -> i64 {
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

pub(crate) fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
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

pub(crate) fn runtime_profile_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(crate) fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub(crate) fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    key.replacen("__route_circuit__", "__route_health__", 1)
}

pub(crate) fn runtime_profile_route_circuit_reopen_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit_reopen__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(crate) fn runtime_profile_route_circuit_open_until(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    runtime
        .profile_route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now)
}

pub(crate) fn runtime_profile_route_circuit_probe_seconds(
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

pub(crate) fn runtime_soften_persisted_route_circuits_for_startup(
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

pub(crate) fn reserve_runtime_profile_route_circuit_half_open_probe(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_circuit_key(profile_name, route_kind);
    let route_label = runtime_route_kind_label(route_kind);
    let Some(until) = runtime.profile_route_circuit_open_until.get(&key).copied() else {
        return Ok(true);
    };
    if until > now {
        return Ok(false);
    }
    let health_key = runtime_profile_route_circuit_health_key(&key);
    let reopen_key = runtime_profile_route_circuit_reopen_key(profile_name, route_kind);
    let health_score =
        runtime_profile_effective_health_score_from_map(&runtime.profile_health, &health_key, now);
    if health_score == 0 {
        runtime.profile_route_circuit_open_until.remove(&key);
        runtime.profile_health.remove(&reopen_key);
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("profile_circuit_clear:{profile_name}:{route_label}"),
        );
        return Ok(true);
    }

    let probe_seconds = runtime_profile_circuit_half_open_probe_seconds(health_score);
    let reserve_until = now.saturating_add(probe_seconds);
    runtime
        .profile_route_circuit_open_until
        .insert(key, reserve_until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_circuit_half_open_probe:{profile_name}:{route_label}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_circuit_half_open_probe profile={profile_name} route={} until={reserve_until} health={health_score} probe_seconds={probe_seconds}",
            route_label
        ),
    );
    Ok(true)
}

pub(crate) fn clear_runtime_profile_circuit_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime
        .profile_route_circuit_open_until
        .remove(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .is_some()
}
