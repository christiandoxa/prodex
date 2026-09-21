pub use crate::probe::RuntimeQuotaSource as InfoQuotaSource;

#[derive(Debug, Clone)]
pub struct InfoQuotaAggregate {
    pub quota_compatible_profiles: usize,
    pub live_profiles: usize,
    pub snapshot_profiles: usize,
    pub unavailable_profiles: usize,
    pub five_hour_profiles_with_data: usize,
    pub weekly_profiles_with_data: usize,
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
