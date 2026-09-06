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

#[derive(Debug, Clone, Serialize)]
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

impl<'de> Deserialize<'de> for UsageResponse {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawUsageResponse::deserialize(deserializer)?;
        let indexed_rate_limits = raw.rate_limits_by_limit_id.unwrap_or_default();
        let indexed_main = indexed_rate_limits
            .iter()
            .find(|(id, pair)| {
                id.eq_ignore_ascii_case("codex")
                    || extra_string(&pair.extra, &["limit_id", "limitId"])
                        .is_some_and(|value| value.eq_ignore_ascii_case("codex"))
            })
            .map(|(_, pair)| pair.clone());
        let mut rate_limit = indexed_main.or(raw.rate_limit).or(raw.rate_limits);
        let backend_blocked = raw
            .rate_limit_reached_type
            .as_ref()
            .is_some_and(|value| !value.is_null());
        if let Some(pair) = rate_limit.as_mut() {
            let ordinary_usage_allowed = raw
                .extra
                .get("ordinaryUsageAllowed")
                .or_else(|| raw.extra.get("ordinary_usage_allowed"));
            match ordinary_usage_allowed {
                Some(value) if value.as_bool() == Some(false) => pair.allowed = Some(false),
                Some(value) if value.as_bool() == Some(true) => {}
                Some(value) => {
                    pair.extra
                        .insert("ordinaryUsageAllowed".to_string(), value.clone());
                }
                None => {}
            }
            if backend_blocked {
                pair.allowed = Some(false);
            }
        }
        let plan_type = raw.plan_type.or_else(|| {
            rate_limit
                .as_ref()
                .and_then(|pair| extra_string(&pair.extra, &["plan_type", "planType"]))
        });
        let mut additional_rate_limits = raw.additional_rate_limits;

        for (limit_id, pair) in indexed_rate_limits {
            if limit_id.eq_ignore_ascii_case("codex")
                || extra_string(&pair.extra, &["limit_id", "limitId"])
                    .is_some_and(|value| value.eq_ignore_ascii_case("codex"))
                || additional_rate_limits.iter().any(|additional| {
                    additional
                        .limit_id
                        .as_deref()
                        .is_some_and(|id| id.eq_ignore_ascii_case(&limit_id))
                })
            {
                continue;
            }
            additional_rate_limits.push(additional_rate_limit_from_indexed_pair(limit_id, pair));
        }

        Ok(Self {
            email: raw.email,
            plan_type,
            rate_limit,
            code_review_rate_limit: raw.code_review_rate_limit,
            rate_limit_reset_credits: raw.rate_limit_reset_credits,
            additional_rate_limits,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawUsageResponse {
    #[serde(default)]
    email: Option<String>,
    #[serde(default, alias = "planType")]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit: Option<WindowPair>,
    #[serde(default, rename = "rateLimits", alias = "rate_limits")]
    rate_limits: Option<WindowPair>,
    #[serde(default, alias = "codeReviewRateLimit")]
    code_review_rate_limit: Option<WindowPair>,
    #[serde(default, alias = "rateLimitResetCredits")]
    rate_limit_reset_credits: Option<RateLimitResetCreditsSummary>,
    #[serde(
        default,
        alias = "additionalRateLimits",
        deserialize_with = "deserialize_null_default"
    )]
    additional_rate_limits: Vec<AdditionalRateLimit>,
    #[serde(default, alias = "rateLimitsByLimitId")]
    rate_limits_by_limit_id: Option<BTreeMap<String, WindowPair>>,
    #[serde(default, alias = "rateLimitReachedType")]
    rate_limit_reached_type: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

fn additional_rate_limit_from_indexed_pair(
    limit_id: String,
    pair: WindowPair,
) -> AdditionalRateLimit {
    let limit_id = extra_string(&pair.extra, &["limit_id", "limitId"])
        .or_else(|| (!limit_id.trim().is_empty()).then_some(limit_id));
    let limit_name = extra_string(&pair.extra, &["limit_name", "limitName"]);
    let metered_feature = extra_string(&pair.extra, &["metered_feature", "meteredFeature"]);
    let extra = pair.extra.clone();
    AdditionalRateLimit {
        limit_id,
        limit_name,
        metered_feature,
        allowed: pair.allowed,
        limit_reached: pair.limit_reached,
        rate_limit: pair,
        extra,
    }
}

fn extra_string(extra: &BTreeMap<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        extra
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowPair {
    /// Explicit backend admission state. `None` means unavailable, not denied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<bool>,
    #[serde(
        default,
        alias = "limitReached",
        skip_serializing_if = "Option::is_none"
    )]
    pub limit_reached: Option<bool>,
    #[serde(default, alias = "primary", alias = "primaryWindow")]
    pub primary_window: Option<UsageWindow>,
    #[serde(default, alias = "secondary", alias = "secondaryWindow")]
    pub secondary_window: Option<UsageWindow>,
    /// Preserve future fields from the provider's rate-limit object.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalRateLimit {
    /// Optional backend bucket identifier. Unknown identifiers remain generic and non-routable.
    #[serde(default, alias = "limitId", skip_serializing_if = "Option::is_none")]
    pub limit_id: Option<String>,
    #[serde(alias = "limitName")]
    pub limit_name: Option<String>,
    #[serde(alias = "meteredFeature")]
    pub metered_feature: Option<String>,
    #[serde(
        default,
        alias = "rateLimit",
        deserialize_with = "deserialize_null_default"
    )]
    pub rate_limit: WindowPair,
    /// Exact backend admission state when the usage endpoint provides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "limitReached")]
    pub limit_reached: Option<bool>,
    /// Retain fields introduced by a newer usage endpoint; unknown fields are not routing authority.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageWindow {
    pub used_percent: Option<i64>,
    pub reset_at: Option<i64>,
    pub limit_window_seconds: Option<i64>,
}

impl<'de> Deserialize<'de> for UsageWindow {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawUsageWindow::deserialize(deserializer)?;
        Ok(Self {
            used_percent: raw.used_percent,
            reset_at: raw.reset_at,
            limit_window_seconds: raw.limit_window_seconds.or_else(|| {
                raw.window_duration_mins
                    .and_then(|minutes| minutes.checked_mul(60))
            }),
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawUsageWindow {
    #[serde(default, alias = "usedPercent")]
    used_percent: Option<i64>,
    #[serde(default, alias = "resetAt", alias = "resetsAt")]
    reset_at: Option<i64>,
    #[serde(default, alias = "limitWindowSeconds")]
    limit_window_seconds: Option<i64>,
    #[serde(default, alias = "windowDurationMins")]
    window_duration_mins: Option<i64>,
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
