use super::*;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub(crate) struct RuntimeDoctorPolicySettingSuggestion {
    pub(crate) section: String,
    pub(crate) key: String,
    pub(crate) current_value: u64,
    pub(crate) suggested_value: u64,
    pub(crate) rationale: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub(crate) struct RuntimeDoctorPolicySuggestion {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) severity: String,
    pub(crate) reason: String,
    pub(crate) markers: Vec<String>,
    pub(crate) settings: Vec<RuntimeDoctorPolicySettingSuggestion>,
    pub(crate) snippet: String,
}

fn runtime_doctor_marker_count(summary: &RuntimeDoctorSummary, marker: &'static str) -> usize {
    summary.marker_counts.get(marker).copied().unwrap_or(0)
}

fn runtime_doctor_marker_last_field<'a>(
    summary: &'a RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<&'a str> {
    summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| fields.get(field))
        .map(String::as_str)
}

fn runtime_doctor_marker_last_usize_field(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<usize> {
    runtime_doctor_marker_last_field(summary, marker, field)?
        .parse()
        .ok()
}

fn runtime_doctor_scaled_up(value: usize) -> usize {
    value
        .saturating_add(value.saturating_add(1) / 2)
        .max(value.saturating_add(1))
        .max(1)
}

fn runtime_doctor_scaled_down(value: usize) -> usize {
    value.saturating_mul(3).saturating_add(3) / 4
}

fn runtime_doctor_suggested_u64(value: usize) -> u64 {
    value.max(1) as u64
}

fn runtime_doctor_policy_setting(
    key: &str,
    current_value: usize,
    suggested_value: usize,
    rationale: impl Into<String>,
) -> RuntimeDoctorPolicySettingSuggestion {
    RuntimeDoctorPolicySettingSuggestion {
        section: "runtime_proxy".to_string(),
        key: key.to_string(),
        current_value: current_value as u64,
        suggested_value: runtime_doctor_suggested_u64(suggested_value),
        rationale: rationale.into(),
    }
}

fn runtime_doctor_policy_setting_u64(
    key: &str,
    current_value: u64,
    suggested_value: u64,
    rationale: impl Into<String>,
) -> RuntimeDoctorPolicySettingSuggestion {
    RuntimeDoctorPolicySettingSuggestion {
        section: "runtime_proxy".to_string(),
        key: key.to_string(),
        current_value,
        suggested_value: suggested_value.max(1),
        rationale: rationale.into(),
    }
}

fn runtime_doctor_policy_snippet(settings: &[RuntimeDoctorPolicySettingSuggestion]) -> String {
    let mut lines = vec!["[runtime_proxy]".to_string()];
    for setting in settings {
        lines.push(format!("{} = {}", setting.key, setting.suggested_value));
    }
    lines.join("\n")
}

fn runtime_doctor_policy_suggestion(
    id: &str,
    title: &str,
    severity: &str,
    reason: impl Into<String>,
    markers: &[&str],
    settings: Vec<RuntimeDoctorPolicySettingSuggestion>,
) -> RuntimeDoctorPolicySuggestion {
    RuntimeDoctorPolicySuggestion {
        id: id.to_string(),
        title: title.to_string(),
        severity: severity.to_string(),
        reason: reason.into(),
        markers: markers.iter().map(|marker| (*marker).to_string()).collect(),
        snippet: runtime_doctor_policy_snippet(&settings),
        settings,
    }
}

fn runtime_doctor_lane_policy_key(lane: &str) -> Option<(&'static str, usize)> {
    let snapshot = collect_runtime_tuning_snapshot();
    match lane {
        "responses" => Some(("responses_active_limit", snapshot.lane_limits.responses)),
        "compact" => Some(("compact_active_limit", snapshot.lane_limits.compact)),
        "websocket" => Some(("websocket_active_limit", snapshot.lane_limits.websocket)),
        "standard" => Some(("standard_active_limit", snapshot.lane_limits.standard)),
        _ => None,
    }
}

fn runtime_doctor_lane_pressure_suggestion(
    summary: &RuntimeDoctorSummary,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let count = runtime_doctor_marker_count(summary, "runtime_proxy_lane_limit_reached");
    if count == 0 {
        return None;
    }
    let snapshot = collect_runtime_tuning_snapshot();
    let lane =
        runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
            .unwrap_or("responses");
    let (lane_key, current_lane_limit) = runtime_doctor_lane_policy_key(lane)?;
    let observed_active = runtime_doctor_marker_last_usize_field(
        summary,
        "runtime_proxy_lane_limit_reached",
        "active",
    )
    .unwrap_or(current_lane_limit);
    let observed_limit = runtime_doctor_marker_last_usize_field(
        summary,
        "runtime_proxy_lane_limit_reached",
        "limit",
    )
    .unwrap_or(current_lane_limit);
    let target_lane_limit = runtime_doctor_scaled_up(
        current_lane_limit
            .max(observed_limit)
            .max(observed_active.saturating_add(1)),
    );
    let mut settings = vec![runtime_doctor_policy_setting(
        lane_key,
        current_lane_limit,
        target_lane_limit,
        format!("raise the {lane} lane cap after repeated lane-limit markers"),
    )];
    if target_lane_limit >= snapshot.active_request_limit {
        settings.push(runtime_doctor_policy_setting(
            "active_request_limit",
            snapshot.active_request_limit,
            target_lane_limit.saturating_add(2),
            "keep the global admission cap above the suggested lane cap",
        ));
    }
    Some(runtime_doctor_policy_suggestion(
        "lane_pressure",
        "Lane pressure",
        "medium",
        format!(
            "{count} lane-limit marker(s) on lane={lane}; apply only if host/network headroom exists"
        ),
        &["runtime_proxy_lane_limit_reached"],
        settings,
    ))
}

fn runtime_doctor_active_pressure_suggestion(
    summary: &RuntimeDoctorSummary,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let count = runtime_doctor_marker_count(summary, "runtime_proxy_active_limit_reached");
    if count == 0 {
        return None;
    }
    let snapshot = collect_runtime_tuning_snapshot();
    let observed_active = runtime_doctor_marker_last_usize_field(
        summary,
        "runtime_proxy_active_limit_reached",
        "active",
    )
    .unwrap_or(snapshot.active_request_limit);
    let observed_limit = runtime_doctor_marker_last_usize_field(
        summary,
        "runtime_proxy_active_limit_reached",
        "limit",
    )
    .unwrap_or(snapshot.active_request_limit);
    let target = runtime_doctor_scaled_up(
        snapshot
            .active_request_limit
            .max(observed_limit)
            .max(observed_active.saturating_add(1)),
    );
    Some(runtime_doctor_policy_suggestion(
        "active_request_pressure",
        "Active request pressure",
        "medium",
        format!(
            "{count} global active-limit marker(s); raise only if local CPU/network is not saturated"
        ),
        &["runtime_proxy_active_limit_reached"],
        vec![runtime_doctor_policy_setting(
            "active_request_limit",
            snapshot.active_request_limit,
            target,
            "allow more pre-commit requests through local admission",
        )],
    ))
}

fn runtime_doctor_profile_inflight_suggestion(
    summary: &RuntimeDoctorSummary,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let count = runtime_doctor_marker_count(summary, "profile_inflight_saturated");
    if count == 0 {
        return None;
    }
    let snapshot = collect_runtime_tuning_snapshot();
    let profile =
        runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "profile")
            .unwrap_or("unknown");
    let observed_hard =
        runtime_doctor_marker_last_usize_field(summary, "profile_inflight_saturated", "hard_limit")
            .unwrap_or(snapshot.profile_inflight_hard_limit);
    let target_hard = runtime_doctor_scaled_up(
        snapshot
            .profile_inflight_hard_limit
            .max(observed_hard)
            .max(1),
    );
    let target_soft = runtime_doctor_scaled_up(snapshot.profile_inflight_soft_limit)
        .min(target_hard.saturating_sub(1).max(1))
        .max(1);
    Some(runtime_doctor_policy_suggestion(
        "profile_inflight_saturation",
        "Profile in-flight saturation",
        "medium",
        format!(
            "{count} per-profile in-flight saturation marker(s), latest profile={profile}; raise only if account fan-out is intentional"
        ),
        &["profile_inflight_saturated"],
        vec![
            runtime_doctor_policy_setting(
                "profile_inflight_soft_limit",
                snapshot.profile_inflight_soft_limit,
                target_soft,
                "delay soft load penalty until a profile has more concurrent work",
            ),
            runtime_doctor_policy_setting(
                "profile_inflight_hard_limit",
                snapshot.profile_inflight_hard_limit,
                target_hard,
                "raise the fresh-selection hard cap for a busy profile",
            ),
        ],
    ))
}

fn runtime_doctor_latest_marker<'a>(
    summary: &RuntimeDoctorSummary,
    markers: &'a [&'static str],
) -> Option<&'a str> {
    markers
        .iter()
        .copied()
        .find(|marker| runtime_doctor_marker_count(summary, marker) > 0)
}

struct RuntimeDoctorWebsocketExecutorSuggestionInput<'a> {
    id: &'a str,
    title: &'a str,
    markers: &'a [&'static str],
    worker_key: &'a str,
    queue_key: &'a str,
    overflow_key: &'a str,
    current_worker_count: usize,
    current_queue_capacity: usize,
    current_overflow_capacity: usize,
}

fn runtime_doctor_websocket_executor_suggestion(
    summary: &RuntimeDoctorSummary,
    input: RuntimeDoctorWebsocketExecutorSuggestionInput<'_>,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let count = input
        .markers
        .iter()
        .map(|marker| runtime_doctor_marker_count(summary, marker))
        .sum::<usize>();
    if count == 0 {
        return None;
    }
    let marker = runtime_doctor_latest_marker(summary, input.markers)?;
    let observed_worker = runtime_doctor_marker_last_usize_field(summary, marker, "worker_count")
        .unwrap_or(input.current_worker_count);
    let observed_queue = runtime_doctor_marker_last_usize_field(summary, marker, "queue_capacity")
        .unwrap_or(input.current_queue_capacity);
    let observed_pending =
        runtime_doctor_marker_last_usize_field(summary, marker, "overflow_pending").unwrap_or(0);
    let observed_max_pending =
        runtime_doctor_marker_last_usize_field(summary, marker, "overflow_max_pending")
            .unwrap_or(0);
    let target_worker = runtime_doctor_scaled_up(input.current_worker_count.max(observed_worker));
    let target_queue = runtime_doctor_scaled_up(
        input
            .current_queue_capacity
            .max(observed_queue)
            .max(target_worker),
    );
    let overflow_base = input
        .current_overflow_capacity
        .max(observed_pending)
        .max(observed_max_pending)
        .max(target_queue);
    let target_overflow = runtime_doctor_scaled_up(overflow_base);
    Some(runtime_doctor_policy_suggestion(
        input.id,
        input.title,
        "medium",
        format!(
            "{count} websocket executor overflow marker(s), latest={marker}; raise only for bursty session starts"
        ),
        input.markers,
        vec![
            runtime_doctor_policy_setting(
                input.worker_key,
                input.current_worker_count,
                target_worker,
                "increase bounded executor parallelism",
            ),
            runtime_doctor_policy_setting(
                input.queue_key,
                input.current_queue_capacity,
                target_queue,
                "increase bounded executor queue capacity",
            ),
            runtime_doctor_policy_setting(
                input.overflow_key,
                input.current_overflow_capacity,
                target_overflow,
                "increase burst overflow buffering after the bounded queue fills",
            ),
        ],
    ))
}

fn runtime_doctor_websocket_connect_suggestion(
    summary: &RuntimeDoctorSummary,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let snapshot = collect_runtime_tuning_snapshot();
    runtime_doctor_websocket_executor_suggestion(
        summary,
        RuntimeDoctorWebsocketExecutorSuggestionInput {
            id: "websocket_connect_overflow",
            title: "Websocket connect overflow",
            markers: &[
                "websocket_connect_overflow_rejected",
                "websocket_connect_overflow_reject",
                "websocket_connect_overflow_enqueue",
                "websocket_connect_overflow_dispatch",
            ],
            worker_key: "websocket_connect_worker_count",
            queue_key: "websocket_connect_queue_capacity",
            overflow_key: "websocket_connect_overflow_capacity",
            current_worker_count: snapshot.websocket_connect_worker_count,
            current_queue_capacity: snapshot.websocket_connect_queue_capacity,
            current_overflow_capacity: snapshot.websocket_connect_overflow_capacity,
        },
    )
}

fn runtime_doctor_websocket_dns_suggestion(
    summary: &RuntimeDoctorSummary,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let snapshot = collect_runtime_tuning_snapshot();
    runtime_doctor_websocket_executor_suggestion(
        summary,
        RuntimeDoctorWebsocketExecutorSuggestionInput {
            id: "websocket_dns_overflow",
            title: "Websocket DNS overflow",
            markers: &[
                "websocket_dns_overflow_reject",
                "websocket_dns_overflow_enqueue",
                "websocket_dns_overflow_dispatch",
            ],
            worker_key: "websocket_dns_worker_count",
            queue_key: "websocket_dns_queue_capacity",
            overflow_key: "websocket_dns_overflow_capacity",
            current_worker_count: snapshot.websocket_dns_worker_count,
            current_queue_capacity: snapshot.websocket_dns_queue_capacity,
            current_overflow_capacity: snapshot.websocket_dns_overflow_capacity,
        },
    )
}

fn runtime_doctor_persistence_suggestion(
    summary: &RuntimeDoctorSummary,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let state_count = runtime_doctor_marker_count(summary, "state_save_queue_backpressure");
    let journal_count =
        runtime_doctor_marker_count(summary, "continuation_journal_queue_backpressure");
    if state_count + journal_count == 0 {
        return None;
    }
    let snapshot = collect_runtime_tuning_snapshot();
    let target_compact = runtime_doctor_scaled_down(snapshot.lane_limits.compact).max(1);
    let target_standard = runtime_doctor_scaled_down(snapshot.lane_limits.standard).max(1);
    let target_pressure_wait = snapshot
        .pressure_admission_wait_budget_ms
        .max(snapshot.admission_wait_budget_ms)
        .saturating_add(500);
    Some(runtime_doctor_policy_suggestion(
        "persistence_backpressure",
        "Persistence backpressure",
        "medium",
        format!(
            "state-save backpressure={state_count}, continuation-journal backpressure={journal_count}; throttle churn while queues drain"
        ),
        &[
            "state_save_queue_backpressure",
            "continuation_journal_queue_backpressure",
        ],
        vec![
            runtime_doctor_policy_setting(
                "compact_active_limit",
                snapshot.lane_limits.compact,
                target_compact,
                "reduce fresh compact churn that creates continuation state writes",
            ),
            runtime_doctor_policy_setting(
                "standard_active_limit",
                snapshot.lane_limits.standard,
                target_standard,
                "reduce side-lane churn while persistence is behind",
            ),
            runtime_doctor_policy_setting_u64(
                "pressure_admission_wait_budget_ms",
                snapshot.pressure_admission_wait_budget_ms,
                target_pressure_wait,
                "let pressure-mode admission wait briefly for queues to drain",
            ),
        ],
    ))
}

fn runtime_doctor_route_health_suggestion(
    summary: &RuntimeDoctorSummary,
) -> Option<RuntimeDoctorPolicySuggestion> {
    let count = runtime_doctor_marker_count(summary, "profile_health");
    if count == 0 {
        return None;
    }
    let snapshot = collect_runtime_tuning_snapshot();
    let profile =
        runtime_doctor_marker_last_field(summary, "profile_health", "profile").unwrap_or("unknown");
    let route =
        runtime_doctor_marker_last_field(summary, "profile_health", "route").unwrap_or("unknown");
    let reason =
        runtime_doctor_marker_last_field(summary, "profile_health", "reason").unwrap_or("unknown");
    let target_soft = runtime_doctor_scaled_down(snapshot.profile_inflight_soft_limit).max(1);
    let target_hard = runtime_doctor_scaled_down(snapshot.profile_inflight_hard_limit)
        .max(target_soft.saturating_add(1));
    Some(runtime_doctor_policy_suggestion(
        "route_scoped_profile_health",
        "Route-scoped profile health",
        "low",
        format!(
            "{count} route-scoped health marker(s), latest={profile}/{route} reason={reason}; lower per-profile fresh pressure if this repeats"
        ),
        &["profile_health"],
        vec![
            runtime_doctor_policy_setting(
                "profile_inflight_soft_limit",
                snapshot.profile_inflight_soft_limit,
                target_soft,
                "spread fresh work away from accounts accumulating route-specific health penalties",
            ),
            runtime_doctor_policy_setting(
                "profile_inflight_hard_limit",
                snapshot.profile_inflight_hard_limit,
                target_hard,
                "cap fresh work per profile more tightly while route health recovers",
            ),
        ],
    ))
}

pub(crate) fn runtime_doctor_policy_suggestions(
    summary: &RuntimeDoctorSummary,
) -> Vec<RuntimeDoctorPolicySuggestion> {
    [
        runtime_doctor_lane_pressure_suggestion(summary),
        runtime_doctor_active_pressure_suggestion(summary),
        runtime_doctor_profile_inflight_suggestion(summary),
        runtime_doctor_websocket_connect_suggestion(summary),
        runtime_doctor_websocket_dns_suggestion(summary),
        runtime_doctor_persistence_suggestion(summary),
        runtime_doctor_route_health_suggestion(summary),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(crate) fn runtime_doctor_policy_suggestion_lines(
    suggestions: &[RuntimeDoctorPolicySuggestion],
) -> Vec<String> {
    let mut lines = vec!["Runtime Policy Suggestions".to_string()];
    if suggestions.is_empty() {
        lines.push("No policy.toml suggestion matched the sampled runtime markers.".to_string());
        return lines;
    }
    for suggestion in suggestions {
        lines.push(format!("- {}: {}", suggestion.title, suggestion.reason));
        lines.push("  policy.toml:".to_string());
        for line in suggestion.snippet.lines() {
            lines.push(format!("  {line}"));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIVE_REQUEST_PRESSURE_LOG: &[u8] =
        include_bytes!("../../tests/fixtures/runtime_doctor/active_request_pressure.log");
    const LANE_PRESSURE_LOG: &[u8] =
        include_bytes!("../../tests/fixtures/runtime_doctor/lane_pressure.log");
    const PERSISTENCE_BACKPRESSURE_LOG: &[u8] =
        include_bytes!("../../tests/fixtures/runtime_doctor/persistence_backpressure.log");
    const PROFILE_INFLIGHT_SATURATION_LOG: &[u8] =
        include_bytes!("../../tests/fixtures/runtime_doctor/profile_inflight_saturation.log");
    const ROUTE_SCOPED_PROFILE_HEALTH_LOG: &[u8] =
        include_bytes!("../../tests/fixtures/runtime_doctor/route_scoped_profile_health.log");
    const WEBSOCKET_CONNECT_OVERFLOW_LOG: &[u8] =
        include_bytes!("../../tests/fixtures/runtime_doctor/websocket_connect_overflow.log");
    const WEBSOCKET_DNS_OVERFLOW_LOG: &[u8] =
        include_bytes!("../../tests/fixtures/runtime_doctor/websocket_dns_overflow.log");

    fn runtime_doctor_fixture_suggestions(log: &[u8]) -> Vec<RuntimeDoctorPolicySuggestion> {
        let mut summary = summarize_runtime_log_tail(log);
        summary.pointer_exists = true;
        summary.log_exists = true;
        diagnosis::runtime_doctor_finalize_summary(&mut summary);
        runtime_doctor_policy_suggestions(&summary)
    }

    fn runtime_doctor_suggestion<'a>(
        suggestions: &'a [RuntimeDoctorPolicySuggestion],
        id: &str,
    ) -> &'a RuntimeDoctorPolicySuggestion {
        suggestions
            .iter()
            .find(|suggestion| suggestion.id == id)
            .unwrap_or_else(|| panic!("missing suggestion {id}: {suggestions:#?}"))
    }

    fn runtime_doctor_setting<'a>(
        suggestion: &'a RuntimeDoctorPolicySuggestion,
        key: &str,
    ) -> &'a RuntimeDoctorPolicySettingSuggestion {
        suggestion
            .settings
            .iter()
            .find(|setting| setting.key == key)
            .unwrap_or_else(|| panic!("missing setting {key}: {suggestion:#?}"))
    }

    #[test]
    fn runtime_doctor_policy_suggestions_cover_pressure_fixture_logs() {
        let suggestions = runtime_doctor_fixture_suggestions(LANE_PRESSURE_LOG);
        let lane = runtime_doctor_suggestion(&suggestions, "lane_pressure");
        assert_eq!(lane.markers, vec!["runtime_proxy_lane_limit_reached"]);
        assert!(lane.snippet.contains("compact_active_limit = "));
        assert!(
            runtime_doctor_setting(lane, "compact_active_limit").suggested_value
                > runtime_doctor_setting(lane, "compact_active_limit").current_value
        );

        let suggestions = runtime_doctor_fixture_suggestions(ACTIVE_REQUEST_PRESSURE_LOG);
        let active = runtime_doctor_suggestion(&suggestions, "active_request_pressure");
        assert!(active.snippet.contains("active_request_limit = "));
        assert!(
            runtime_doctor_setting(active, "active_request_limit").suggested_value
                > runtime_doctor_setting(active, "active_request_limit").current_value
        );

        let suggestions = runtime_doctor_fixture_suggestions(PROFILE_INFLIGHT_SATURATION_LOG);
        let inflight = runtime_doctor_suggestion(&suggestions, "profile_inflight_saturation");
        assert!(inflight.snippet.contains("profile_inflight_hard_limit = "));
        assert!(
            runtime_doctor_setting(inflight, "profile_inflight_hard_limit").suggested_value
                > runtime_doctor_setting(inflight, "profile_inflight_hard_limit").current_value
        );
    }

    #[test]
    fn runtime_doctor_policy_suggestions_cover_websocket_and_persistence_fixture_logs() {
        let suggestions = runtime_doctor_fixture_suggestions(WEBSOCKET_CONNECT_OVERFLOW_LOG);
        let connect = runtime_doctor_suggestion(&suggestions, "websocket_connect_overflow");
        assert!(
            connect
                .snippet
                .contains("websocket_connect_worker_count = ")
        );
        assert!(
            connect
                .snippet
                .contains("websocket_connect_queue_capacity = ")
        );
        assert!(
            connect
                .snippet
                .contains("websocket_connect_overflow_capacity = ")
        );

        let suggestions = runtime_doctor_fixture_suggestions(WEBSOCKET_DNS_OVERFLOW_LOG);
        let dns = runtime_doctor_suggestion(&suggestions, "websocket_dns_overflow");
        assert!(dns.snippet.contains("websocket_dns_worker_count = "));
        assert!(dns.snippet.contains("websocket_dns_queue_capacity = "));
        assert!(dns.snippet.contains("websocket_dns_overflow_capacity = "));

        let suggestions = runtime_doctor_fixture_suggestions(PERSISTENCE_BACKPRESSURE_LOG);
        let persistence = runtime_doctor_suggestion(&suggestions, "persistence_backpressure");
        assert!(persistence.snippet.contains("compact_active_limit = "));
        assert!(persistence.snippet.contains("standard_active_limit = "));
        assert!(
            persistence
                .snippet
                .contains("pressure_admission_wait_budget_ms = ")
        );
    }

    #[test]
    fn runtime_doctor_policy_suggestions_cover_route_scoped_health_fixture_log() {
        let suggestions = runtime_doctor_fixture_suggestions(ROUTE_SCOPED_PROFILE_HEALTH_LOG);
        let health = runtime_doctor_suggestion(&suggestions, "route_scoped_profile_health");

        assert!(health.reason.contains("alpha/responses"));
        assert!(health.snippet.contains("profile_inflight_soft_limit = "));
        assert!(health.snippet.contains("profile_inflight_hard_limit = "));
        assert!(
            runtime_doctor_setting(health, "profile_inflight_soft_limit").suggested_value
                <= runtime_doctor_setting(health, "profile_inflight_soft_limit").current_value
        );
    }

    #[test]
    fn runtime_doctor_policy_suggestion_lines_render_snippet() {
        let suggestions = runtime_doctor_fixture_suggestions(LANE_PRESSURE_LOG);
        let lines = runtime_doctor_policy_suggestion_lines(&suggestions);

        assert_eq!(lines[0], "Runtime Policy Suggestions");
        assert!(lines.iter().any(|line| line.contains("Lane pressure")));
        assert!(lines.iter().any(|line| line.trim() == "[runtime_proxy]"));
        assert!(
            lines
                .iter()
                .any(|line| line.trim_start().starts_with("compact_active_limit = "))
        );
    }
}
