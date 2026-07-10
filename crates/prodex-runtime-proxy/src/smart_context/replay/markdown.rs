//! Markdown rendering for smart-context replay evaluation reports.

use super::{SmartContextReplayEvaluation, smart_context_evaluate_replay_corpus_json};

pub fn smart_context_render_replay_corpus_markdown(
    text: &str,
) -> Result<String, serde_json::Error> {
    let evaluation = smart_context_evaluate_replay_corpus_json(text)?;
    Ok(smart_context_render_replay_evaluation_markdown(&evaluation))
}

pub fn smart_context_render_replay_evaluation_markdown(
    evaluation: &SmartContextReplayEvaluation,
) -> String {
    let mut report = String::new();
    report.push_str("# Smart Context Replay Evaluation\n\n");
    report.push_str("## Acceptance Thresholds\n\n");
    report.push_str(&format!(
        "- min_median_input_token_reduction_percent_vs_exact: {}\n",
        evaluation
            .acceptance_thresholds
            .min_median_input_token_reduction_percent_vs_exact
    ));
    report.push_str(&format!(
        "- min_long_sessions_with_at_least_20_percent_reduction_percent: {}\n",
        evaluation
            .acceptance_thresholds
            .min_long_sessions_with_at_least_20_percent_reduction_percent
    ));
    report.push_str(&format!(
        "- max_success_regression_basis_points: {}\n",
        evaluation
            .acceptance_thresholds
            .max_success_regression_basis_points
    ));
    report.push_str(&format!(
        "- min_continuation_integrity_percent: {}\n",
        evaluation
            .acceptance_thresholds
            .min_continuation_integrity_percent
    ));
    report.push_str(&format!(
        "- min_tool_call_integrity_percent: {}\n",
        evaluation
            .acceptance_thresholds
            .min_tool_call_integrity_percent
    ));
    report.push_str(&format!(
        "- min_critical_signal_recall_percent: {}\n",
        evaluation
            .acceptance_thresholds
            .min_critical_signal_recall_percent
    ));
    report.push_str(&format!(
        "- max_unresolved_mandatory_artifact_refs: {}\n",
        evaluation
            .acceptance_thresholds
            .max_unresolved_mandatory_artifact_refs
    ));
    report.push_str(&format!(
        "- max_corrupted_json_count: {}\n",
        evaluation.acceptance_thresholds.max_corrupted_json_count
    ));
    report.push_str(&format!(
        "- max_p95_rewrite_overhead_ms: {}\n",
        evaluation.acceptance_thresholds.max_p95_rewrite_overhead_ms
    ));
    report.push_str(&format!(
        "- max_continuation_fallback_rate_percent: {}\n\n",
        evaluation
            .acceptance_thresholds
            .max_continuation_fallback_rate_percent
    ));
    report.push_str("## Measurements\n\n");
    report.push_str(&format!(
        "- eligible_long_sessions: {}\n",
        evaluation.eligible_long_sessions
    ));
    report.push_str(&format!(
        "- current_comparison_sessions: {}\n",
        evaluation.current_comparison_sessions
    ));
    report.push_str(&format!(
        "- median_input_token_reduction_percent_vs_exact: {}\n",
        evaluation.median_input_token_reduction_percent_vs_exact
    ));
    report.push_str(&format!(
        "- current_median_input_token_reduction_percent_vs_exact: {}\n",
        evaluation.current_median_input_token_reduction_percent_vs_exact
    ));
    report.push_str(&format!(
        "- median_additional_input_token_reduction_percent_vs_current: {}\n",
        evaluation.median_additional_input_token_reduction_percent_vs_current
    ));
    report.push_str(&format!(
        "- long_sessions_with_at_least_20_percent_reduction_percent: {}\n",
        evaluation.long_sessions_with_at_least_20_percent_reduction_percent
    ));
    report.push_str(&format!(
        "- exact_median_total_tokens_until_completion: {}\n",
        evaluation.exact_median_total_tokens_until_completion
    ));
    report.push_str(&format!(
        "- current_median_total_tokens_until_completion: {}\n",
        evaluation.current_median_total_tokens_until_completion
    ));
    report.push_str(&format!(
        "- optimized_median_total_tokens_until_completion: {}\n",
        evaluation.optimized_median_total_tokens_until_completion
    ));
    report.push_str(&format!(
        "- exact_success_rate_percent: {}\n",
        evaluation.exact_success_rate_percent
    ));
    report.push_str(&format!(
        "- current_success_rate_percent: {}\n",
        evaluation.current_success_rate_percent
    ));
    report.push_str(&format!(
        "- optimized_success_rate_percent: {}\n",
        evaluation.optimized_success_rate_percent
    ));
    report.push_str(&format!(
        "- optimized_missing_context_recovery_turns: {}\n",
        evaluation.optimized_missing_context_recovery_turns
    ));
    report.push_str(&format!(
        "- success_regression_basis_points: {}\n",
        evaluation.success_regression_basis_points
    ));
    report.push_str(&format!(
        "- continuation_integrity_percent: {}\n",
        evaluation.continuation_integrity_percent
    ));
    report.push_str(&format!(
        "- tool_call_integrity_percent: {}\n",
        evaluation.tool_call_integrity_percent
    ));
    report.push_str(&format!(
        "- critical_signal_recall_percent: {}\n",
        evaluation.critical_signal_recall_percent
    ));
    report.push_str(&format!(
        "- unresolved_mandatory_artifact_refs: {}\n",
        evaluation.unresolved_mandatory_artifact_refs
    ));
    report.push_str(&format!(
        "- corrupted_json_count: {}\n",
        evaluation.corrupted_json_count
    ));
    report.push_str(&format!(
        "- p95_rewrite_overhead_ms: {}\n",
        evaluation.p95_rewrite_overhead_ms
    ));
    report.push_str(&format!(
        "- continuation_fallback_rate_percent: {}\n",
        evaluation.continuation_fallback_rate_percent
    ));
    if evaluation.missing_required_coverage.is_empty() {
        report.push_str("- required_replay_coverage: complete\n");
    } else {
        report.push_str(&format!(
            "- missing_required_coverage: {}\n",
            evaluation.missing_required_coverage.join(", ")
        ));
    }
    report.push_str(&format!("- passed: {}\n", evaluation.passed));
    if !evaluation.failures.is_empty() {
        report.push_str("\n## Failures\n\n");
        for failure in &evaluation.failures {
            report.push_str(&format!(
                "- {}: actual {}, expected {}\n",
                failure.criterion, failure.actual, failure.expected
            ));
        }
    }
    if !evaluation.scenario_failures.is_empty() {
        report.push_str("\n## Scenario Failures\n\n");
        for failure in &evaluation.scenario_failures {
            report.push_str(&format!(
                "- {}: {}\n",
                failure.scenario_id,
                failure.criteria.join(", ")
            ));
        }
    }
    report
}
