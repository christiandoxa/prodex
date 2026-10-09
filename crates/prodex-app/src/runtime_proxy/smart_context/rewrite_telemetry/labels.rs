//! Smart-context telemetry label and category formatting helpers.

use super::*;

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_pressure_band_label(
    value: runtime_proxy_crate::SmartContextPressureBand,
) -> String {
    runtime_smart_context_telemetry_label(0, value as u64)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_estimator_confidence_label(
    value: runtime_proxy_crate::SmartContextEstimatorConfidence,
) -> String {
    runtime_smart_context_telemetry_label(1, value as u64)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rollout_mode_label(
    mode: runtime_proxy_crate::SmartContextRolloutMode,
) -> String {
    runtime_smart_context_telemetry_label(2, mode as u64)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_budget_mode_label(
    mode: runtime_proxy_crate::SmartContextBudgetMode,
) -> String {
    runtime_smart_context_telemetry_label(3, mode as u64)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_budget_policy_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextBudgetPolicyReason],
) -> String {
    let bits = reasons.iter().fold(0_u64, |bits, reason| {
        bits | match reason {
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ExactnessRequired => 1,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged => 2,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::MissingRehydrateRefs => 4,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnknownTokenWindow => 8,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnsafeAccounting => 16,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe => 32,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::PlentyOfBudget => 64,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ModerateBudget => 128,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::TightBudget => 256,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::CriticalBudget => 512,
        }
    });
    runtime_smart_context_telemetry_label(6, bits)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_transformed_segment_categories(
    stats: &RuntimeSmartContextTransformStats,
) -> String {
    let bits = u64::from(stats.tool_outputs_condensed > 0)
        | (u64::from(stats.tool_call_args_condensed > 0) << 1)
        | (u64::from(stats.duplicate_texts > 0 || stats.cross_turn_duplicate_texts > 0) << 2)
        | (u64::from(stats.repeat_tool_output_refs > 0) << 3)
        | (u64::from(stats.blob_outputs_condensed > 0) << 4)
        | (u64::from(stats.rehydrated_refs > 0) << 5)
        | (u64::from(stats.static_context_deltas > 0) << 6)
        | (u64::from(stats.repo_state_facts > 0) << 7);
    runtime_smart_context_telemetry_label(4, bits)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextExactnessReason],
) -> String {
    let bits = reasons.iter().fold(0_u64, |bits, reason| {
        bits | match reason {
            runtime_proxy_crate::SmartContextExactnessReason::ExplicitExactMode => 1,
            runtime_proxy_crate::SmartContextExactnessReason::PreviousResponseAffinity => 2,
            runtime_proxy_crate::SmartContextExactnessReason::TurnStateAffinity => 4,
            runtime_proxy_crate::SmartContextExactnessReason::SessionAffinity => 8,
            runtime_proxy_crate::SmartContextExactnessReason::ToolOutputWithoutArtifact => 16,
        }
    });
    runtime_smart_context_telemetry_label(5, bits)
}

fn runtime_smart_context_telemetry_label(kind: i64, value: u64) -> String {
    prodex_mojo_core::runtime_decisions::smart_context_telemetry_label(kind, value)
        .expect("Mojo Smart Context telemetry label returned invalid output")
}
