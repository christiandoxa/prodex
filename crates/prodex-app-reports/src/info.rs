use prodex_quota::{
    MainWindowSnapshot, RuntimeQuotaWindowStatus, UsageResponse, UsageWindow, WindowPair,
    find_main_window, format_precise_reset_time, remaining_percent,
};
use prodex_runtime_state::RuntimeProfileUsageSnapshot as RuntimeProfileUsageSnapshotGeneric;
use prodex_runtime_tuning::RuntimeTuningSnapshot;
use prodex_shared_types::{
    InfoQuotaAggregate, InfoQuotaSource, InfoQuotaWindow, InfoRuntimeLoadSummary,
    InfoRuntimeQuotaObservation, InfoRunwayEstimate, ProcessRow, ProdexProcessInfo,
    RunProfileProbeReport,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

mod processes;
mod quota;
mod runtime_load;
mod runtime_tuning;
mod summaries;
mod token_usage_render;
pub use processes::*;
pub use quota::*;
pub use runtime_load::*;
pub use runtime_tuning::*;
pub use summaries::*;
pub use token_usage_render::*;

pub type RuntimeProfileUsageSnapshot = RuntimeProfileUsageSnapshotGeneric<RuntimeQuotaWindowStatus>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InfoTokenUsageCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
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
#[path = "../tests/src/info.rs"]
mod tests;
