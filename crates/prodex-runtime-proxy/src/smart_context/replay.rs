use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartContextReplayVariant {
    Exact,
    Current,
    Optimized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartContextReplayScenarioMetrics {
    pub scenario_id: String,
    pub variant: SmartContextReplayVariant,
    #[serde(default)]
    pub scenario_tags: Vec<String>,
    #[serde(default)]
    pub context_window_tokens: Option<u64>,
    pub eligible: bool,
    pub turns: usize,
    pub input_tokens: u64,
    pub total_tokens_until_completion: u64,
    pub completion_success: bool,
    pub test_or_build_passed: bool,
    pub critical_signal_recall_percent: u8,
    pub continuation_integrity_percent: u8,
    pub tool_call_integrity_percent: u8,
    pub missing_context_recovery_turns: u16,
    pub full_request_fallback: bool,
    pub unresolved_mandatory_artifact_refs: u16,
    pub corrupted_json: bool,
    pub rewrite_overhead_ms: u64,
    pub explicit_exact_mode: bool,
    pub unsafe_request: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayEvaluation {
    pub acceptance_thresholds: SmartContextReplayAcceptanceThresholds,
    pub eligible_long_sessions: usize,
    pub current_comparison_sessions: usize,
    pub median_input_token_reduction_percent_vs_exact: u8,
    pub current_median_input_token_reduction_percent_vs_exact: u8,
    pub median_additional_input_token_reduction_percent_vs_current: u8,
    pub long_sessions_with_at_least_20_percent_reduction_percent: u8,
    pub exact_median_total_tokens_until_completion: u64,
    pub current_median_total_tokens_until_completion: u64,
    pub optimized_median_total_tokens_until_completion: u64,
    pub exact_success_rate_percent: u8,
    pub current_success_rate_percent: u8,
    pub optimized_success_rate_percent: u8,
    pub optimized_missing_context_recovery_turns: u64,
    pub success_regression_basis_points: i16,
    pub continuation_integrity_percent: u8,
    pub tool_call_integrity_percent: u8,
    pub critical_signal_recall_percent: u8,
    pub unresolved_mandatory_artifact_refs: u64,
    pub corrupted_json_count: usize,
    pub p95_rewrite_overhead_ms: u64,
    pub continuation_fallback_rate_percent: u8,
    pub missing_required_coverage: Vec<String>,
    pub passed: bool,
    pub failures: Vec<SmartContextReplayAcceptanceFailure>,
    pub scenario_failures: Vec<SmartContextReplayScenarioFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayAcceptanceThresholds {
    pub min_median_input_token_reduction_percent_vs_exact: u8,
    pub min_long_sessions_with_at_least_20_percent_reduction_percent: u8,
    pub max_success_regression_basis_points: i16,
    pub min_continuation_integrity_percent: u8,
    pub min_tool_call_integrity_percent: u8,
    pub min_critical_signal_recall_percent: u8,
    pub max_unresolved_mandatory_artifact_refs: u64,
    pub max_corrupted_json_count: usize,
    pub max_p95_rewrite_overhead_ms: u64,
    pub max_continuation_fallback_rate_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayAcceptanceFailure {
    pub criterion: &'static str,
    pub actual: String,
    pub expected: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayScenarioFailure {
    pub scenario_id: String,
    pub criteria: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartContextReplayCorpus {
    pub metrics: Vec<SmartContextReplayScenarioMetrics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum SmartContextReplayCorpusInput {
    Metrics(Vec<SmartContextReplayScenarioMetrics>),
    Corpus(SmartContextReplayCorpus),
}

pub const SMART_CONTEXT_REPLAY_ACCEPTANCE_THRESHOLDS: SmartContextReplayAcceptanceThresholds =
    SmartContextReplayAcceptanceThresholds {
        min_median_input_token_reduction_percent_vs_exact: 35,
        min_long_sessions_with_at_least_20_percent_reduction_percent: 80,
        max_success_regression_basis_points: 100,
        min_continuation_integrity_percent: 100,
        min_tool_call_integrity_percent: 100,
        min_critical_signal_recall_percent: 100,
        max_unresolved_mandatory_artifact_refs: 0,
        max_corrupted_json_count: 0,
        max_p95_rewrite_overhead_ms: 30,
        max_continuation_fallback_rate_percent: 19,
    };

impl Default for SmartContextReplayAcceptanceThresholds {
    fn default() -> Self {
        SMART_CONTEXT_REPLAY_ACCEPTANCE_THRESHOLDS
    }
}

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

pub fn smart_context_parse_replay_corpus_json(
    text: &str,
) -> Result<SmartContextReplayCorpus, serde_json::Error> {
    match serde_json::from_str::<SmartContextReplayCorpusInput>(text)? {
        SmartContextReplayCorpusInput::Metrics(metrics) => Ok(SmartContextReplayCorpus { metrics }),
        SmartContextReplayCorpusInput::Corpus(corpus) => Ok(corpus),
    }
}

pub fn smart_context_evaluate_replay_corpus_json(
    text: &str,
) -> Result<SmartContextReplayEvaluation, serde_json::Error> {
    let corpus = smart_context_parse_replay_corpus_json(text)?;
    Ok(smart_context_evaluate_replay_corpus(&corpus.metrics))
}

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
