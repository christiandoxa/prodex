use prodex_quota::{AuthSummary, UsageResponse};
use prodex_state::ProfileProvider;
use std::path::PathBuf;

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
    pub result: Result<UsageResponse, String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeProfileProbeCacheEntry {
    pub checked_at: i64,
    pub auth: AuthSummary,
    pub result: Result<UsageResponse, String>,
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
