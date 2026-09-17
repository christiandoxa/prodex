use prodex_quota::format_precise_reset_time;
pub use prodex_runtime_quota::{
    runtime_usage_snapshot_is_usable, usage_from_runtime_usage_snapshot,
};
use prodex_shared_types::{
    InfoQuotaWindow, InfoRuntimeLoadSummary, InfoRuntimeQuotaObservation, InfoRunwayEstimate,
    ProcessRow, ProdexProcessInfo,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

mod processes;
mod runtime_load;
mod summaries;
mod token_usage_render;
pub use processes::*;
pub use runtime_load::*;
pub use summaries::*;
pub use token_usage_render::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InfoTokenUsageCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct InfoTokenUsageEvent {
    pub timestamp: String,
    pub request: Option<u64>,
    pub profile: String,
    pub transport: String,
    pub source: String,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens_per_second: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoTokenUsageSummary {
    pub log_count: usize,
    pub event_count: usize,
    pub total: InfoTokenUsageCounts,
    pub by_profile: BTreeMap<String, InfoTokenUsageProfile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoTokenUsageProfile {
    pub event_count: usize,
    pub total: InfoTokenUsageCounts,
}

#[cfg(test)]
#[path = "../../tests/src/reports/info.rs"]
mod tests;
