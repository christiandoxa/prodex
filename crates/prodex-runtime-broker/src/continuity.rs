use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeBrokerContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

pub fn runtime_broker_continuation_metrics(
    statuses: &RuntimeContinuationStatuses,
    now: i64,
    stale_verified_seconds: i64,
) -> RuntimeBrokerContinuationMetrics {
    let mut metrics = RuntimeBrokerContinuationMetrics {
        response_bindings: statuses.response.len(),
        turn_state_bindings: statuses.turn_state.len(),
        session_id_bindings: statuses.session_id.len(),
        warm: 0,
        verified: 0,
        suspect: 0,
        dead: 0,
        failure_counts: RuntimeBrokerContinuationSignalMetrics::default(),
        not_found_streaks: RuntimeBrokerContinuationSignalMetrics::default(),
        stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics::default(),
    };
    for (kind, status) in statuses
        .response
        .values()
        .map(|status| (RuntimeBrokerContinuationBindingKind::Response, status))
        .chain(
            statuses
                .turn_state
                .values()
                .map(|status| (RuntimeBrokerContinuationBindingKind::TurnState, status)),
        )
        .chain(
            statuses
                .session_id
                .values()
                .map(|status| (RuntimeBrokerContinuationBindingKind::SessionId, status)),
        )
    {
        match status.state {
            RuntimeContinuationBindingLifecycle::Warm => metrics.warm += 1,
            RuntimeContinuationBindingLifecycle::Verified => metrics.verified += 1,
            RuntimeContinuationBindingLifecycle::Suspect => metrics.suspect += 1,
            RuntimeContinuationBindingLifecycle::Dead => metrics.dead += 1,
        }
        runtime_broker_add_continuation_signal(
            &mut metrics.failure_counts,
            kind,
            status.failure_count as usize,
        );
        runtime_broker_add_continuation_signal(
            &mut metrics.not_found_streaks,
            kind,
            status.not_found_streak as usize,
        );
        if runtime_broker_continuation_status_is_stale_verified(status, now, stale_verified_seconds)
        {
            runtime_broker_add_continuation_signal(&mut metrics.stale_verified_bindings, kind, 1);
        }
    }
    metrics
}

pub fn runtime_broker_previous_response_continuity_metrics(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    negative_cache_decay_seconds: i64,
) -> RuntimeBrokerPreviousResponseContinuityMetrics {
    const PREFIX: &str = "__previous_response_not_found__:";

    let mut metrics = RuntimeBrokerPreviousResponseContinuityMetrics::default();
    for (key, entry) in profile_health {
        let Some(rest) = key.strip_prefix(PREFIX) else {
            continue;
        };
        let Some((route, _)) = rest.split_once(':') else {
            continue;
        };
        let score = runtime_broker_effective_score(entry, now, negative_cache_decay_seconds);
        if score == 0 {
            continue;
        }
        if runtime_broker_add_route_continuity_signal(&mut metrics.negative_cache_entries, route, 1)
        {
            let _ = runtime_broker_add_route_continuity_signal(
                &mut metrics.negative_cache_failures,
                route,
                score as usize,
            );
        }
    }
    metrics
}

pub fn runtime_broker_add_route_continuity_signal(
    metrics: &mut RuntimeBrokerRouteContinuityMetrics,
    route: &str,
    value: usize,
) -> bool {
    use prodex_mojo_core::runtime_broker_continuity::ContinuityRouteKind;
    match prodex_mojo_core::runtime_broker_continuity::route_kind(route)
        .expect("Mojo broker route classification returned invalid output")
    {
        ContinuityRouteKind::Responses => metrics.responses += value,
        ContinuityRouteKind::Compact => metrics.compact += value,
        ContinuityRouteKind::Websocket => metrics.websocket += value,
        ContinuityRouteKind::Standard => metrics.standard += value,
        ContinuityRouteKind::Unknown => return false,
    }
    true
}

pub fn runtime_broker_merge_continuity_failure_reason_metrics(
    metrics: &mut RuntimeBrokerContinuityFailureReasonMetrics,
    delta: RuntimeBrokerContinuityFailureReasonMetrics,
) {
    for (reason, count) in delta.chain_retried_owner {
        *metrics.chain_retried_owner.entry(reason).or_insert(0) += count;
    }
    for (reason, count) in delta.chain_dead_upstream_confirmed {
        *metrics
            .chain_dead_upstream_confirmed
            .entry(reason)
            .or_insert(0) += count;
    }
    for (reason, count) in delta.stale_continuation {
        *metrics.stale_continuation.entry(reason).or_insert(0) += count;
    }
}

pub fn runtime_broker_continuity_failure_reason_metrics_from_log_bytes(
    log: &[u8],
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    use prodex_mojo_core::runtime_broker_continuity::{ContinuityEvent, ContinuityReasonSource};

    let text = String::from_utf8_lossy(log);
    let mut metrics = RuntimeBrokerContinuityFailureReasonMetrics::default();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let parsed = trimmed
            .starts_with('{')
            .then(|| serde_json::from_str::<serde_json::Value>(trimmed).ok())
            .flatten();
        let event = parsed
            .as_ref()
            .and_then(|value| runtime_broker_json_string_field(value, "event"));
        let reason = parsed
            .as_ref()
            .and_then(|value| runtime_broker_json_string_field(value, "reason"));
        let message = parsed
            .as_ref()
            .and_then(|value| runtime_broker_json_string_field(value, "message"));
        let plan = prodex_mojo_core::runtime_broker_continuity::continuity_line_plan(
            line, event, reason, message,
        )
        .expect("Mojo broker continuity parser returned invalid output");
        let (Some(event), Some(source)) = (plan.event, plan.reason_source) else {
            continue;
        };
        let reason = match source {
            ContinuityReasonSource::DirectReason => reason.map(str::to_string),
            ContinuityReasonSource::Message => message.and_then(|message| {
                runtime_broker_continuity_reason_from_span(
                    message,
                    plan.reason_start,
                    plan.reason_length,
                )
            }),
            ContinuityReasonSource::RawLine => runtime_broker_continuity_reason_from_span(
                line,
                plan.reason_start,
                plan.reason_length,
            ),
        };
        let Some(reason) = reason else {
            continue;
        };
        match event {
            ContinuityEvent::ChainRetriedOwner => {
                *metrics.chain_retried_owner.entry(reason).or_insert(0) += 1;
            }
            ContinuityEvent::ChainDeadUpstreamConfirmed => {
                *metrics
                    .chain_dead_upstream_confirmed
                    .entry(reason)
                    .or_insert(0) += 1;
            }
            ContinuityEvent::StaleContinuation => {
                *metrics.stale_continuation.entry(reason).or_insert(0) += 1;
            }
        }
    }
    metrics
}

fn runtime_broker_json_string_field<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Option<&'a str> {
    match value {
        serde_json::Value::Object(map) => {
            map.get(key)
                .and_then(serde_json::Value::as_str)
                .or_else(|| {
                    map.values()
                        .find_map(|value| runtime_broker_json_string_field(value, key))
                })
        }
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| runtime_broker_json_string_field(value, key)),
        _ => None,
    }
}

fn runtime_broker_continuity_reason_from_span(
    value: &str,
    start: usize,
    length: usize,
) -> Option<String> {
    let end = start.checked_add(length)?;
    let raw = value.get(start..end)?;
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        serde_json::from_str::<String>(raw)
            .ok()
            .or_else(|| Some(raw.trim_matches('"').to_string()))
    } else {
        Some(raw.trim_matches('"').to_string())
    }
}

pub fn runtime_broker_continuity_failure_reason_metrics_with_live(
    parsed_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    baseline_metrics: &RuntimeBrokerContinuityFailureReasonMetrics,
    live_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    let persisted_since_baseline = runtime_broker_subtract_continuity_failure_reason_metrics(
        parsed_metrics.clone(),
        baseline_metrics,
    );
    let pending_live = runtime_broker_subtract_continuity_failure_reason_metrics(
        live_metrics,
        &persisted_since_baseline,
    );
    let mut merged = parsed_metrics;
    runtime_broker_merge_continuity_failure_reason_metrics(&mut merged, pending_live);
    merged
}

pub fn runtime_broker_subtract_continuity_failure_reason_metrics(
    metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    delta: &RuntimeBrokerContinuityFailureReasonMetrics,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    RuntimeBrokerContinuityFailureReasonMetrics {
        chain_retried_owner: runtime_broker_subtract_reason_metrics(
            metrics.chain_retried_owner,
            &delta.chain_retried_owner,
        ),
        chain_dead_upstream_confirmed: runtime_broker_subtract_reason_metrics(
            metrics.chain_dead_upstream_confirmed,
            &delta.chain_dead_upstream_confirmed,
        ),
        stale_continuation: runtime_broker_subtract_reason_metrics(
            metrics.stale_continuation,
            &delta.stale_continuation,
        ),
    }
}

fn runtime_broker_add_continuation_signal(
    metrics: &mut RuntimeBrokerContinuationSignalMetrics,
    kind: RuntimeBrokerContinuationBindingKind,
    value: usize,
) {
    match kind {
        RuntimeBrokerContinuationBindingKind::Response => metrics.response += value,
        RuntimeBrokerContinuationBindingKind::TurnState => metrics.turn_state += value,
        RuntimeBrokerContinuationBindingKind::SessionId => metrics.session_id += value,
    }
}

fn runtime_broker_continuation_status_is_stale_verified(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    stale_verified_seconds: i64,
) -> bool {
    prodex_mojo_core::runtime_broker_continuity::stale_verified(
        status.state == RuntimeContinuationBindingLifecycle::Verified,
        status.last_not_found_at,
        status.last_verified_at,
        status.last_touched_at,
        now,
        stale_verified_seconds,
    )
    .expect("Mojo broker stale-continuation policy returned invalid output")
}

fn runtime_broker_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    prodex_mojo_core::runtime_broker_continuity::effective_score(
        entry.score,
        entry.updated_at,
        now,
        decay_seconds,
    )
    .expect("Mojo broker health decay policy returned invalid output")
}

fn runtime_broker_subtract_reason_metrics(
    metrics: BTreeMap<String, usize>,
    delta: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    metrics
        .into_iter()
        .filter_map(|(reason, count)| {
            let remaining = count.saturating_sub(delta.get(&reason).copied().unwrap_or_default());
            (remaining > 0).then_some((reason, remaining))
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeBrokerDegradedHealthMetrics {
    pub profiles: usize,
    pub routes: usize,
}

pub fn runtime_broker_degraded_health_metrics(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    health_decay_seconds: i64,
) -> RuntimeBrokerDegradedHealthMetrics {
    let mut metrics = RuntimeBrokerDegradedHealthMetrics::default();
    for (key, entry) in profile_health {
        if runtime_broker_effective_score(entry, now, health_decay_seconds) == 0 {
            continue;
        }
        use prodex_mojo_core::runtime_broker_continuity::HealthKeyKind;
        match prodex_mojo_core::runtime_broker_continuity::health_key_kind(key)
            .expect("Mojo broker health-key classification returned invalid output")
        {
            HealthKeyKind::Route => metrics.routes += 1,
            HealthKeyKind::Profile => metrics.profiles += 1,
            HealthKeyKind::Ignore => {}
        }
    }
    metrics
}
