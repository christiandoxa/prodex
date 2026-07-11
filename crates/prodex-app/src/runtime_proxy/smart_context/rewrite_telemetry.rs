use super::*;

#[path = "rewrite_telemetry/labels.rs"]
mod labels;
pub(super) use labels::runtime_smart_context_reason_labels;
use labels::{
    runtime_smart_context_budget_mode_label, runtime_smart_context_budget_policy_reason_labels,
    runtime_smart_context_estimator_confidence_label, runtime_smart_context_pressure_band_label,
    runtime_smart_context_rollout_mode_label, runtime_smart_context_transformed_segment_categories,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRewriteTelemetryRecord {
    pub(super) body_bytes_before: usize,
    pub(super) body_bytes_after: usize,
    pub(super) estimated_tokens_before: u64,
    pub(super) estimated_tokens_after: u64,
    pub(super) rewrite_kind: String,
    pub(super) status: String,
    pub(super) fallback_reason: Option<String>,
    pub(super) upstream_context_errors: u16,
    pub(super) previous_response_not_found: bool,
    pub(super) invalid_tool_call_continuation: bool,
    pub(super) missing_artifact_requests: u16,
    pub(super) repeated_tool_call_count: u16,
    pub(super) model_reread_requests: u16,
    pub(super) corrective_user_messages: u16,
    pub(super) test_or_build_failed_after_rewrite: bool,
    pub(super) task_completed: Option<bool>,
    pub(super) additional_turns_before_task_completion: Option<u16>,
    pub(super) final_total_input_tokens: Option<u64>,
    pub(super) pressure_basis_points: Option<u32>,
    pub(super) pressure_band: String,
    pub(super) estimator_confidence: String,
    pub(super) effective_usable_context_tokens: Option<u64>,
    pub(super) absolute_safety_floor_tokens: u64,
}

pub(super) fn runtime_smart_context_rewrite_telemetry_samples(
    history: &[RuntimeSmartContextRewriteTelemetryRecord],
) -> Vec<runtime_proxy_crate::SmartContextRewriteTelemetrySample> {
    history
        .iter()
        .filter_map(runtime_smart_context_rewrite_telemetry_sample)
        .collect()
}

fn runtime_smart_context_rewrite_telemetry_sample(
    record: &RuntimeSmartContextRewriteTelemetryRecord,
) -> Option<runtime_proxy_crate::SmartContextRewriteTelemetrySample> {
    let fallback = record.fallback_reason.is_some();
    let potentially_rewritten = matches!(record.rewrite_kind.as_str(), "rewritten" | "minified")
        || matches!(
            record.status.as_str(),
            "ok_saved" | "ok_surgical_rehydrate" | "ok_minified"
        );
    if !fallback && !potentially_rewritten {
        return None;
    }
    Some(runtime_proxy_crate::SmartContextRewriteTelemetrySample {
        body_bytes_before: record.body_bytes_before,
        body_bytes_after: record.body_bytes_after,
        estimated_tokens_before: record.estimated_tokens_before,
        estimated_tokens_after: record.estimated_tokens_after,
        safe: runtime_smart_context_rewrite_telemetry_record_safe_saved(record),
        fallback,
        upstream_context_errors: record.upstream_context_errors,
        previous_response_not_found: record.previous_response_not_found,
        invalid_tool_call_continuation: record.invalid_tool_call_continuation,
        missing_artifact_requests: record.missing_artifact_requests,
        repeated_tool_call_count: record.repeated_tool_call_count,
        model_reread_requests: record.model_reread_requests,
        corrective_user_messages: record.corrective_user_messages,
        test_or_build_failed_after_rewrite: record.test_or_build_failed_after_rewrite,
        task_completed: record.task_completed,
        additional_turns_before_task_completion: record.additional_turns_before_task_completion,
        final_total_input_tokens: record.final_total_input_tokens,
    })
}

fn runtime_smart_context_rewrite_telemetry_record_safe_saved(
    record: &RuntimeSmartContextRewriteTelemetryRecord,
) -> bool {
    record.fallback_reason.is_none()
        && !runtime_smart_context_rewrite_telemetry_record_quality_risk(record)
        && matches!(record.rewrite_kind.as_str(), "rewritten" | "minified")
        && matches!(
            record.status.as_str(),
            "ok_saved" | "ok_surgical_rehydrate" | "ok_minified"
        )
        && record.estimated_tokens_after < record.estimated_tokens_before
}

fn runtime_smart_context_rewrite_telemetry_record_quality_risk(
    record: &RuntimeSmartContextRewriteTelemetryRecord,
) -> bool {
    record.upstream_context_errors > 0
        || record.previous_response_not_found
        || record.invalid_tool_call_continuation
        || record.missing_artifact_requests > 0
        || record.repeated_tool_call_count > 0
        || record.model_reread_requests > 0
        || record.corrective_user_messages > 0
        || record.test_or_build_failed_after_rewrite
        || record.task_completed == Some(false)
}

pub(super) struct RuntimeSmartContextLogInput<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) route_kind: RuntimeRouteKind,
    pub(super) transport: RuntimeSmartContextTransport,
    pub(super) tier: &'a str,
    pub(super) decision: &'a str,
    pub(super) reasons: &'a str,
    pub(super) body_bytes_before: usize,
    pub(super) body_bytes_after: usize,
    pub(super) stats: RuntimeSmartContextTransformStats,
    pub(super) budget: &'a RuntimeSmartContextBudget,
    pub(super) self_check: &'static str,
}

pub(super) fn runtime_smart_context_log(input: RuntimeSmartContextLogInput<'_>) {
    let RuntimeSmartContextLogInput {
        request_id,
        shared,
        route_kind,
        transport,
        tier,
        decision,
        reasons,
        body_bytes_before,
        body_bytes_after,
        stats,
        budget,
        self_check,
    } = input;
    let rollout = runtime_proxy_crate::smart_context_rollout_decision(
        runtime_proxy_crate::SmartContextRolloutDecisionInput {
            enabled: true,
            explicit_exact_mode: false,
            shadow_mode: shared.runtime_config.smart_context_shadow,
            canary_percent: shared.runtime_config.smart_context_canary_percent,
            stable_key: format!(
                "{}:{}:{}:{}",
                shared.log_path.display(),
                runtime_route_kind_label(route_kind),
                transport.label(),
                request_id
            ),
        },
    );
    runtime_smart_context_record_rewrite_telemetry(
        shared,
        RuntimeSmartContextRewriteTelemetryRecord {
            body_bytes_before,
            body_bytes_after,
            estimated_tokens_before:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                    body_bytes_before,
                ),
            estimated_tokens_after:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_after),
            rewrite_kind: decision.to_string(),
            status: self_check.to_string(),
            fallback_reason: runtime_smart_context_telemetry_fallback_reason(decision, self_check)
                .map(str::to_string),
            upstream_context_errors: 0,
            previous_response_not_found: false,
            invalid_tool_call_continuation: false,
            missing_artifact_requests: 0,
            repeated_tool_call_count: 0,
            model_reread_requests: 0,
            corrective_user_messages: 0,
            test_or_build_failed_after_rewrite: false,
            task_completed: None,
            additional_turns_before_task_completion: None,
            final_total_input_tokens: None,
            pressure_basis_points: budget.pressure.pressure_basis_points,
            pressure_band: runtime_smart_context_pressure_band_label(budget.pressure.pressure_band)
                .to_string(),
            estimator_confidence: runtime_smart_context_estimator_confidence_label(
                budget.pressure.estimator_confidence,
            )
            .to_string(),
            effective_usable_context_tokens: budget.pressure.effective_usable_context_tokens,
            absolute_safety_floor_tokens: budget.pressure.absolute_safety_floor_tokens,
        },
    );
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_autopilot",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport.label()),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("tier", tier),
                runtime_proxy_log_field("decision", decision),
                runtime_proxy_log_field("reasons", reasons),
                runtime_proxy_log_field("token_usage_source", budget.token_usage_source),
                runtime_proxy_log_field(
                    "model_context_window_tokens",
                    budget.model_context_window_tokens.to_string(),
                ),
                runtime_proxy_log_field(
                    "model_context_window_source",
                    budget.model_context_window_source,
                ),
                runtime_proxy_log_field(
                    "observed_context_tokens",
                    budget
                        .observed_context_tokens
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
                runtime_proxy_log_field("body_bytes_before", body_bytes_before.to_string()),
                runtime_proxy_log_field("body_bytes_after", body_bytes_after.to_string()),
                runtime_proxy_log_field(
                    "estimated_tokens_before",
                    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                        body_bytes_before,
                    )
                    .to_string(),
                ),
                runtime_proxy_log_field(
                    "estimated_tokens_after",
                    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                        body_bytes_after,
                    )
                    .to_string(),
                ),
                runtime_proxy_log_field(
                    "body_bytes_saved",
                    body_bytes_before
                        .saturating_sub(body_bytes_after)
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "rewrite_ratio_percent",
                    runtime_smart_context_rewrite_ratio_percent(
                        body_bytes_before,
                        body_bytes_after,
                    )
                    .to_string(),
                ),
                runtime_proxy_log_field("self_check", self_check),
                runtime_proxy_log_field("rewrite_kind", decision),
                runtime_proxy_log_field("rewrite_status", self_check),
                runtime_proxy_log_field(
                    "rollout_mode",
                    runtime_smart_context_rollout_mode_label(rollout.mode),
                ),
                runtime_proxy_log_field("rollout_reason", rollout.reason),
                runtime_proxy_log_field("rollout_canary_bucket", rollout.canary_bucket.to_string()),
                runtime_proxy_log_field(
                    "rollout_canary_percent",
                    rollout.canary_percent.to_string(),
                ),
                runtime_proxy_log_field(
                    "fallback_reason",
                    runtime_smart_context_telemetry_fallback_reason(decision, self_check)
                        .unwrap_or("-"),
                ),
                runtime_proxy_log_field("available_tokens", budget.available_tokens.to_string()),
                runtime_proxy_log_field(
                    "pressure_basis_points",
                    budget
                        .pressure
                        .pressure_basis_points
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
                runtime_proxy_log_field(
                    "pressure_band",
                    runtime_smart_context_pressure_band_label(budget.pressure.pressure_band),
                ),
                runtime_proxy_log_field(
                    "estimator_confidence",
                    runtime_smart_context_estimator_confidence_label(
                        budget.pressure.estimator_confidence,
                    ),
                ),
                runtime_proxy_log_field(
                    "effective_usable_context_tokens",
                    budget
                        .pressure
                        .effective_usable_context_tokens
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
                runtime_proxy_log_field(
                    "absolute_safety_floor_tokens",
                    budget.pressure.absolute_safety_floor_tokens.to_string(),
                ),
                runtime_proxy_log_field(
                    "budget_mode",
                    runtime_smart_context_budget_mode_label(budget.policy.mode),
                ),
                runtime_proxy_log_field(
                    "max_inline_tool_output_bytes",
                    budget.policy.max_inline_tool_output_bytes.to_string(),
                ),
                runtime_proxy_log_field(
                    "max_rehydrate_tokens",
                    budget.policy.max_rehydrate_tokens.to_string(),
                ),
                runtime_proxy_log_field(
                    "policy_reasons",
                    runtime_smart_context_budget_policy_reason_labels(&budget.policy.reasons),
                ),
                runtime_proxy_log_field("artifacts_stored", stats.artifacts_stored.to_string()),
                runtime_proxy_log_field(
                    "tool_outputs_condensed",
                    stats.tool_outputs_condensed.to_string(),
                ),
                runtime_proxy_log_field("duplicate_texts", stats.duplicate_texts.to_string()),
                runtime_proxy_log_field(
                    "cross_turn_duplicate_texts",
                    stats.cross_turn_duplicate_texts.to_string(),
                ),
                runtime_proxy_log_field(
                    "repeat_tool_output_refs",
                    stats.repeat_tool_output_refs.to_string(),
                ),
                runtime_proxy_log_field(
                    "blob_outputs_condensed",
                    stats.blob_outputs_condensed.to_string(),
                ),
                runtime_proxy_log_field("rehydrated_refs", stats.rehydrated_refs.to_string()),
                runtime_proxy_log_field(
                    "rehydration_token_cost",
                    stats.rehydration_token_cost.to_string(),
                ),
                runtime_proxy_log_field(
                    "static_context_deltas",
                    stats.static_context_deltas.to_string(),
                ),
                runtime_proxy_log_field("repo_state_facts", stats.repo_state_facts.to_string()),
                runtime_proxy_log_field("candidate_count", stats.candidate_count.to_string()),
                runtime_proxy_log_field(
                    "selected_candidate_count",
                    stats.selected_candidate_count.to_string(),
                ),
                runtime_proxy_log_field(
                    "rejected_candidate_count",
                    stats.rejected_candidate_count.to_string(),
                ),
                runtime_proxy_log_field(
                    "selected_candidate_utility_points",
                    stats.selected_candidate_utility_points.to_string(),
                ),
                runtime_proxy_log_field(
                    "transformed_segment_categories",
                    runtime_smart_context_transformed_segment_categories(&stats),
                ),
                runtime_proxy_log_field(
                    "segment_rollback_count",
                    stats.segment_rollback_count.to_string(),
                ),
                runtime_proxy_log_field(
                    "full_request_fallback_count",
                    stats.full_request_fallback_count.to_string(),
                ),
                runtime_proxy_log_field(
                    "artifact_hash_failures",
                    stats.artifact_hash_failures.to_string(),
                ),
                runtime_proxy_log_field("task_quality_upstream_context_errors", "0"),
                runtime_proxy_log_field("task_quality_previous_response_not_found", "false"),
                runtime_proxy_log_field("task_quality_invalid_tool_call_continuation", "false"),
                runtime_proxy_log_field("task_quality_missing_artifact_requests", "0"),
                runtime_proxy_log_field("task_quality_repeated_tool_call_count", "0"),
                runtime_proxy_log_field("task_quality_model_reread_requests", "0"),
                runtime_proxy_log_field("task_quality_corrective_user_messages", "0"),
                runtime_proxy_log_field("task_quality_test_or_build_failed_after_rewrite", "false"),
                runtime_proxy_log_field("task_quality_task_completed", "unknown"),
                runtime_proxy_log_field("task_quality_additional_turns_before_completion", "-"),
                runtime_proxy_log_field("task_quality_final_total_input_tokens", "-"),
            ],
        ),
    );
}

fn runtime_smart_context_record_rewrite_telemetry(
    shared: &RuntimeRotationProxyShared,
    record: RuntimeSmartContextRewriteTelemetryRecord,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(mut states) = states.lock() else {
        return;
    };
    let Some(state) = states.get_mut(&shared.log_path) else {
        return;
    };
    if !state.enabled {
        return;
    }
    state.rewrite_telemetry_history.push(record);
    if state.rewrite_telemetry_history.len() > SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT {
        let overflow = state
            .rewrite_telemetry_history
            .len()
            .saturating_sub(SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT);
        state.rewrite_telemetry_history.drain(0..overflow);
    }
}

fn runtime_smart_context_telemetry_fallback_reason<'a>(
    decision: &str,
    self_check: &'a str,
) -> Option<&'a str> {
    if decision == "self_check_passthrough" {
        Some(self_check)
    } else {
        None
    }
}

pub(super) fn runtime_smart_context_rewrite_self_check(
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: &RuntimeSmartContextTransformStats,
) -> &'static str {
    if stats.rehydrated_refs > 0
        && stats.tool_outputs_condensed == 0
        && stats.duplicate_texts == 0
        && stats.static_context_deltas == 0
    {
        "ok_rehydrate_exact"
    } else if body_bytes_after < body_bytes_before {
        "ok_saved"
    } else if body_bytes_after == body_bytes_before {
        "zero_savings"
    } else {
        "growth"
    }
}

pub(super) fn runtime_smart_context_saved_tokens(
    body_bytes_before: usize,
    body_bytes_after: usize,
) -> u64 {
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_before)
        .saturating_sub(
            runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_after),
        )
}

fn runtime_smart_context_rewrite_ratio_percent(
    body_bytes_before: usize,
    body_bytes_after: usize,
) -> usize {
    if body_bytes_before == 0 {
        return 100;
    }
    body_bytes_after.saturating_mul(100) / body_bytes_before
}
