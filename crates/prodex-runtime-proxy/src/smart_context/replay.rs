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

mod evaluation;
mod markdown;

pub use evaluation::smart_context_evaluate_replay_corpus;
pub use markdown::{
    smart_context_render_replay_corpus_markdown, smart_context_render_replay_evaluation_markdown,
};

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
