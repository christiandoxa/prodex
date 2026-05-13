use crate::smart_context::SmartContextTokenCalibrationBucketKey;

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

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SmartContextRewritePolicyBucketKey {
    pub route: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
}

impl From<SmartContextTokenCalibrationBucketKey> for SmartContextRewritePolicyBucketKey {
    fn from(value: SmartContextTokenCalibrationBucketKey) -> Self {
        Self {
            route: value.route,
            model: value.model,
            profile: value.profile,
        }
    }
}

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
    pub estimated_tokens_before: u64,
    pub estimated_tokens_after: u64,
    pub safe: bool,
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextTransformCategory {
    StructuralMinifyJson,
    StaticContextPromptCache,
    CommandOutputCache,
    ToolOutputArtifact,
    CrossTurnDuplicateRef,
    AutoRehydrate,
    MemoryCapsuleSelection,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRewriteTelemetrySample {
    pub category: SmartContextTransformCategory,
    pub sample: SmartContextRewriteTelemetrySample,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRecentRewriteSafety {
    pub category: SmartContextTransformCategory,
    pub safety: SmartContextRecentRewriteSafety,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextTransformRewriteSafetyReason {
    NoTransformSamples,
    InsufficientTransformSamples,
    RecentSafetyFallback,
    FallbackObserved,
    UnsafeSample,
    WeakSavings,
    ModerateSavings,
    StableSavings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRewriteSafetyScoreInput {
    pub category: SmartContextTransformCategory,
    pub recent_rewrite_safety: SmartContextRecentRewriteSafety,
    pub telemetry_samples: Vec<SmartContextTransformRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextPerTransformRewriteSafetyInput {
    pub recent_rewrite_safety: Vec<SmartContextTransformRecentRewriteSafety>,
    pub telemetry_samples: Vec<SmartContextTransformRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRewriteSafetyScore {
    pub category: SmartContextTransformCategory,
    pub decision: SmartContextRewriteBudgetDecision,
    pub safety_score: i32,
    pub telemetry_samples: usize,
    pub safety_samples: usize,
    pub safe_samples: usize,
    pub fallback_samples: usize,
    pub unsafe_samples: usize,
    pub weak_savings_samples: usize,
    pub saved_tokens: u64,
    pub average_body_ratio_percent: Option<usize>,
    pub reasons: Vec<SmartContextTransformRewriteSafetyReason>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextRewriteTelemetryBudgetInput {
    pub recent_rewrite_safety: SmartContextRecentRewriteSafety,
    pub telemetry_samples: Vec<SmartContextRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextBucketedRewriteTelemetrySample {
    pub bucket_key: Option<SmartContextRewritePolicyBucketKey>,
    pub sample: SmartContextRewriteTelemetrySample,
}
