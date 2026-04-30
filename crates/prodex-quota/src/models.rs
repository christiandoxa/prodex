use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAuth {
    pub access_token: String,
    pub account_id: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub last_refresh: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageAuthSyncSource {
    Reloaded,
    Refreshed,
}

#[derive(Debug, Clone)]
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub additional_rate_limits: Vec<AdditionalRateLimit>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuth {
    pub auth_mode: Option<String>,
    pub tokens: Option<StoredTokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub last_refresh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTokens {
    pub access_token: Option<String>,
    pub account_id: Option<String>,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

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
