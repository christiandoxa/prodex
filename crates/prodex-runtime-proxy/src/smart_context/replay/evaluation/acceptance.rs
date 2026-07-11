//! Replay acceptance failures, scenario failures, and required coverage checks.

use std::collections::BTreeSet;

use super::super::{
    SmartContextReplayAcceptanceFailure, SmartContextReplayEvaluation,
    SmartContextReplayScenarioFailure, SmartContextReplayScenarioMetrics,
};
use super::{smart_context_replay_metric_eligible_long, smart_context_replay_reduction_percent};

pub(super) fn smart_context_replay_scenario_failures(
    optimized: &[&SmartContextReplayScenarioMetrics],
    exact: &[&SmartContextReplayScenarioMetrics],
) -> Vec<SmartContextReplayScenarioFailure> {
    let mut failures = Vec::new();
    for metric in optimized {
        let mut criteria = Vec::new();
        if smart_context_replay_metric_eligible_long(metric) {
            if let Some(exact_metric) = exact
                .iter()
                .find(|candidate| candidate.scenario_id == metric.scenario_id)
            {
                let reduction = smart_context_replay_reduction_percent(
                    exact_metric.input_tokens,
                    metric.input_tokens,
                );
                if reduction < 20 {
                    criteria.push(format!("input_token_reduction_percent={reduction}<20"));
                }
            } else {
                criteria.push("missing_exact_baseline".to_string());
            }
        }
        if !metric.completion_success {
            criteria.push("completion_success=false".to_string());
        }
        if !metric.test_or_build_passed {
            criteria.push("test_or_build_passed=false".to_string());
        }
        if metric.critical_signal_recall_percent < 100 {
            criteria.push(format!(
                "critical_signal_recall_percent={}<100",
                metric.critical_signal_recall_percent
            ));
        }
        if metric.continuation_integrity_percent < 100 {
            criteria.push(format!(
                "continuation_integrity_percent={}<100",
                metric.continuation_integrity_percent
            ));
        }
        if metric.tool_call_integrity_percent < 100 {
            criteria.push(format!(
                "tool_call_integrity_percent={}<100",
                metric.tool_call_integrity_percent
            ));
        }
        if metric.unresolved_mandatory_artifact_refs > 0 {
            criteria.push(format!(
                "unresolved_mandatory_artifact_refs={}>0",
                metric.unresolved_mandatory_artifact_refs
            ));
        }
        if metric.corrupted_json {
            criteria.push("corrupted_json=true".to_string());
        }
        if metric.rewrite_overhead_ms > 30 {
            criteria.push(format!(
                "rewrite_overhead_ms={}>30",
                metric.rewrite_overhead_ms
            ));
        }
        if smart_context_replay_metric_eligible_long(metric) && metric.full_request_fallback {
            criteria.push("full_request_fallback=true".to_string());
        }
        if !criteria.is_empty() {
            failures.push(SmartContextReplayScenarioFailure {
                scenario_id: metric.scenario_id.clone(),
                criteria,
            });
        }
    }
    failures
}

pub(super) fn smart_context_replay_push_acceptance_failures(
    evaluation: &mut SmartContextReplayEvaluation,
) {
    let thresholds = evaluation.acceptance_thresholds;
    smart_context_replay_require_min(
        evaluation,
        "median_input_token_reduction_percent_vs_exact",
        u64::from(evaluation.median_input_token_reduction_percent_vs_exact),
        u64::from(thresholds.min_median_input_token_reduction_percent_vs_exact),
    );
    smart_context_replay_require_min(
        evaluation,
        "long_sessions_with_at_least_20_percent_reduction_percent",
        u64::from(evaluation.long_sessions_with_at_least_20_percent_reduction_percent),
        u64::from(thresholds.min_long_sessions_with_at_least_20_percent_reduction_percent),
    );
    smart_context_replay_require_max_i16(
        evaluation,
        "success_regression_basis_points",
        evaluation.success_regression_basis_points,
        thresholds.max_success_regression_basis_points,
    );
    smart_context_replay_require_min(
        evaluation,
        "continuation_integrity_percent",
        u64::from(evaluation.continuation_integrity_percent),
        u64::from(thresholds.min_continuation_integrity_percent),
    );
    smart_context_replay_require_min(
        evaluation,
        "tool_call_integrity_percent",
        u64::from(evaluation.tool_call_integrity_percent),
        u64::from(thresholds.min_tool_call_integrity_percent),
    );
    smart_context_replay_require_min(
        evaluation,
        "critical_signal_recall_percent",
        u64::from(evaluation.critical_signal_recall_percent),
        u64::from(thresholds.min_critical_signal_recall_percent),
    );
    smart_context_replay_require_max(
        evaluation,
        "unresolved_mandatory_artifact_refs",
        evaluation.unresolved_mandatory_artifact_refs,
        thresholds.max_unresolved_mandatory_artifact_refs,
    );
    smart_context_replay_require_max(
        evaluation,
        "corrupted_json_count",
        evaluation.corrupted_json_count as u64,
        thresholds.max_corrupted_json_count as u64,
    );
    smart_context_replay_require_max(
        evaluation,
        "p95_rewrite_overhead_ms",
        evaluation.p95_rewrite_overhead_ms,
        thresholds.max_p95_rewrite_overhead_ms,
    );
    smart_context_replay_require_max(
        evaluation,
        "continuation_fallback_rate_percent",
        u64::from(evaluation.continuation_fallback_rate_percent),
        u64::from(thresholds.max_continuation_fallback_rate_percent),
    );
    if !evaluation.missing_required_coverage.is_empty() {
        evaluation
            .failures
            .push(SmartContextReplayAcceptanceFailure {
                criterion: "required_replay_coverage",
                actual: evaluation.missing_required_coverage.join(", "),
                expected: "complete required scenario coverage".to_string(),
            });
    }
}

pub(super) fn smart_context_replay_missing_required_coverage(
    optimized: &[&SmartContextReplayScenarioMetrics],
) -> Vec<String> {
    let mut tags = BTreeSet::new();
    let mut windows = BTreeSet::new();
    let mut has_long_continuation = false;
    for metric in optimized
        .iter()
        .copied()
        .filter(|metric| metric.eligible && !metric.explicit_exact_mode && !metric.unsafe_request)
    {
        if metric.turns >= 30 {
            has_long_continuation = true;
        }
        if let Some(window) = metric.context_window_tokens {
            windows.insert(window);
        }
        for tag in &metric.scenario_tags {
            let normalized = tag.trim().to_ascii_lowercase().replace('-', "_");
            if !normalized.is_empty() {
                tags.insert(normalized);
            }
        }
    }

    let mut missing = Vec::new();
    if !has_long_continuation {
        missing.push("30_plus_turn_continuation_session".to_string());
    }
    for required in [
        "repeated_build_test_output",
        "compiler_runtime_errors",
        "large_git_diffs",
        "repository_navigation",
        "multi_file_refactoring",
        "changing_static_instructions",
        "missing_corrupted_artifacts",
        "duplicate_tool_calls_outputs",
        "noisy_binary_like_commands",
        "failure_recovery",
    ] {
        if !tags.contains(required) {
            missing.push(required.to_string());
        }
    }
    for (label, accepted) in [
        ("window_16k", [16_000, 16_384, 0, 0]),
        ("window_32k", [32_000, 32_768, 0, 0]),
        ("window_128k", [128_000, 131_072, 0, 0]),
        ("window_200k", [200_000, 0, 0, 0]),
    ] {
        if !accepted
            .into_iter()
            .filter(|window| *window > 0)
            .any(|window| windows.contains(&window))
        {
            missing.push(label.to_string());
        }
    }
    missing
}

fn smart_context_replay_require_min(
    evaluation: &mut SmartContextReplayEvaluation,
    criterion: &'static str,
    actual: u64,
    expected: u64,
) {
    if actual < expected {
        evaluation
            .failures
            .push(SmartContextReplayAcceptanceFailure {
                criterion,
                actual: actual.to_string(),
                expected: format!(">= {expected}"),
            });
    }
}

fn smart_context_replay_require_max(
    evaluation: &mut SmartContextReplayEvaluation,
    criterion: &'static str,
    actual: u64,
    expected: u64,
) {
    if actual > expected {
        evaluation
            .failures
            .push(SmartContextReplayAcceptanceFailure {
                criterion,
                actual: actual.to_string(),
                expected: format!("<= {expected}"),
            });
    }
}

fn smart_context_replay_require_max_i16(
    evaluation: &mut SmartContextReplayEvaluation,
    criterion: &'static str,
    actual: i16,
    expected: i16,
) {
    if actual > expected {
        evaluation
            .failures
            .push(SmartContextReplayAcceptanceFailure {
                criterion,
                actual: actual.to_string(),
                expected: format!("<= {expected}"),
            });
    }
}
