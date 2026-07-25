use super::rewrite_telemetry::RuntimeSmartContextRewriteTelemetryRecord;
use super::token_calibration::RuntimeSmartContextTokenCalibrationObservation;
use crate::runtime_state_shared::RuntimeSmartContextArtifactStore;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextPrepareError {
    pub(super) missing_artifact_count: usize,
}

impl std::fmt::Display for RuntimeSmartContextPrepareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Smart Context cannot resolve {} legacy artifact reference(s) in the active scope",
            self.missing_artifact_count
        )
    }
}

impl std::error::Error for RuntimeSmartContextPrepareError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextTransport {
    Http,
    Websocket,
}

impl RuntimeSmartContextTransport {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Websocket => "websocket",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextTransformStats {
    pub(super) artifacts_stored: usize,
    pub(super) tool_outputs_condensed: usize,
    pub(super) tool_call_args_condensed: usize,
    pub(super) duplicate_texts: usize,
    pub(super) cross_turn_duplicate_texts: usize,
    pub(super) repeat_tool_output_refs: usize,
    pub(super) blob_outputs_condensed: usize,
    pub(super) rehydrated_refs: usize,
    pub(super) rehydration_token_cost: usize,
    pub(super) static_context_deltas: usize,
    pub(super) repo_state_facts: usize,
    pub(super) candidate_count: usize,
    pub(super) selected_candidate_count: usize,
    pub(super) rejected_candidate_count: usize,
    pub(super) selected_candidate_utility_points: u64,
    pub(super) segment_rollback_count: usize,
    pub(super) full_request_fallback_count: usize,
    pub(super) artifact_hash_failures: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextTransformOutcome {
    pub(super) stats: RuntimeSmartContextTransformStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRewriteSafetyObservation {
    pub(super) safe: bool,
    pub(super) saved_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRewriteSafetyRecord {
    pub(super) observation: RuntimeSmartContextRewriteSafetyObservation,
    pub(super) observed_at_unix_secs: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeSmartContextProxyState {
    pub(super) generation: u64,
    pub(super) enabled: bool,
    pub(super) degraded_reason: Option<String>,
    pub(super) model_context_window_tokens: Option<u64>,
    pub(super) artifacts: RuntimeSmartContextArtifactStore,
    pub(super) artifact_path: Option<PathBuf>,
    pub(super) last_token_usage: Option<runtime_proxy_crate::RuntimeTokenUsage>,
    pub(super) token_usage_history: Vec<runtime_proxy_crate::RuntimeTokenUsage>,
    pub(super) token_calibration_history: Vec<RuntimeSmartContextTokenCalibrationObservation>,
    pub(super) rewrite_telemetry_history: Vec<RuntimeSmartContextRewriteTelemetryRecord>,
    pub(super) rewrite_safety_history: Vec<RuntimeSmartContextRewriteSafetyRecord>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSmartContextScopeConfig {
    pub(super) default_scope: runtime_proxy_crate::ContextScopeId,
    pub(super) profile_scopes: BTreeMap<String, runtime_proxy_crate::ContextScopeId>,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeSmartContextEngine {
    pub(super) enabled: AtomicBool,
    pub(super) states:
        RwLock<BTreeMap<runtime_proxy_crate::ContextScopeId, RuntimeSmartContextProxyState>>,
    pub(super) scope_config: RwLock<Option<RuntimeSmartContextScopeConfig>>,
}

impl RuntimeSmartContextEngine {
    pub(super) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextBudget {
    pub(super) tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    pub(super) policy: runtime_proxy_crate::SmartContextAdaptiveBudgetPolicy,
    pub(super) model_context_window_tokens: u64,
    pub(super) model_context_window_source: &'static str,
    pub(super) available_tokens: usize,
    pub(super) observed_context_tokens: Option<usize>,
    pub(super) token_usage_source: &'static str,
    pub(super) request_token_count: runtime_proxy_crate::SmartContextTokenCount,
    pub(super) pressure: runtime_proxy_crate::SmartContextPressureSnapshot,
}

pub(super) type RuntimeSmartContextBudgetInputs = (
    Vec<runtime_proxy_crate::RuntimeTokenUsage>,
    Vec<runtime_proxy_crate::RuntimeTokenUsage>,
    Vec<runtime_proxy_crate::SmartContextTokenCalibrationSample>,
    Option<u64>,
    runtime_proxy_crate::SmartContextRecentRewriteSafety,
    Vec<runtime_proxy_crate::SmartContextRewriteTelemetrySample>,
);
