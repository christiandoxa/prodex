//! Smart-context telemetry label and category formatting helpers.

use super::*;

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_pressure_band_label(
    value: runtime_proxy_crate::SmartContextPressureBand,
) -> &'static str {
    match value {
        runtime_proxy_crate::SmartContextPressureBand::Unknown => "unknown",
        runtime_proxy_crate::SmartContextPressureBand::Low => "low",
        runtime_proxy_crate::SmartContextPressureBand::Moderate => "moderate",
        runtime_proxy_crate::SmartContextPressureBand::High => "high",
        runtime_proxy_crate::SmartContextPressureBand::Critical => "critical",
        runtime_proxy_crate::SmartContextPressureBand::Exhausted => "exhausted",
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_estimator_confidence_label(
    value: runtime_proxy_crate::SmartContextEstimatorConfidence,
) -> &'static str {
    match value {
        runtime_proxy_crate::SmartContextEstimatorConfidence::High => "high",
        runtime_proxy_crate::SmartContextEstimatorConfidence::Medium => "medium",
        runtime_proxy_crate::SmartContextEstimatorConfidence::Low => "low",
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rollout_mode_label(
    mode: runtime_proxy_crate::SmartContextRolloutMode,
) -> &'static str {
    match mode {
        runtime_proxy_crate::SmartContextRolloutMode::Apply => "apply",
        runtime_proxy_crate::SmartContextRolloutMode::Shadow => "shadow",
        runtime_proxy_crate::SmartContextRolloutMode::Disabled => "disabled",
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_budget_mode_label(
    mode: runtime_proxy_crate::SmartContextBudgetMode,
) -> &'static str {
    match mode {
        runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough => "exact_pass_through",
        runtime_proxy_crate::SmartContextBudgetMode::LargeLossless => "large_lossless",
        runtime_proxy_crate::SmartContextBudgetMode::ArtifactCondensed => "artifact_condensed",
        runtime_proxy_crate::SmartContextBudgetMode::MinimalRefsOnly => "minimal_refs_only",
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_budget_policy_reason_labels(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_transformed_segment_categories(
    stats: &RuntimeSmartContextTransformStats,
) -> String {
    let mut categories = Vec::new();
    if stats.tool_outputs_condensed > 0 {
        categories.push("tool_output");
    }
    if stats.tool_call_args_condensed > 0 {
        categories.push("tool_argument");
    }
    if stats.duplicate_texts > 0 || stats.cross_turn_duplicate_texts > 0 {
        categories.push("duplicate_context");
    }
    if stats.repeat_tool_output_refs > 0 {
        categories.push("repeat_tool_output");
    }
    if stats.blob_outputs_condensed > 0 {
        categories.push("blob_output");
    }
    if stats.rehydrated_refs > 0 {
        categories.push("rehydration");
    }
    if stats.static_context_deltas > 0 {
        categories.push("static_context");
    }
    if stats.repo_state_facts > 0 {
        categories.push("repo_state");
    }
    if categories.is_empty() {
        "-".to_string()
    } else {
        categories.join(",")
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_reason_labels(
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
        })
        .collect::<Vec<_>>()
        .join(",")
}
