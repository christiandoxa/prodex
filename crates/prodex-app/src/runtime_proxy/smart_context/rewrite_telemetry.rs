use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRewriteTelemetryRecord {
    pub(super) body_bytes_before: usize,
    pub(super) body_bytes_after: usize,
    pub(super) estimated_tokens_before: u64,
    pub(super) estimated_tokens_after: u64,
    pub(super) rewrite_kind: String,
    pub(super) status: String,
    pub(super) fallback_reason: Option<String>,
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
    })
}

fn runtime_smart_context_rewrite_telemetry_record_safe_saved(
    record: &RuntimeSmartContextRewriteTelemetryRecord,
) -> bool {
    record.fallback_reason.is_none()
        && matches!(record.rewrite_kind.as_str(), "rewritten" | "minified")
        && matches!(
            record.status.as_str(),
            "ok_saved" | "ok_surgical_rehydrate" | "ok_minified"
        )
        && record.estimated_tokens_after < record.estimated_tokens_before
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_smart_context_log(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    tier: &str,
    decision: &str,
    reasons: &str,
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: RuntimeSmartContextTransformStats,
    budget: &RuntimeSmartContextBudget,
    self_check: &'static str,
) {
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
                    "fallback_reason",
                    runtime_smart_context_telemetry_fallback_reason(decision, self_check)
                        .unwrap_or("-"),
                ),
                runtime_proxy_log_field("available_tokens", budget.available_tokens.to_string()),
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
                    "static_context_deltas",
                    stats.static_context_deltas.to_string(),
                ),
                runtime_proxy_log_field("repo_state_facts", stats.repo_state_facts.to_string()),
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

fn runtime_smart_context_budget_mode_label(
    mode: runtime_proxy_crate::SmartContextBudgetMode,
) -> &'static str {
    match mode {
        runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough => "exact_pass_through",
        runtime_proxy_crate::SmartContextBudgetMode::LargeLossless => "large_lossless",
        runtime_proxy_crate::SmartContextBudgetMode::ArtifactCondensed => "artifact_condensed",
        runtime_proxy_crate::SmartContextBudgetMode::MinimalRefsOnly => "minimal_refs_only",
    }
}

fn runtime_smart_context_budget_policy_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextBudgetPolicyReason],
) -> String {
    if reasons.is_empty() {
        return "-".to_string();
    }
    reasons
        .iter()
        .map(|reason| match reason {
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ExactnessRequired => {
                "exactness_required"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged => {
                "static_context_changed"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::MissingRehydrateRefs => {
                "missing_rehydrate_refs"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnknownTokenWindow => {
                "unknown_token_window"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnsafeAccounting => {
                "unsafe_accounting"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe => {
                "recent_rewrite_savings_safe"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::PlentyOfBudget => {
                "plenty_of_budget"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ModerateBudget => {
                "moderate_budget"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::TightBudget => "tight_budget",
            runtime_proxy_crate::SmartContextBudgetPolicyReason::CriticalBudget => {
                "critical_budget"
            }
        })
        .collect::<Vec<_>>()
        .join(",")
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

pub(super) fn runtime_smart_context_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextExactnessReason],
) -> String {
    if reasons.is_empty() {
        return "-".to_string();
    }
    reasons
        .iter()
        .map(|reason| match reason {
            runtime_proxy_crate::SmartContextExactnessReason::ExplicitExactMode => "exact_mode",
            runtime_proxy_crate::SmartContextExactnessReason::PreviousResponseAffinity => {
                "previous_response"
            }
            runtime_proxy_crate::SmartContextExactnessReason::TurnStateAffinity => "turn_state",
            runtime_proxy_crate::SmartContextExactnessReason::SessionAffinity => "session",
            runtime_proxy_crate::SmartContextExactnessReason::ToolOutputWithoutArtifact => {
                "tool_output_without_artifact"
            }
            runtime_proxy_crate::SmartContextExactnessReason::RehydrateRequired => "rehydrate",
        })
        .collect::<Vec<_>>()
        .join(",")
}
