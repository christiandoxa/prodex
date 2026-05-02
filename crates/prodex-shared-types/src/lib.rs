use prodex_quota::AuthSummary;
use prodex_state::ProfileProvider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use prodex_quota::{
    AdditionalRateLimit, MainWindowSnapshot, RuntimeQuotaPressureBand, RuntimeQuotaSummary,
    RuntimeQuotaWindowStatus, RuntimeQuotaWindowSummary, StoredAuth, StoredTokens, UsageResponse,
    UsageWindow, WindowPair,
};
pub use prodex_runtime_proxy::RuntimeProxyRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

#[derive(Debug, Clone)]
pub struct InfoQuotaAggregate {
    pub quota_compatible_profiles: usize,
    pub live_profiles: usize,
    pub snapshot_profiles: usize,
    pub unavailable_profiles: usize,
    pub five_hour_pool_remaining: i64,
    pub weekly_pool_remaining: i64,
    pub earliest_five_hour_reset_at: Option<i64>,
    pub earliest_weekly_reset_at: Option<i64>,
}

impl InfoQuotaAggregate {
    pub fn profiles_with_data(&self) -> usize {
        self.live_profiles + self.snapshot_profiles
    }
}

#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: u32,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProdexProcessInfo {
    pub pid: u32,
    pub runtime: bool,
}

#[derive(Debug, Clone)]
pub struct InfoRuntimeQuotaObservation {
    pub timestamp: i64,
    pub profile: String,
    pub five_hour_remaining: i64,
    pub weekly_remaining: i64,
}

#[derive(Debug, Clone, Default)]
pub struct InfoRuntimeLoadSummary {
    pub log_count: usize,
    pub observations: Vec<InfoRuntimeQuotaObservation>,
    pub active_inflight_units: usize,
    pub recent_selection_events: usize,
    pub recent_first_timestamp: Option<i64>,
    pub recent_last_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub struct InfoRunwayEstimate {
    pub burn_per_hour: f64,
    pub observed_profiles: usize,
    pub observed_span_seconds: i64,
    pub exhaust_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum InfoQuotaWindow {
    FiveHour,
    Weekly,
}

#[derive(Debug, Clone)]
pub struct RunProfileProbeJob {
    pub name: String,
    pub order_index: usize,
    pub provider: ProfileProvider,
    pub codex_home: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RunProfileProbeReport {
    pub name: String,
    pub order_index: usize,
    pub auth: AuthSummary,
    pub result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeProfileProbeCacheEntry {
    pub checked_at: i64,
    pub auth: AuthSummary,
    pub result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug, Clone)]
pub struct ReadyProfileCandidate {
    pub name: String,
    pub usage: UsageResponse,
    pub order_index: usize,
    pub preferred: bool,
    pub provider_priority: usize,
    pub quota_source: RuntimeQuotaSource,
}

#[derive(Debug, Clone, Copy)]
pub struct ReadyProfileScore {
    pub total_pressure: i64,
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub reserve_floor: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

#[derive(Debug, Clone)]
pub struct RecoveredLoad<T> {
    pub value: T,
    pub recovered_from_backup: bool,
}

#[derive(Debug, Clone)]
pub struct RecoveredVersionedLoad<T> {
    pub value: T,
    pub generation: u64,
    pub recovered_from_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedJson<T> {
    #[serde(default)]
    pub generation: u64,
    pub value: T,
}
