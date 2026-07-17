use super::*;

fn runtime_route_kind_to_proxy(
    route_kind: RuntimeRouteKind,
) -> runtime_proxy_crate::RuntimeRouteKind {
    match route_kind {
        RuntimeRouteKind::Responses => runtime_proxy_crate::RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact => runtime_proxy_crate::RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket => runtime_proxy_crate::RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard => runtime_proxy_crate::RuntimeRouteKind::Standard,
    }
}

pub(crate) fn update_runtime_profile_route_performance(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    next_score: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_performance_key(profile_name, route_kind);
    if next_score == 0 {
        prodex_runtime_store::clear_runtime_profile_score(&mut runtime.profile_health, &key, now);
    } else {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score.min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
                updated_at: now,
            },
        );
    }
    drop(runtime);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_latency",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field(
                    "score",
                    next_score
                        .min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX)
                        .to_string(),
                ),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
    Ok(())
}

pub(crate) fn note_runtime_profile_latency_observation(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
    elapsed_ms: u64,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let next_score = runtime_proxy_crate::runtime_profile_latency_observation_next_score(
        current_score,
        elapsed_ms,
        runtime_route_kind_to_proxy(route_kind),
        stage,
    );
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        &format!("{stage}_{elapsed_ms}ms"),
    );
}

pub(crate) fn note_runtime_profile_latency_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let next_score = runtime_proxy_crate::runtime_profile_latency_failure_next_score(current_score);
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        stage,
    );
}

pub(crate) fn reset_runtime_profile_success_streak(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) {
    prodex_runtime_store::clear_runtime_profile_score(
        &mut runtime.profile_health,
        &runtime_profile_route_success_streak_key(profile_name, route_kind),
        Local::now().timestamp(),
    );
}

pub(crate) fn bump_runtime_profile_bad_pairing_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_bad_pairing_key(profile_name, route_kind);
    let current_score = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    );
    let next_score =
        runtime_proxy_crate::runtime_profile_bad_pairing_next_score(current_score, delta);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        RuntimeStateMutation::ProfileBadPairing(format!(
            "{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        )),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_bad_pairing",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("score", next_score.to_string()),
                runtime_proxy_log_field("delta", delta.to_string()),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
    Ok(())
}

pub(crate) fn bump_runtime_profile_health_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let current_score = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0);
    let circuit_key = runtime_profile_route_circuit_key(profile_name, route_kind);
    let current_reopen_stage = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS,
    );
    let bump_decision = runtime_proxy_crate::runtime_profile_health_bump_decision(
        current_score,
        delta,
        runtime
            .profile_route_circuit_open_until
            .contains_key(&circuit_key),
        current_reopen_stage,
    );
    let next_score = bump_decision.next_score;
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    let circuit_until = if let Some(circuit_open_seconds) = bump_decision.circuit_open_seconds {
        let reopen_stage = bump_decision.circuit_reopen_stage.unwrap_or(0);
        if bump_decision.circuit_reopen_stage == Some(0) {
            prodex_runtime_store::clear_runtime_profile_score(
                &mut runtime.profile_health,
                &runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                now,
            );
        } else {
            runtime.profile_health.insert(
                runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                RuntimeProfileHealth {
                    score: reopen_stage,
                    updated_at: now,
                },
            );
        }
        let until = now.saturating_add(circuit_open_seconds);
        runtime
            .profile_route_circuit_open_until
            .entry(circuit_key.clone())
            .and_modify(|current| *current = (*current).max(until))
            .or_insert(until);
        mark_runtime_profile_route_circuit_update(&mut runtime, &circuit_key);
        Some((until, reopen_stage))
    } else {
        None
    };
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        RuntimeStateMutation::ProfileHealth(format!(
            "{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        )),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_health",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("score", next_score.to_string()),
                runtime_proxy_log_field("delta", delta.to_string()),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
    if let Some((until, reopen_stage)) = circuit_until {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "profile_circuit_open",
                [
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("until", until.to_string()),
                    runtime_proxy_log_field("reopen_stage", reopen_stage.to_string()),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("score", next_score.to_string()),
                ],
            ),
        );
    }
    Ok(())
}

pub(crate) fn recover_runtime_profile_health_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) {
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let streak_key = runtime_profile_route_success_streak_key(profile_name, route_kind);
    let current_score = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now));
    let current_streak = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &streak_key,
        now,
        RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS,
    );
    let decision = runtime_proxy_crate::runtime_profile_health_recovery_decision(
        current_score,
        current_streak,
    );

    if let (Some(next_score), Some(next_success_streak)) =
        (decision.next_score, decision.next_success_streak)
    {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score,
                updated_at: now,
            },
        );
        runtime.profile_health.insert(
            streak_key,
            RuntimeProfileHealth {
                score: next_success_streak,
                updated_at: now,
            },
        );
    } else {
        prodex_runtime_store::clear_runtime_profile_score(&mut runtime.profile_health, &key, now);
        prodex_runtime_store::clear_runtime_profile_score(
            &mut runtime.profile_health,
            &streak_key,
            now,
        );
    }
}

pub(crate) fn note_runtime_profile_transport_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
    err: &anyhow::Error,
) {
    let Some(failure_kind) = runtime_proxy_transport_failure_kind(err) else {
        return;
    };
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_transport_failure",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field(
                    "class",
                    runtime_transport_failure_kind_label(failure_kind),
                ),
                runtime_proxy_log_field("context", context),
            ],
        ),
    );
    let _ = bump_runtime_profile_health_score(
        shared,
        profile_name,
        route_kind,
        runtime_profile_transport_health_penalty(failure_kind),
        context,
    );
    let _ = bump_runtime_profile_bad_pairing_score(
        shared,
        profile_name,
        route_kind,
        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
        context,
    );
    note_runtime_profile_latency_failure(shared, profile_name, route_kind, context);
    let _ = mark_runtime_profile_transport_backoff(shared, profile_name, route_kind, context);
}
