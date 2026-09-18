use crate::smart_context::SmartContextTokenCountSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextTokenBudgetTier {
    Exact,
    Large,
    Condensed,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextBudgetMode {
    ExactPassThrough,
    LargeLossless,
    ArtifactCondensed,
    MinimalRefsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextBudgetPolicyReason {
    ExactnessRequired,
    StaticContextChanged,
    MissingRehydrateRefs,
    UnknownTokenWindow,
    UnsafeAccounting,
    RecentRewriteSavingsSafe,
    PlentyOfBudget,
    ModerateBudget,
    TightBudget,
    CriticalBudget,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmartContextRecentRewriteSafety {
    pub safe_rewrites: usize,
    pub fallback_rewrites: usize,
    pub saved_tokens: u64,
}

pub const SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS: u64 = 256;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_RECENT_LIMIT: usize = 4;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT: usize = 2;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_RELAX_MAX_AVERAGE_BODY_RATIO_PERCENT: usize = 70;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_TIGHTEN_MIN_AVERAGE_BODY_RATIO_PERCENT: usize = 85;
pub const SMART_CONTEXT_REWRITE_BUDGET_RELAX_NUMERATOR: u64 = 5;
pub const SMART_CONTEXT_REWRITE_BUDGET_RELAX_DENOMINATOR: u64 = 4;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_NUMERATOR: u64 = 9;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_DENOMINATOR: u64 = 10;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_INLINE_BYTES: usize = 256;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_REHYDRATE_TOKENS: u64 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SmartContextRewriteBudgetDecision {
    #[default]
    NoChange,
    Relax,
    Tighten,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmartContextRewriteTelemetrySample {
    pub body_bytes_before: usize,
    pub body_bytes_after: usize,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub token_count_source: SmartContextTokenCountSource,
    pub safe: bool,
    pub fallback: bool,
    pub upstream_context_errors: u16,
    pub previous_response_not_found: bool,
    pub invalid_tool_call_continuation: bool,
    pub missing_artifact_requests: u16,
    pub repeated_tool_call_count: u16,
    pub model_reread_requests: u16,
    pub corrective_user_messages: u16,
    pub test_or_build_failed_after_rewrite: bool,
    pub task_completed: Option<bool>,
    pub additional_turns_before_task_completion: Option<u16>,
    pub final_total_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextRewriteTelemetryBudgetInput {
    pub recent_rewrite_safety: SmartContextRecentRewriteSafety,
    pub telemetry_samples: Vec<SmartContextRewriteTelemetrySample>,
}
