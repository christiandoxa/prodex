use super::*;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorPolicySettingSuggestion {
    pub section: String,
    pub key: String,
    pub current_value: u64,
    pub suggested_value: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorPolicySuggestion {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub reason: String,
    pub markers: Vec<String>,
    pub settings: Vec<RuntimeDoctorPolicySettingSuggestion>,
    pub snippet: String,
}

#[cfg(not(feature = "mojo"))]
mod rust_fallback {
    use super::*;

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

    fn runtime_doctor_lane_policy_key(
        lane: &str,
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<(&'static str, usize)> {
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
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<RuntimeDoctorPolicySuggestion> {
        let count = runtime_doctor_marker_count(summary, "runtime_proxy_lane_limit_reached");
        if count == 0 {
            return None;
        }
        let lane =
            runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
                .unwrap_or("responses");
        let (lane_key, current_lane_limit) = runtime_doctor_lane_policy_key(lane, snapshot)?;
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
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<RuntimeDoctorPolicySuggestion> {
        let count = runtime_doctor_marker_count(summary, "runtime_proxy_active_limit_reached");
        if count == 0 {
            return None;
        }
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
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<RuntimeDoctorPolicySuggestion> {
        let count = runtime_doctor_marker_count(summary, "profile_inflight_saturated");
        if count == 0 {
            return None;
        }
        let profile =
            runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "profile")
                .unwrap_or("unknown");
        let observed_hard = runtime_doctor_marker_last_usize_field(
            summary,
            "profile_inflight_saturated",
            "hard_limit",
        )
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
        let observed_worker =
            runtime_doctor_marker_last_usize_field(summary, marker, "worker_count")
                .unwrap_or(input.current_worker_count);
        let observed_queue =
            runtime_doctor_marker_last_usize_field(summary, marker, "queue_capacity")
                .unwrap_or(input.current_queue_capacity);
        let observed_pending =
            runtime_doctor_marker_last_usize_field(summary, marker, "overflow_pending")
                .unwrap_or(0);
        let observed_max_pending =
            runtime_doctor_marker_last_usize_field(summary, marker, "overflow_max_pending")
                .unwrap_or(0);
        let target_worker =
            runtime_doctor_scaled_up(input.current_worker_count.max(observed_worker));
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
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<RuntimeDoctorPolicySuggestion> {
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
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<RuntimeDoctorPolicySuggestion> {
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
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<RuntimeDoctorPolicySuggestion> {
        let state_count = runtime_doctor_marker_count(summary, "state_save_queue_backpressure");
        let journal_count =
            runtime_doctor_marker_count(summary, "continuation_journal_queue_backpressure");
        if state_count + journal_count == 0 {
            return None;
        }
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
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Option<RuntimeDoctorPolicySuggestion> {
        let count = runtime_doctor_marker_count(summary, "profile_health");
        if count == 0 {
            return None;
        }
        let profile = runtime_doctor_marker_last_field(summary, "profile_health", "profile")
            .unwrap_or("unknown");
        let route = runtime_doctor_marker_last_field(summary, "profile_health", "route")
            .unwrap_or("unknown");
        let reason = runtime_doctor_marker_last_field(summary, "profile_health", "reason")
            .unwrap_or("unknown");
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

    pub(super) fn runtime_doctor_policy_suggestions(
        summary: &RuntimeDoctorSummary,
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Vec<RuntimeDoctorPolicySuggestion> {
        [
            runtime_doctor_lane_pressure_suggestion(summary, snapshot),
            runtime_doctor_active_pressure_suggestion(summary, snapshot),
            runtime_doctor_profile_inflight_suggestion(summary, snapshot),
            runtime_doctor_websocket_connect_suggestion(summary, snapshot),
            runtime_doctor_websocket_dns_suggestion(summary, snapshot),
            runtime_doctor_persistence_suggestion(summary, snapshot),
            runtime_doctor_route_health_suggestion(summary, snapshot),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::*;

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_marker_last_field<'a>(
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

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_context_dependent(summary: &RuntimeDoctorSummary) -> bool {
    summary
        .previous_response_fresh_fallback_blocked_by_request_shape
        .get("continuation_only")
        .copied()
        .unwrap_or_default()
        > 0
        || summary
            .marker_counts
            .get("previous_response_fresh_fallback_blocked")
            .copied()
            .unwrap_or_default()
            > 0
            && runtime_doctor_plan_marker_last_field(
                summary,
                "previous_response_fresh_fallback_blocked",
                "request_shape",
            ) == Some("continuation_only")
}

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_count(summary: &RuntimeDoctorSummary, marker: &'static str) -> i64 {
    summary
        .marker_counts
        .get(marker)
        .copied()
        .unwrap_or_default()
        .min(RUNTIME_DOCTOR_PLAN_MAX_COUNT as usize) as i64
}

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_optional_field(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> i64 {
    runtime_doctor_plan_marker_last_field(summary, marker, field)
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as u64) as i64)
        .unwrap_or(-1)
}

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_optional_summary_value(value: Option<usize>) -> i64 {
    value
        .map(|value| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as usize) as i64)
        .unwrap_or(-1)
}

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_latest_marker(
    summary: &RuntimeDoctorSummary,
    markers: &[&'static str],
) -> Option<&'static str> {
    markers
        .iter()
        .copied()
        .find(|marker| runtime_doctor_plan_count(summary, marker) > 0)
}

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_latest_field(
    summary: &RuntimeDoctorSummary,
    marker: Option<&'static str>,
    field: &str,
) -> i64 {
    marker
        .map(|marker| runtime_doctor_plan_optional_field(summary, marker, field))
        .unwrap_or(-1)
}

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_lane(summary: &RuntimeDoctorSummary) -> i64 {
    match runtime_doctor_plan_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
    {
        Some("responses") => RUNTIME_DOCTOR_PLAN_LANE_RESPONSES,
        Some("compact") => RUNTIME_DOCTOR_PLAN_LANE_COMPACT,
        Some("websocket") => RUNTIME_DOCTOR_PLAN_LANE_WEBSOCKET,
        Some("standard") => RUNTIME_DOCTOR_PLAN_LANE_STANDARD,
        Some(_) => RUNTIME_DOCTOR_PLAN_LANE_OTHER,
        None => RUNTIME_DOCTOR_PLAN_LANE_MISSING,
    }
}

#[cfg(feature = "mojo")]
fn runtime_doctor_plan_compact_reason(summary: &RuntimeDoctorSummary) -> i64 {
    match runtime_doctor_plan_marker_last_field(summary, "compact_final_failure", "reason") {
        Some("quota") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_QUOTA,
        Some("overload") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_OVERLOAD,
        Some("transport") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_TRANSPORT,
        Some("inflight_saturation") => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_INFLIGHT,
        _ => RUNTIME_DOCTOR_PLAN_COMPACT_REASON_UNKNOWN,
    }
}

#[cfg(feature = "mojo")]
pub(crate) fn runtime_doctor_plan_input(
    summary: &RuntimeDoctorSummary,
    operation: i64,
) -> RuntimeDoctorPlanInput {
    let connect_marker = runtime_doctor_plan_latest_marker(
        summary,
        &[
            "websocket_connect_overflow_rejected",
            "websocket_connect_overflow_reject",
            "websocket_connect_overflow_enqueue",
            "websocket_connect_overflow_dispatch",
        ],
    );
    let dns_marker = runtime_doctor_plan_latest_marker(
        summary,
        &[
            "websocket_dns_overflow_reject",
            "websocket_dns_overflow_enqueue",
            "websocket_dns_overflow_dispatch",
        ],
    );
    let counts = RuntimeDoctorPlanMarkerCounts {
        lane: runtime_doctor_plan_count(summary, "runtime_proxy_lane_limit_reached"),
        active: runtime_doctor_plan_count(summary, "runtime_proxy_active_limit_reached"),
        profile_inflight: runtime_doctor_plan_count(summary, "profile_inflight_saturated"),
        profile_health: runtime_doctor_plan_count(summary, "profile_health"),
        websocket_rejected: runtime_doctor_plan_count(
            summary,
            "websocket_connect_overflow_rejected",
        ),
        websocket_reject: runtime_doctor_plan_count(summary, "websocket_connect_overflow_reject"),
        websocket_enqueue: runtime_doctor_plan_count(summary, "websocket_connect_overflow_enqueue"),
        websocket_dispatch: runtime_doctor_plan_count(
            summary,
            "websocket_connect_overflow_dispatch",
        ),
        auth_failed: runtime_doctor_plan_count(summary, "profile_auth_recovery_failed"),
        auth_recovered: runtime_doctor_plan_count(summary, "profile_auth_recovered"),
        state_backpressure: runtime_doctor_plan_count(summary, "state_save_queue_backpressure"),
        journal_backpressure: runtime_doctor_plan_count(
            summary,
            "continuation_journal_queue_backpressure",
        ),
        sync_probe_skip: runtime_doctor_plan_count(summary, "selection_skip_sync_probe"),
        probe_backpressure: runtime_doctor_plan_count(
            summary,
            "profile_probe_refresh_backpressure",
        ),
        transport_backoff: runtime_doctor_plan_count(summary, "profile_transport_backoff"),
        profile_transport_failure: runtime_doctor_plan_count(summary, "profile_transport_failure"),
        stream_read_error: runtime_doctor_plan_count(summary, "stream_read_error"),
        upstream_connect_timeout: runtime_doctor_plan_count(summary, "upstream_connect_timeout"),
        upstream_connect_error: runtime_doctor_plan_count(summary, "upstream_connect_error"),
        upstream_connect_dns_error: runtime_doctor_plan_count(
            summary,
            "upstream_connect_dns_error",
        ),
        upstream_tls_handshake_error: runtime_doctor_plan_count(
            summary,
            "upstream_tls_handshake_error",
        ),
        quota_blocked: runtime_doctor_plan_count(summary, "quota_blocked"),
        responses_pre_send_skip: runtime_doctor_plan_count(summary, "responses_pre_send_skip"),
        websocket_pre_send_skip: runtime_doctor_plan_count(summary, "websocket_pre_send_skip"),
        precommit_budget: runtime_doctor_plan_count(summary, "precommit_budget_exhausted"),
        compact_precommit_budget: runtime_doctor_plan_count(
            summary,
            "compact_precommit_budget_exhausted",
        ),
        compact_exit_precommit_budget: runtime_doctor_plan_count(
            summary,
            "compact_exit_precommit_budget_exhausted",
        ),
        compact_candidate: runtime_doctor_plan_count(summary, "compact_candidate_exhausted"),
        compact_exit_candidate: runtime_doctor_plan_count(
            summary,
            "compact_exit_candidate_exhausted",
        ),
        dns_reject: runtime_doctor_plan_count(summary, "websocket_dns_overflow_reject"),
        dns_enqueue: runtime_doctor_plan_count(summary, "websocket_dns_overflow_enqueue"),
        dns_dispatch: runtime_doctor_plan_count(summary, "websocket_dns_overflow_dispatch"),
    };
    let observations = RuntimeDoctorPlanObservations {
        lane_active: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_lane_limit_reached",
            "active",
        ),
        lane_limit: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_lane_limit_reached",
            "limit",
        ),
        active_active: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_active_limit_reached",
            "active",
        ),
        active_limit: runtime_doctor_plan_optional_field(
            summary,
            "runtime_proxy_active_limit_reached",
            "limit",
        ),
        inflight_hard_limit: runtime_doctor_plan_optional_field(
            summary,
            "profile_inflight_saturated",
            "hard_limit",
        ),
        websocket_pending: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "overflow_pending",
        ),
        websocket_max_pending: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "overflow_max_pending",
        ),
        websocket_worker_count: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "worker_count",
        ),
        websocket_queue_capacity: runtime_doctor_plan_latest_field(
            summary,
            connect_marker,
            "queue_capacity",
        ),
        dns_pending: runtime_doctor_plan_latest_field(summary, dns_marker, "overflow_pending"),
        dns_max_pending: runtime_doctor_plan_latest_field(
            summary,
            dns_marker,
            "overflow_max_pending",
        ),
        dns_worker_count: runtime_doctor_plan_latest_field(summary, dns_marker, "worker_count"),
        dns_queue_capacity: runtime_doctor_plan_latest_field(summary, dns_marker, "queue_capacity"),
        state_backlog: runtime_doctor_plan_optional_summary_value(summary.state_save_queue_backlog),
        journal_backlog: runtime_doctor_plan_optional_summary_value(
            summary.continuation_journal_save_backlog,
        ),
        probe_backlog: runtime_doctor_plan_optional_summary_value(
            summary.profile_probe_refresh_backlog,
        ),
        sync_cold_start_jobs: runtime_doctor_plan_optional_field(
            summary,
            "selection_skip_sync_probe",
            "cold_start_jobs",
        ),
        sync_cold_start_profiles: runtime_doctor_plan_optional_field(
            summary,
            "selection_skip_sync_probe",
            "cold_start_profiles",
        ),
    };
    RuntimeDoctorPlanInput {
        operation,
        lane: runtime_doctor_plan_lane(summary),
        compact_exit_pressure: i64::from(
            runtime_doctor_plan_marker_last_field(summary, "compact_final_failure", "exit")
                == Some("pressure"),
        ),
        compact_reason: runtime_doctor_plan_compact_reason(summary),
        quota_stale_risk: i64::from(summary.quota_freshness_pressure == "stale_risk"),
        context_dependent: i64::from(runtime_doctor_plan_context_dependent(summary)),
        counts,
        observations,
        tuning: RuntimeDoctorPlanTuning::default(),
    }
}

#[cfg(feature = "mojo")]
mod mojo {
    use super::*;

    fn field<'a>(summary: &'a RuntimeDoctorSummary, marker: &str, name: &str) -> Option<&'a str> {
        summary
            .marker_last_fields
            .get(marker)
            .and_then(|fields| fields.get(name))
            .map(String::as_str)
    }

    fn tuning(snapshot: RuntimeDoctorTuningSnapshot) -> RuntimeDoctorPlanTuning {
        let bounded = |value: u64| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as u64) as i64;
        let bounded_usize =
            |value: usize| value.min(RUNTIME_DOCTOR_PLAN_MAX_SCALAR as usize) as i64;
        RuntimeDoctorPlanTuning {
            active_request_limit: bounded_usize(snapshot.active_request_limit),
            responses_active_limit: bounded_usize(snapshot.lane_limits.responses),
            compact_active_limit: bounded_usize(snapshot.lane_limits.compact),
            websocket_active_limit: bounded_usize(snapshot.lane_limits.websocket),
            standard_active_limit: bounded_usize(snapshot.lane_limits.standard),
            admission_wait_budget_ms: bounded(snapshot.admission_wait_budget_ms),
            pressure_admission_wait_budget_ms: bounded(snapshot.pressure_admission_wait_budget_ms),
            websocket_connect_worker_count: bounded_usize(snapshot.websocket_connect_worker_count),
            websocket_connect_queue_capacity: bounded_usize(
                snapshot.websocket_connect_queue_capacity,
            ),
            websocket_connect_overflow_capacity: bounded_usize(
                snapshot.websocket_connect_overflow_capacity,
            ),
            websocket_dns_worker_count: bounded_usize(snapshot.websocket_dns_worker_count),
            websocket_dns_queue_capacity: bounded_usize(snapshot.websocket_dns_queue_capacity),
            websocket_dns_overflow_capacity: bounded_usize(
                snapshot.websocket_dns_overflow_capacity,
            ),
            profile_inflight_soft_limit: bounded_usize(snapshot.profile_inflight_soft_limit),
            profile_inflight_hard_limit: bounded_usize(snapshot.profile_inflight_hard_limit),
        }
    }

    fn marker_name(marker: i64, dns: bool) -> &'static str {
        match (marker, dns) {
            (RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECTED, false) => {
                "websocket_connect_overflow_rejected"
            }
            (RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECT, false) => {
                "websocket_connect_overflow_reject"
            }
            (RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_ENQUEUE, false) => {
                "websocket_connect_overflow_enqueue"
            }
            (RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_DISPATCH, false) => {
                "websocket_connect_overflow_dispatch"
            }
            (RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_REJECT, true) => "websocket_dns_overflow_reject",
            (RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_ENQUEUE, true) => {
                "websocket_dns_overflow_enqueue"
            }
            (RUNTIME_DOCTOR_PLAN_MARKER_WEBSOCKET_DISPATCH, true) => {
                "websocket_dns_overflow_dispatch"
            }
            _ => "-",
        }
    }

    fn setting_key(key: i64) -> Option<&'static str> {
        Some(match key {
            RUNTIME_DOCTOR_PLAN_SETTING_RESPONSES_ACTIVE => "responses_active_limit",
            RUNTIME_DOCTOR_PLAN_SETTING_COMPACT_ACTIVE => "compact_active_limit",
            RUNTIME_DOCTOR_PLAN_SETTING_WEBSOCKET_ACTIVE => "websocket_active_limit",
            RUNTIME_DOCTOR_PLAN_SETTING_STANDARD_ACTIVE => "standard_active_limit",
            RUNTIME_DOCTOR_PLAN_SETTING_ACTIVE_REQUEST => "active_request_limit",
            RUNTIME_DOCTOR_PLAN_SETTING_PROFILE_SOFT => "profile_inflight_soft_limit",
            RUNTIME_DOCTOR_PLAN_SETTING_PROFILE_HARD => "profile_inflight_hard_limit",
            RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_WORKERS => "websocket_connect_worker_count",
            RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_QUEUE => "websocket_connect_queue_capacity",
            RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_OVERFLOW => "websocket_connect_overflow_capacity",
            RUNTIME_DOCTOR_PLAN_SETTING_DNS_WORKERS => "websocket_dns_worker_count",
            RUNTIME_DOCTOR_PLAN_SETTING_DNS_QUEUE => "websocket_dns_queue_capacity",
            RUNTIME_DOCTOR_PLAN_SETTING_DNS_OVERFLOW => "websocket_dns_overflow_capacity",
            RUNTIME_DOCTOR_PLAN_SETTING_PRESSURE_WAIT => "pressure_admission_wait_budget_ms",
            _ => return None,
        })
    }

    fn title(id: i64) -> Option<&'static str> {
        Some(match id {
            RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE => "Lane pressure",
            RUNTIME_DOCTOR_PLAN_SUGGESTION_ACTIVE => "Active request pressure",
            RUNTIME_DOCTOR_PLAN_SUGGESTION_PROFILE_INFLIGHT => "Profile in-flight saturation",
            RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_CONNECT => "Websocket connect overflow",
            RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_DNS => "Websocket DNS overflow",
            RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE => "Persistence backpressure",
            RUNTIME_DOCTOR_PLAN_SUGGESTION_ROUTE_HEALTH => "Route-scoped profile health",
            _ => return None,
        })
    }

    fn suggestion_markers(id: i64) -> Option<Vec<String>> {
        Some(
            match id {
                RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE => {
                    vec!["runtime_proxy_lane_limit_reached"]
                }
                RUNTIME_DOCTOR_PLAN_SUGGESTION_ACTIVE => {
                    vec!["runtime_proxy_active_limit_reached"]
                }
                RUNTIME_DOCTOR_PLAN_SUGGESTION_PROFILE_INFLIGHT => {
                    vec!["profile_inflight_saturated"]
                }
                RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_CONNECT => vec![
                    "websocket_connect_overflow_rejected",
                    "websocket_connect_overflow_reject",
                    "websocket_connect_overflow_enqueue",
                    "websocket_connect_overflow_dispatch",
                ],
                RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_DNS => vec![
                    "websocket_dns_overflow_reject",
                    "websocket_dns_overflow_enqueue",
                    "websocket_dns_overflow_dispatch",
                ],
                RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE => vec![
                    "state_save_queue_backpressure",
                    "continuation_journal_queue_backpressure",
                ],
                RUNTIME_DOCTOR_PLAN_SUGGESTION_ROUTE_HEALTH => vec!["profile_health"],
                _ => return None,
            }
            .into_iter()
            .map(str::to_string)
            .collect(),
        )
    }

    fn rationale(id: i64, key: i64, lane: &str) -> String {
        match (id, key) {
            (RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE, RUNTIME_DOCTOR_PLAN_SETTING_ACTIVE_REQUEST) => {
                "keep the global admission cap above the suggested lane cap".to_string()
            }
            (RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE, _) => {
                format!("raise the {lane} lane cap after repeated lane-limit markers")
            }
            (RUNTIME_DOCTOR_PLAN_SUGGESTION_ACTIVE, _) => {
                "allow more pre-commit requests through local admission".to_string()
            }
            (
                RUNTIME_DOCTOR_PLAN_SUGGESTION_PROFILE_INFLIGHT,
                RUNTIME_DOCTOR_PLAN_SETTING_PROFILE_SOFT,
            ) => "delay soft load penalty until a profile has more concurrent work".to_string(),
            (RUNTIME_DOCTOR_PLAN_SUGGESTION_PROFILE_INFLIGHT, _) => {
                "raise the fresh-selection hard cap for a busy profile".to_string()
            }
            (
                RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_CONNECT
                | RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_DNS,
                _,
            ) => match key {
                RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_WORKERS
                | RUNTIME_DOCTOR_PLAN_SETTING_DNS_WORKERS => {
                    "increase bounded executor parallelism".to_string()
                }
                RUNTIME_DOCTOR_PLAN_SETTING_CONNECT_QUEUE
                | RUNTIME_DOCTOR_PLAN_SETTING_DNS_QUEUE => {
                    "increase bounded executor queue capacity".to_string()
                }
                _ => "increase burst overflow buffering after the bounded queue fills".to_string(),
            },
            (
                RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE,
                RUNTIME_DOCTOR_PLAN_SETTING_COMPACT_ACTIVE,
            ) => "reduce fresh compact churn that creates continuation state writes".to_string(),
            (
                RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE,
                RUNTIME_DOCTOR_PLAN_SETTING_STANDARD_ACTIVE,
            ) => "reduce side-lane churn while persistence is behind".to_string(),
            (RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE, _) => {
                "let pressure-mode admission wait briefly for queues to drain".to_string()
            }
            (
                RUNTIME_DOCTOR_PLAN_SUGGESTION_ROUTE_HEALTH,
                RUNTIME_DOCTOR_PLAN_SETTING_PROFILE_SOFT,
            ) => {
                "spread fresh work away from accounts accumulating route-specific health penalties"
                    .to_string()
            }
            _ => "cap fresh work per profile more tightly while route health recovers".to_string(),
        }
    }

    fn from_plan(
        summary: &RuntimeDoctorSummary,
        plan: &RuntimeDoctorPlan,
        index: usize,
    ) -> RuntimeDoctorPolicySuggestion {
        let id = plan.suggestion_ids[index];
        let severity = match plan.suggestion_severities[index] {
            RUNTIME_DOCTOR_PLAN_SEVERITY_LOW => "low",
            _ => "medium",
        };
        let lane =
            field(summary, "runtime_proxy_lane_limit_reached", "lane").unwrap_or("responses");
        let count = plan.suggestion_counts[index];
        let (reason, markers) = match id {
            RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE => (
                format!(
                    "{count} lane-limit marker(s) on lane={lane}; apply only if host/network headroom exists"
                ),
                suggestion_markers(id).expect("validated Mojo suggestion id"),
            ),
            RUNTIME_DOCTOR_PLAN_SUGGESTION_ACTIVE => (
                format!(
                    "{count} global active-limit marker(s); raise only if local CPU/network is not saturated"
                ),
                suggestion_markers(id).expect("validated Mojo suggestion id"),
            ),
            RUNTIME_DOCTOR_PLAN_SUGGESTION_PROFILE_INFLIGHT => (
                format!(
                    "{count} per-profile in-flight saturation marker(s), latest profile={}; raise only if account fan-out is intentional",
                    field(summary, "profile_inflight_saturated", "profile").unwrap_or("unknown")
                ),
                suggestion_markers(id).expect("validated Mojo suggestion id"),
            ),
            RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_CONNECT => (
                format!(
                    "{count} websocket executor overflow marker(s), latest={}; raise only for bursty session starts",
                    marker_name(plan.suggestion_markers[index], false)
                ),
                suggestion_markers(id).expect("validated Mojo suggestion id"),
            ),
            RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_DNS => (
                format!(
                    "{count} websocket executor overflow marker(s), latest={}; raise only for bursty session starts",
                    marker_name(plan.suggestion_markers[index], true)
                ),
                suggestion_markers(id).expect("validated Mojo suggestion id"),
            ),
            RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE => (
                format!(
                    "state-save backpressure={}, continuation-journal backpressure={}; throttle churn while queues drain",
                    runtime_doctor_plan_count(summary, "state_save_queue_backpressure"),
                    runtime_doctor_plan_count(summary, "continuation_journal_queue_backpressure")
                ),
                suggestion_markers(id).expect("validated Mojo suggestion id"),
            ),
            RUNTIME_DOCTOR_PLAN_SUGGESTION_ROUTE_HEALTH => (
                format!(
                    "{count} route-scoped health marker(s), latest={}/{} reason={}; lower per-profile fresh pressure if this repeats",
                    field(summary, "profile_health", "profile").unwrap_or("unknown"),
                    field(summary, "profile_health", "route").unwrap_or("unknown"),
                    field(summary, "profile_health", "reason").unwrap_or("unknown")
                ),
                suggestion_markers(id).expect("validated Mojo suggestion id"),
            ),
            _ => unreachable!("Mojo plan validation should reject unknown suggestion ids"),
        };
        let setting_count = plan.suggestion_setting_counts[index] as usize;
        let settings = (0..setting_count)
            .map(|setting| {
                let flat = index * RUNTIME_DOCTOR_PLAN_MAX_SETTINGS + setting;
                let key = plan.setting_keys[flat];
                RuntimeDoctorPolicySettingSuggestion {
                    section: "runtime_proxy".to_string(),
                    key: setting_key(key)
                        .expect("validated Mojo setting key")
                        .to_string(),
                    current_value: plan.setting_current_values[flat] as u64,
                    suggested_value: plan.setting_suggested_values[flat] as u64,
                    rationale: rationale(id, key, lane),
                }
            })
            .collect::<Vec<_>>();
        let mut snippet = vec!["[runtime_proxy]".to_string()];
        for setting in &settings {
            snippet.push(format!("{} = {}", setting.key, setting.suggested_value));
        }
        RuntimeDoctorPolicySuggestion {
            id: match id {
                RUNTIME_DOCTOR_PLAN_SUGGESTION_LANE => "lane_pressure",
                RUNTIME_DOCTOR_PLAN_SUGGESTION_ACTIVE => "active_request_pressure",
                RUNTIME_DOCTOR_PLAN_SUGGESTION_PROFILE_INFLIGHT => "profile_inflight_saturation",
                RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_CONNECT => "websocket_connect_overflow",
                RUNTIME_DOCTOR_PLAN_SUGGESTION_WEBSOCKET_DNS => "websocket_dns_overflow",
                RUNTIME_DOCTOR_PLAN_SUGGESTION_PERSISTENCE => "persistence_backpressure",
                RUNTIME_DOCTOR_PLAN_SUGGESTION_ROUTE_HEALTH => "route_scoped_profile_health",
                _ => unreachable!("validated Mojo suggestion id"),
            }
            .to_string(),
            title: title(id).expect("validated Mojo suggestion id").to_string(),
            severity: severity.to_string(),
            reason,
            markers,
            settings,
            snippet: snippet.join("\n"),
        }
    }

    pub(super) fn policy_suggestions(
        summary: &RuntimeDoctorSummary,
        snapshot: RuntimeDoctorTuningSnapshot,
    ) -> Vec<RuntimeDoctorPolicySuggestion> {
        let mut input =
            runtime_doctor_plan_input(summary, RUNTIME_DOCTOR_PLAN_OP_POLICY_SUGGESTIONS);
        input.tuning = tuning(snapshot);
        let plan = runtime_doctor_plan(input)
            .expect("Mojo runtime-doctor policy plan returned invalid output");
        (0..plan.suggestion_count as usize)
            .map(|index| from_plan(summary, &plan, index))
            .collect()
    }
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_policy_suggestions(
    summary: &RuntimeDoctorSummary,
    snapshot: RuntimeDoctorTuningSnapshot,
) -> Vec<RuntimeDoctorPolicySuggestion> {
    mojo::policy_suggestions(summary, snapshot)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_policy_suggestions(
    summary: &RuntimeDoctorSummary,
    snapshot: RuntimeDoctorTuningSnapshot,
) -> Vec<RuntimeDoctorPolicySuggestion> {
    rust_fallback::runtime_doctor_policy_suggestions(summary, snapshot)
}

pub fn runtime_doctor_policy_suggestion_lines(
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
#[path = "../tests/src/suggestions.rs"]
mod tests;
