use super::*;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UsageResponse {
    pub(crate) email: Option<String>,
    pub(crate) plan_type: Option<String>,
    pub(crate) rate_limit: Option<WindowPair>,
    pub(crate) code_review_rate_limit: Option<WindowPair>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub(crate) additional_rate_limits: Vec<AdditionalRateLimit>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WindowPair {
    pub(crate) primary_window: Option<UsageWindow>,
    pub(crate) secondary_window: Option<UsageWindow>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AdditionalRateLimit {
    pub(crate) limit_name: Option<String>,
    pub(crate) metered_feature: Option<String>,
    pub(crate) rate_limit: WindowPair,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UsageWindow {
    pub(crate) used_percent: Option<i64>,
    pub(crate) reset_at: Option<i64>,
    pub(crate) limit_window_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StoredAuth {
    pub(crate) auth_mode: Option<String>,
    pub(crate) tokens: Option<StoredTokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    pub(crate) openai_api_key: Option<String>,
    #[serde(default)]
    pub(crate) last_refresh: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StoredTokens {
    pub(crate) access_token: Option<String>,
    pub(crate) account_id: Option<String>,
    pub(crate) id_token: Option<String>,
    pub(crate) refresh_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IdTokenClaims {
    #[serde(default)]
    pub(crate) email: Option<String>,
    #[serde(rename = "https://api.openai.com/profile", default)]
    pub(crate) profile: Option<IdTokenProfileClaims>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IdTokenProfileClaims {
    #[serde(default)]
    pub(crate) email: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InfoQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

#[derive(Debug, Clone)]
pub(crate) struct InfoQuotaAggregate {
    pub(crate) quota_compatible_profiles: usize,
    pub(crate) live_profiles: usize,
    pub(crate) snapshot_profiles: usize,
    pub(crate) unavailable_profiles: usize,
    pub(crate) five_hour_pool_remaining: i64,
    pub(crate) weekly_pool_remaining: i64,
    pub(crate) earliest_five_hour_reset_at: Option<i64>,
    pub(crate) earliest_weekly_reset_at: Option<i64>,
}

impl InfoQuotaAggregate {
    pub(crate) fn profiles_with_data(&self) -> usize {
        self.live_profiles + self.snapshot_profiles
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessRow {
    pub(crate) pid: u32,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProdexProcessInfo {
    pub(crate) pid: u32,
    pub(crate) runtime: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct InfoRuntimeQuotaObservation {
    pub(crate) timestamp: i64,
    pub(crate) profile: String,
    pub(crate) five_hour_remaining: i64,
    pub(crate) weekly_remaining: i64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InfoRuntimeLoadSummary {
    pub(crate) log_count: usize,
    pub(crate) observations: Vec<InfoRuntimeQuotaObservation>,
    pub(crate) active_inflight_units: usize,
    pub(crate) recent_selection_events: usize,
    pub(crate) recent_first_timestamp: Option<i64>,
    pub(crate) recent_last_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct InfoRunwayEstimate {
    pub(crate) burn_per_hour: f64,
    pub(crate) observed_profiles: usize,
    pub(crate) observed_span_seconds: i64,
    pub(crate) exhaust_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum InfoQuotaWindow {
    FiveHour,
    Weekly,
}

pub(crate) struct ProfileEmailLookupJob {
    pub(crate) name: String,
    pub(crate) codex_home: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct RunProfileProbeJob {
    pub(crate) name: String,
    pub(crate) order_index: usize,
    pub(crate) provider: ProfileProvider,
    pub(crate) codex_home: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct RunProfileProbeReport {
    pub(crate) name: String,
    pub(crate) order_index: usize,
    pub(crate) auth: AuthSummary,
    pub(crate) result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ReadyProfileCandidate {
    pub(crate) name: String,
    pub(crate) usage: UsageResponse,
    pub(crate) order_index: usize,
    pub(crate) preferred: bool,
    pub(crate) provider_priority: usize,
    pub(crate) quota_source: RuntimeQuotaSource,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MainWindowSnapshot {
    pub(crate) remaining_percent: i64,
    pub(crate) reset_at: i64,
    pub(crate) pressure_score: i64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReadyProfileScore {
    pub(crate) total_pressure: i64,
    pub(crate) weekly_pressure: i64,
    pub(crate) five_hour_pressure: i64,
    pub(crate) reserve_floor: i64,
    pub(crate) weekly_remaining: i64,
    pub(crate) five_hour_remaining: i64,
    pub(crate) weekly_reset_at: i64,
    pub(crate) five_hour_reset_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RuntimeQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeQuotaWindowSummary {
    pub(crate) status: RuntimeQuotaWindowStatus,
    pub(crate) remaining_percent: i64,
    pub(crate) reset_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeQuotaSummary {
    pub(crate) five_hour: RuntimeQuotaWindowSummary,
    pub(crate) weekly: RuntimeQuotaWindowSummary,
    pub(crate) route_band: RuntimeQuotaPressureBand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RuntimeQuotaPressureBand {
    Healthy,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProxyRequest {
    pub(crate) method: String,
    pub(crate) path_and_query: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct RecoveredLoad<T> {
    pub(crate) value: T,
    pub(crate) recovered_from_backup: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RecoveredVersionedLoad<T> {
    pub(crate) value: T,
    pub(crate) generation: u64,
    pub(crate) recovered_from_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VersionedJson<T> {
    #[serde(default)]
    pub(crate) generation: u64,
    pub(crate) value: T,
}
