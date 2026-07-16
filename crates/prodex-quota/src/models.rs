use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub fn deserialize_null_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSummary {
    pub label: String,
    pub quota_compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotQuotaInfo {
    pub login: Option<String>,
    pub access_type_sku: Option<String>,
    pub copilot_plan: Option<String>,
    pub limited_user_quotas: BTreeMap<String, i64>,
    pub monthly_quotas: BTreeMap<String, i64>,
    pub limited_user_reset_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiQuotaInfo {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub buckets: Vec<GeminiQuotaBucket>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiQuotaBucket {
    #[serde(default, rename = "remainingAmount")]
    pub remaining_amount: Option<String>,
    #[serde(default, rename = "remainingFraction")]
    pub remaining_fraction: Option<f64>,
    #[serde(default, rename = "resetTime")]
    pub reset_time: Option<String>,
    #[serde(default, rename = "tokenType")]
    pub token_type: Option<String>,
    #[serde(default, rename = "modelId")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalQuotaInfo {
    pub provider: String,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    pub status: String,
    pub main: String,
    #[serde(default)]
    pub reset: Option<String>,
    #[serde(default)]
    pub available: Option<bool>,
    #[serde(default)]
    pub details: Vec<ExternalQuotaDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalQuotaDetail {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum ProviderQuotaSnapshot {
    OpenAi(UsageResponse),
    Copilot(CopilotQuotaInfo),
    Gemini(GeminiQuotaInfo),
    External(ExternalQuotaInfo),
}

#[derive(Debug, Clone)]
pub struct QuotaReport {
    pub name: String,
    pub active: bool,
    pub auth: AuthSummary,
    pub workspace_id: Option<String>,
    pub workspace_name: Option<String>,
    pub result: std::result::Result<ProviderQuotaSnapshot, String>,
    pub fetched_at: i64,
}

#[derive(Debug, Clone)]
pub struct RenderedQuotaReportWindow {
    pub output: String,
    pub shown_profiles: usize,
    pub total_profiles: usize,
    pub start_profile: usize,
    pub hidden_before: usize,
    pub hidden_after: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaReportSort {
    Current,
    Remaining,
    Profile,
    Auth,
    Account,
    Plan,
}

impl QuotaReportSort {
    pub fn next(self) -> Self {
        match self {
            Self::Current => Self::Remaining,
            Self::Remaining => Self::Profile,
            Self::Profile => Self::Auth,
            Self::Auth => Self::Account,
            Self::Account => Self::Plan,
            Self::Plan => Self::Current,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Remaining => "remaining",
            Self::Profile => "profile",
            Self::Auth => "auth",
            Self::Account => "account",
            Self::Plan => "plan",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaAuthFilter {
    All,
    Label(String),
    QuotaCompatible,
    NonQuotaCompatible,
}

impl QuotaAuthFilter {
    pub fn parse(raw: &str) -> Result<Self> {
        let value = raw.trim().to_ascii_lowercase();
        if value.is_empty() {
            bail!("quota auth filter cannot be empty");
        }

        Ok(match value.as_str() {
            "all" | "*" => Self::All,
            "quota-compatible" | "compatible" => Self::QuotaCompatible,
            "non-quota-compatible"
            | "not-quota-compatible"
            | "quota-incompatible"
            | "incompatible" => Self::NonQuotaCompatible,
            _ => Self::Label(value),
        })
    }

    pub fn matches(&self, auth: &AuthSummary) -> bool {
        match self {
            Self::All => true,
            Self::Label(label) => auth.label.eq_ignore_ascii_case(label),
            Self::QuotaCompatible => auth.quota_compatible,
            Self::NonQuotaCompatible => !auth.quota_compatible,
        }
    }
}

/// Clone remains intentional because the runtime auth cache takes bounded
/// snapshots before refresh and per-request handoff. Every copy zeroizes on drop.
#[derive(Clone, PartialEq, Eq)]
pub struct UsageAuth {
    pub access_token: String,
    pub account_id: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub last_refresh: Option<i64>,
}

impl fmt::Debug for UsageAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsageAuth")
            .field("access_token", &"<redacted>")
            .field(
                "account_id",
                &self.account_id.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at", &self.expires_at)
            .field("last_refresh", &self.last_refresh)
            .finish()
    }
}

impl Zeroize for UsageAuth {
    fn zeroize(&mut self) {
        self.access_token.zeroize();
        self.account_id.zeroize();
        self.refresh_token.zeroize();
    }
}

impl Drop for UsageAuth {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for UsageAuth {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageAuthSyncSource {
    Reloaded,
    Refreshed,
}

#[derive(Debug)]
pub struct UsageAuthSyncOutcome {
    pub auth: UsageAuth,
    pub source: UsageAuthSyncSource,
    pub auth_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedLimit {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageResponse {
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub rate_limit: Option<WindowPair>,
    pub code_review_rate_limit: Option<WindowPair>,
    #[serde(default, alias = "rateLimitResetCredits")]
    pub rate_limit_reset_credits: Option<RateLimitResetCreditsSummary>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub additional_rate_limits: Vec<AdditionalRateLimit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitResetCreditsSummary {
    #[serde(rename = "availableCount", alias = "available_count")]
    pub available_count: i64,
}

pub fn usage_plan_capacity_pressure_scale_bps(usage: &UsageResponse) -> i64 {
    usage
        .plan_type
        .as_deref()
        .map(plan_capacity_pressure_scale_bps)
        .unwrap_or(10_000)
}

pub fn plan_capacity_pressure_scale_bps(plan_type: &str) -> i64 {
    let normalized = plan_type
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_'))
        .collect::<String>();

    match normalized.as_str() {
        "pro20x" | "pro20" | "20x" | "ultra" | "max" => 2_000,
        "pro" | "prolite" | "pro5x" | "5x" => 5_000,
        "free" | "basic" => 12_000,
        _ => 10_000,
    }
}

pub fn scale_quota_pressure_for_plan(pressure: i64, scale_bps: i64) -> i64 {
    if pressure == i64::MAX {
        return i64::MAX;
    }

    pressure
        .saturating_mul(scale_bps.max(0))
        .checked_div(10_000)
        .unwrap_or(i64::MAX)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPair {
    pub primary_window: Option<UsageWindow>,
    pub secondary_window: Option<UsageWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalRateLimit {
    pub limit_name: Option<String>,
    pub metered_feature: Option<String>,
    pub rate_limit: WindowPair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindow {
    pub used_percent: Option<i64>,
    pub reset_at: Option<i64>,
    pub limit_window_seconds: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct StoredAuth {
    pub auth_mode: Option<String>,
    pub tokens: Option<StoredTokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub bedrock_api_key: Option<BedrockApiKeyAuth>,
    #[serde(default)]
    pub last_refresh: Option<String>,
}

impl fmt::Debug for StoredAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredAuth")
            .field("auth_mode", &self.auth_mode)
            .field("tokens", &self.tokens)
            .field(
                "openai_api_key",
                &self.openai_api_key.as_ref().map(|_| "<redacted>"),
            )
            .field("bedrock_api_key", &self.bedrock_api_key)
            .field("last_refresh", &self.last_refresh)
            .finish()
    }
}

impl Zeroize for StoredAuth {
    fn zeroize(&mut self) {
        self.tokens.zeroize();
        self.openai_api_key.zeroize();
        self.bedrock_api_key.zeroize();
    }
}

impl Drop for StoredAuth {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for StoredAuth {}

#[derive(Serialize, Deserialize)]
pub struct BedrockApiKeyAuth {
    pub api_key: Option<String>,
    pub region: Option<String>,
}

impl fmt::Debug for BedrockApiKeyAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BedrockApiKeyAuth")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("region", &self.region)
            .finish()
    }
}

impl Zeroize for BedrockApiKeyAuth {
    fn zeroize(&mut self) {
        self.api_key.zeroize();
    }
}

impl Drop for BedrockApiKeyAuth {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for BedrockApiKeyAuth {}

#[derive(Serialize, Deserialize)]
pub struct StoredTokens {
    pub access_token: Option<String>,
    pub account_id: Option<String>,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
}

impl fmt::Debug for StoredTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredTokens")
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "account_id",
                &self.account_id.as_ref().map(|_| "<redacted>"),
            )
            .field("id_token", &self.id_token.as_ref().map(|_| "<redacted>"))
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl Zeroize for StoredTokens {
    fn zeroize(&mut self) {
        self.access_token.zeroize();
        self.account_id.zeroize();
        self.id_token.zeroize();
        self.refresh_token.zeroize();
    }
}

impl Drop for StoredTokens {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for StoredTokens {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenClaims {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(rename = "https://api.openai.com/profile", default)]
    pub profile: Option<IdTokenProfileClaims>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    pub auth: Option<IdTokenAuthClaims>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenProfileClaims {
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenAuthClaims {
    #[serde(default)]
    pub chatgpt_account_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct MainWindowSnapshot {
    pub remaining_percent: i64,
    pub reset_at: i64,
    pub pressure_score: i64,
}

pub use prodex_runtime_state::RuntimeQuotaWindowStatus;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeQuotaWindowSummary {
    pub status: RuntimeQuotaWindowStatus,
    pub remaining_percent: i64,
    pub reset_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeQuotaSummary {
    pub five_hour: RuntimeQuotaWindowSummary,
    pub weekly: RuntimeQuotaWindowSummary,
    pub route_band: RuntimeQuotaPressureBand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeQuotaPressureBand {
    Healthy,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}
