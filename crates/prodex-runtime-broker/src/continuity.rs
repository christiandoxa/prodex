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
    match route {
        "responses" => metrics.responses += value,
        "compact" => metrics.compact += value,
        "websocket" => metrics.websocket += value,
        "standard" => metrics.standard += value,
        _ => return false,
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
    let text = String::from_utf8_lossy(log);
    let mut metrics = RuntimeBrokerContinuityFailureReasonMetrics::default();
    for line in text.lines() {
        let Some(event) = runtime_broker_continuity_failure_event(line) else {
            continue;
        };
        let Some(reason) = runtime_broker_continuity_failure_reason(line) else {
            continue;
        };
        match event {
            "chain_retried_owner" => {
                *metrics.chain_retried_owner.entry(reason).or_insert(0) += 1;
            }
            "chain_dead_upstream_confirmed" => {
                *metrics
                    .chain_dead_upstream_confirmed
                    .entry(reason)
                    .or_insert(0) += 1;
            }
            "stale_continuation" => {
                *metrics.stale_continuation.entry(reason).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    metrics
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

fn runtime_broker_continuation_status_last_event_at(
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

fn runtime_broker_continuation_status_is_stale_verified(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    stale_verified_seconds: i64,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Verified
        && runtime_broker_continuation_status_last_event_at(status)
            .is_some_and(|last| now.saturating_sub(last) >= stale_verified_seconds)
}

fn runtime_broker_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
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

fn runtime_broker_continuity_failure_event(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('{')
        && let Some(event) = runtime_broker_json_string_field(trimmed, "event")
        && let Some(event) = runtime_broker_known_continuity_failure_event(&event)
    {
        return Some(event);
    }
    if trimmed.starts_with('{')
        && let Some(message) = runtime_broker_json_string_field(trimmed, "message")
        && let Some(event) = message
            .split_whitespace()
            .find_map(runtime_broker_known_continuity_failure_event)
    {
        return Some(event);
    }
    line.split_whitespace()
        .find_map(runtime_broker_known_continuity_failure_event)
}

fn runtime_broker_continuity_failure_reason(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('{')
        && let Some(reason) = runtime_broker_json_string_field(trimmed, "reason")
    {
        return Some(reason);
    }
    if trimmed.starts_with('{')
        && let Some(message) = runtime_broker_json_string_field(trimmed, "message")
        && let Some(reason) = runtime_broker_log_field_value(&message, "reason")
    {
        return Some(reason);
    }
    runtime_broker_log_field_value(line, "reason")
}

fn runtime_broker_known_continuity_failure_event(token: &str) -> Option<&'static str> {
    match token {
        "chain_retried_owner" => Some("chain_retried_owner"),
        "chain_dead_upstream_confirmed" => Some("chain_dead_upstream_confirmed"),
        "stale_continuation" => Some("stale_continuation"),
        _ => None,
    }
}

fn runtime_broker_json_string_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index < line.len() {
        let found = line[index..].find(&needle)?;
        index += found + needle.len();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b':') {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'"') {
            continue;
        }
        return runtime_broker_parse_json_string(line, index);
    }
    None
}

fn runtime_broker_parse_json_string(line: &str, quote_index: usize) -> Option<String> {
    let bytes = line.as_bytes();
    if bytes.get(quote_index) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut index = quote_index + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some(out),
            b'\\' => {
                index += 1;
                match bytes.get(index).copied()? {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex_start = index + 1;
                        let hex_end = hex_start + 4;
                        let value = line.get(hex_start..hex_end)?;
                        if let Ok(codepoint) = u32::from_str_radix(value, 16)
                            && let Some(ch) = char::from_u32(codepoint)
                        {
                            out.push(ch);
                        }
                        index = hex_end - 1;
                    }
                    other => out.push(other as char),
                }
            }
            _ => {
                let rest = line.get(index..)?;
                let ch = rest.chars().next()?;
                out.push(ch);
                index += ch.len_utf8() - 1;
            }
        }
        index += 1;
    }
    None
}

fn runtime_broker_log_field_value(message: &str, target_key: &str) -> Option<String> {
    let bytes = message.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        index = runtime_broker_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }

        let key_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            continue;
        }
        let key = &message[key_start..index];
        index += 1;
        let value_start = index;
        index = runtime_broker_skip_log_field_value(message, index);
        if key == target_key {
            return Some(runtime_broker_parse_log_field_value(
                &message[value_start..index],
            ));
        }
    }
    None
}

fn runtime_broker_skip_log_whitespace(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_broker_skip_log_field_value(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    if bytes.get(index) == Some(&b'"') {
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    index += 1;
                }
                b'"' => return index + 1,
                _ => index += 1,
            }
        }
        return index;
    }
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_broker_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') && raw_value.ends_with('"') && raw_value.len() >= 2 {
        runtime_broker_parse_json_string(raw_value, 0)
            .unwrap_or_else(|| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
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
        if key.starts_with("__route_health__:") {
            metrics.routes += 1;
        } else if !key.starts_with("__") {
            metrics.profiles += 1;
        }
    }
    metrics
}
