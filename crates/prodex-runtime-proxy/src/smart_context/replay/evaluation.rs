//! Replay corpus evaluation and aggregate metric calculation.

mod acceptance;

use super::{
    SMART_CONTEXT_REPLAY_ACCEPTANCE_THRESHOLDS, SmartContextReplayEvaluation,
    SmartContextReplayScenarioMetrics, SmartContextReplayVariant,
};
use acceptance::{
    smart_context_replay_missing_required_coverage, smart_context_replay_push_acceptance_failures,
    smart_context_replay_scenario_failures,
};

pub fn smart_context_evaluate_replay_corpus(
    metrics: &[SmartContextReplayScenarioMetrics],
) -> SmartContextReplayEvaluation {
    let exact = metrics
        .iter()
        .filter(|metric| metric.variant == SmartContextReplayVariant::Exact)
        .collect::<Vec<_>>();
    let optimized = metrics
        .iter()
        .filter(|metric| metric.variant == SmartContextReplayVariant::Optimized)
        .collect::<Vec<_>>();
    let current = metrics
        .iter()
        .filter(|metric| metric.variant == SmartContextReplayVariant::Current)
        .collect::<Vec<_>>();
    let eligible_long_optimized = optimized
        .iter()
        .copied()
        .filter(|metric| smart_context_replay_metric_eligible_long(metric))
        .collect::<Vec<_>>();

    let mut reductions = Vec::new();
    let mut current_reductions = Vec::new();
    let mut additional_reductions_vs_current = Vec::new();
    let mut reductions_at_least_20 = 0usize;
    for metric in &eligible_long_optimized {
        let Some(exact_metric) = exact
            .iter()
            .find(|candidate| candidate.scenario_id == metric.scenario_id)
        else {
            continue;
        };
        let reduction =
            smart_context_replay_reduction_percent(exact_metric.input_tokens, metric.input_tokens);
        if reduction >= 20 {
            reductions_at_least_20 = reductions_at_least_20.saturating_add(1);
        }
        reductions.push(reduction);

        if let Some(current_metric) = current
            .iter()
            .find(|candidate| candidate.scenario_id == metric.scenario_id)
        {
            current_reductions.push(smart_context_replay_reduction_percent(
                exact_metric.input_tokens,
                current_metric.input_tokens,
            ));
            additional_reductions_vs_current.push(smart_context_replay_reduction_percent(
                current_metric.input_tokens,
                metric.input_tokens,
            ));
        }
    }

    let eligible_long_sessions = reductions.len();
    let current_comparison_sessions = current_reductions.len();
    let median_reduction = smart_context_replay_median_u8(reductions);
    let current_median_reduction = smart_context_replay_median_u8(current_reductions);
    let median_additional_reduction_vs_current =
        smart_context_replay_median_u8(additional_reductions_vs_current);
    let long_sessions_with_at_least_20_percent_reduction_percent =
        smart_context_replay_ratio_percent(reductions_at_least_20, eligible_long_sessions);
    let exact_median_total_tokens_until_completion = smart_context_replay_median_u64(
        exact
            .iter()
            .map(|metric| metric.total_tokens_until_completion),
    );
    let current_median_total_tokens_until_completion = smart_context_replay_median_u64(
        current
            .iter()
            .map(|metric| metric.total_tokens_until_completion),
    );
    let optimized_median_total_tokens_until_completion = smart_context_replay_median_u64(
        optimized
            .iter()
            .map(|metric| metric.total_tokens_until_completion),
    );
    let exact_success_rate_percent = smart_context_replay_success_rate_percent(&exact);
    let current_success_rate_percent = smart_context_replay_success_rate_percent(&current);
    let optimized_success_rate_percent = smart_context_replay_success_rate_percent(&optimized);
    let optimized_missing_context_recovery_turns = optimized.iter().fold(0u64, |total, metric| {
        total.saturating_add(u64::from(metric.missing_context_recovery_turns))
    });
    let success_regression_basis_points =
        (i16::from(exact_success_rate_percent) - i16::from(optimized_success_rate_percent)) * 100;
    let continuation_integrity_percent = smart_context_replay_min_percent(&optimized, |metric| {
        metric.continuation_integrity_percent
    });
    let tool_call_integrity_percent =
        smart_context_replay_min_percent(&optimized, |metric| metric.tool_call_integrity_percent);
    let critical_signal_recall_percent = smart_context_replay_min_percent(&optimized, |metric| {
        metric.critical_signal_recall_percent
    });
    let unresolved_mandatory_artifact_refs = optimized.iter().fold(0u64, |total, metric| {
        total.saturating_add(u64::from(metric.unresolved_mandatory_artifact_refs))
    });
    let corrupted_json_count = optimized
        .iter()
        .filter(|metric| metric.corrupted_json)
        .count();
    let p95_rewrite_overhead_ms = smart_context_replay_percentile_u64(
        optimized.iter().map(|metric| metric.rewrite_overhead_ms),
        95,
    );
    let continuation_fallback_candidates = optimized
        .iter()
        .filter(|metric| {
            metric.eligible
                && metric.turns > 20
                && !metric.explicit_exact_mode
                && !metric.unsafe_request
        })
        .collect::<Vec<_>>();
    let continuation_fallbacks = continuation_fallback_candidates
        .iter()
        .filter(|metric| metric.full_request_fallback)
        .count();
    let continuation_fallback_rate_percent = smart_context_replay_ratio_percent(
        continuation_fallbacks,
        continuation_fallback_candidates.len(),
    );
    let scenario_failures = smart_context_replay_scenario_failures(&optimized, &exact);
    let missing_required_coverage = smart_context_replay_missing_required_coverage(&optimized);

    let mut evaluation = SmartContextReplayEvaluation {
        acceptance_thresholds: SMART_CONTEXT_REPLAY_ACCEPTANCE_THRESHOLDS,
        eligible_long_sessions,
        current_comparison_sessions,
        median_input_token_reduction_percent_vs_exact: median_reduction,
        current_median_input_token_reduction_percent_vs_exact: current_median_reduction,
        median_additional_input_token_reduction_percent_vs_current:
            median_additional_reduction_vs_current,
        long_sessions_with_at_least_20_percent_reduction_percent,
        exact_median_total_tokens_until_completion,
        current_median_total_tokens_until_completion,
        optimized_median_total_tokens_until_completion,
        exact_success_rate_percent,
        current_success_rate_percent,
        optimized_success_rate_percent,
        optimized_missing_context_recovery_turns,
        success_regression_basis_points,
        continuation_integrity_percent,
        tool_call_integrity_percent,
        critical_signal_recall_percent,
        unresolved_mandatory_artifact_refs,
        corrupted_json_count,
        p95_rewrite_overhead_ms,
        continuation_fallback_rate_percent,
        missing_required_coverage,
        passed: true,
        failures: Vec::new(),
        scenario_failures,
    };
    smart_context_replay_push_acceptance_failures(&mut evaluation);
    evaluation.passed = evaluation.failures.is_empty();
    evaluation
}

pub(super) fn smart_context_replay_metric_eligible_long(
    metric: &SmartContextReplayScenarioMetrics,
) -> bool {
    metric.eligible && metric.turns > 20 && !metric.explicit_exact_mode && !metric.unsafe_request
}

pub(super) fn smart_context_replay_reduction_percent(exact_tokens: u64, variant_tokens: u64) -> u8 {
    if exact_tokens == 0 || variant_tokens >= exact_tokens {
        return 0;
    }
    ((exact_tokens - variant_tokens).saturating_mul(100) / exact_tokens).min(100) as u8
}

fn smart_context_replay_success_rate_percent(metrics: &[&SmartContextReplayScenarioMetrics]) -> u8 {
    let successes = metrics
        .iter()
        .filter(|metric| metric.completion_success && metric.test_or_build_passed)
        .count();
    smart_context_replay_ratio_percent(successes, metrics.len())
}

fn smart_context_replay_min_percent(
    metrics: &[&SmartContextReplayScenarioMetrics],
    value: impl Fn(&SmartContextReplayScenarioMetrics) -> u8,
) -> u8 {
    metrics
        .iter()
        .map(|metric| value(metric))
        .min()
        .unwrap_or(100)
}

fn smart_context_replay_ratio_percent(numerator: usize, denominator: usize) -> u8 {
    if denominator == 0 {
        return 0;
    }
    (numerator.saturating_mul(100) / denominator).min(100) as u8
}

fn smart_context_replay_median_u8(mut values: Vec<u8>) -> u8 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[values.len() / 2]
}

fn smart_context_replay_median_u64(values: impl Iterator<Item = u64>) -> u64 {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[values.len() / 2]
}

fn smart_context_replay_percentile_u64(
    values: impl Iterator<Item = u64>,
    percentile: usize,
) -> u64 {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let index = values.len().saturating_mul(percentile).saturating_add(99) / 100;
    values[index.saturating_sub(1).min(values.len() - 1)]
}
