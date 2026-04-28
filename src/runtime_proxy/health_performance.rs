use super::*;

pub(crate) fn runtime_profile_latency_penalty(
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let (good_ms, warn_ms, poor_ms, severe_ms) = match (route_kind, stage) {
        (RuntimeRouteKind::Responses, "ttfb") | (RuntimeRouteKind::Websocket, "connect") => {
            (120, 300, 700, 1_500)
        }
        (RuntimeRouteKind::Compact, _) | (RuntimeRouteKind::Standard, _) => (80, 180, 400, 900),
        _ => (100, 250, 600, 1_200),
    };
    match elapsed_ms {
        elapsed if elapsed <= good_ms => 0,
        elapsed if elapsed <= warn_ms => 2,
        elapsed if elapsed <= poor_ms => 4,
        elapsed if elapsed <= severe_ms => 7,
        _ => RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
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
        runtime.profile_health.remove(&key);
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
    let observed = runtime_profile_latency_penalty(elapsed_ms, route_kind, stage);
    let next_score = if observed == 0 {
        current_score.saturating_sub(2)
    } else {
        (((current_score as u64) * 2) + (observed as u64)).div_ceil(3) as u32
    };
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
    let next_score = current_score
        .saturating_add(RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY)
        .min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX);
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
    runtime
        .profile_health
        .remove(&runtime_profile_route_success_streak_key(
            profile_name,
            route_kind,
        ));
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
    let next_score = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    )
    .saturating_add(delta)
    .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
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
        &format!(
            "profile_bad_pairing:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
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
    let next_score = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
        .saturating_add(delta)
        .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    let circuit_until = if next_score >= RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD {
        let circuit_key = runtime_profile_route_circuit_key(profile_name, route_kind);
        let reopen_stage = if runtime
            .profile_route_circuit_open_until
            .contains_key(&circuit_key)
        {
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS,
            )
            .saturating_add(1)
            .min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)
        } else {
            0
        };
        if reopen_stage == 0 {
            runtime
                .profile_health
                .remove(&runtime_profile_route_circuit_reopen_key(
                    profile_name,
                    route_kind,
                ));
        } else {
            runtime.profile_health.insert(
                runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                RuntimeProfileHealth {
                    score: reopen_stage,
                    updated_at: now,
                },
            );
        }
        let until = now.saturating_add(runtime_profile_circuit_open_seconds(
            next_score,
            reopen_stage,
        ));
        runtime
            .profile_route_circuit_open_until
            .entry(circuit_key)
            .and_modify(|current| *current = (*current).max(until))
            .or_insert(until);
        Some((until, reopen_stage))
    } else {
        None
    };
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "profile_health:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
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
    let Some(current_score) = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
    else {
        runtime.profile_health.remove(&streak_key);
        return;
    };

    let next_streak = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &streak_key,
        now,
        RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS,
    )
    .saturating_add(1)
    .min(RUNTIME_PROFILE_SUCCESS_STREAK_MAX);
    let recovery = RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE
        .saturating_add(next_streak.saturating_sub(1).min(1));
    let next_score = current_score.saturating_sub(recovery);
    if next_score == 0 {
        runtime.profile_health.remove(&key);
        runtime.profile_health.remove(&streak_key);
    } else {
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
                score: next_streak,
                updated_at: now,
            },
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
