use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::{AuthSummary, StoredAuth, UsageAuth, UsageAuthSyncOutcome, UsageAuthSyncSource};

const CHATGPT_AUTH_REFRESH_INTERVAL_DAYS: i64 = 8;
const CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS: i64 = if cfg!(test) { 30 } else { 5 * 60 };

pub fn auth_summary_from_auth_text_result<E>(
    result: std::result::Result<Option<String>, E>,
) -> AuthSummary {
    match result {
        Ok(Some(content)) => auth_summary_from_auth_text(&Zeroizing::new(content)),
        Ok(None) => AuthSummary {
            label: prodex_mojo_core::quota::quota_auth_summary_label(5)
                .expect("Mojo quota auth-summary label returned invalid output")
                .to_string(),
            quota_compatible: false,
        },
        Err(_) => AuthSummary {
            label: prodex_mojo_core::quota::quota_auth_summary_label(4)
                .expect("Mojo quota auth-summary label returned invalid output")
                .to_string(),
            quota_compatible: false,
        },
    }
}

pub fn auth_summary_from_auth_text(content: &str) -> AuthSummary {
    let stored_auth: StoredAuth = match serde_json::from_str(content) {
        Ok(auth) => auth,
        Err(_) => {
            return AuthSummary {
                label: prodex_mojo_core::quota::quota_auth_summary_label(6)
                    .expect("Mojo quota auth-summary label returned invalid output")
                    .to_string(),
                quota_compatible: false,
            };
        }
    };
    auth_summary_from_stored_auth(&stored_auth)
}

pub fn auth_summary_from_stored_auth(stored_auth: &StoredAuth) -> AuthSummary {
    let has_chatgpt_token = stored_auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.access_token.as_deref())
        .is_some_and(|token| !token.trim().is_empty());
    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());
    let has_bedrock_api_key = stored_auth.bedrock_api_key.as_ref().is_some_and(|auth| {
        auth.api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
    });
    let kind = prodex_mojo_core::quota::quota_auth_summary_kind(
        stored_auth.auth_mode.as_deref(),
        has_chatgpt_token,
        has_api_key,
        has_bedrock_api_key,
    )
    .expect("Mojo quota auth-summary policy returned invalid output");
    let label = match kind {
        prodex_mojo_core::quota::QuotaAuthSummaryKind::Other => {
            stored_auth.auth_mode.clone().unwrap_or_else(|| {
                prodex_mojo_core::quota::quota_auth_summary_label(3)
                    .expect("Mojo quota auth-summary label returned invalid output")
                    .to_string()
            })
        }
        prodex_mojo_core::quota::QuotaAuthSummaryKind::Chatgpt => {
            prodex_mojo_core::quota::quota_auth_summary_label(0)
                .expect("Mojo quota auth-summary label returned invalid output")
                .to_string()
        }
        prodex_mojo_core::quota::QuotaAuthSummaryKind::BedrockApiKey => {
            prodex_mojo_core::quota::quota_auth_summary_label(1)
                .expect("Mojo quota auth-summary label returned invalid output")
                .to_string()
        }
        prodex_mojo_core::quota::QuotaAuthSummaryKind::ApiKey => {
            prodex_mojo_core::quota::quota_auth_summary_label(2)
                .expect("Mojo quota auth-summary label returned invalid output")
                .to_string()
        }
    };
    AuthSummary {
        label,
        quota_compatible: kind == prodex_mojo_core::quota::QuotaAuthSummaryKind::Chatgpt,
    }
}

pub fn usage_auth_from_stored_auth(stored_auth: &StoredAuth) -> Result<UsageAuth> {
    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());
    let has_bedrock_api_key = stored_auth.bedrock_api_key.as_ref().is_some_and(|auth| {
        auth.api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
    });
    match prodex_mojo_core::quota::quota_usage_auth_kind(
        stored_auth.auth_mode.as_deref(),
        has_api_key,
        has_bedrock_api_key,
    )
    .expect("Mojo quota usage-auth compatibility policy returned invalid output")
    {
        prodex_mojo_core::quota::QuotaUsageAuthKind::BedrockApiKey => {
            bail!(
                "quota endpoint requires a ChatGPT access token. Amazon Bedrock API key auth is provider-managed."
            );
        }
        prodex_mojo_core::quota::QuotaUsageAuthKind::ApiKey => {
            bail!("quota endpoint requires a ChatGPT access token. Run `codex login` first.");
        }
        prodex_mojo_core::quota::QuotaUsageAuthKind::ChatgptEligible => {}
    }

    let tokens = stored_auth
        .tokens
        .as_ref()
        .context("auth tokens are missing from the stored auth secret")?;
    let access_token = tokens
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .context("access token not found in the stored auth secret")?
        .to_string();
    let stored_account_id = tokens
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(ToOwned::to_owned);
    let account_id = parse_jwt_chatgpt_account_id(&access_token)
        .ok()
        .flatten()
        .or(stored_account_id);
    let refresh_token = tokens
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned);
    let expires_at = parse_jwt_expiration(&access_token).ok().flatten();
    let last_refresh = stored_auth
        .last_refresh
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp());

    Ok(UsageAuth {
        access_token,
        account_id,
        refresh_token,
        expires_at,
        last_refresh,
    })
}

pub fn usage_auth_needs_proactive_refresh(auth: &UsageAuth, now: i64) -> bool {
    usage_auth_needs_proactive_refresh_with_policy(
        auth,
        now,
        CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS,
        CHATGPT_AUTH_REFRESH_INTERVAL_DAYS,
    )
}

pub fn usage_auth_needs_proactive_refresh_with_policy(
    auth: &UsageAuth,
    now: i64,
    expiry_skew_seconds: i64,
    refresh_interval_days: i64,
) -> bool {
    prodex_mojo_core::quota::quota_auth_needs_proactive_refresh(
        auth.expires_at,
        auth.last_refresh,
        now,
        expiry_skew_seconds,
        refresh_interval_days,
    )
    .expect("Mojo quota auth-refresh policy returned invalid output")
}

pub fn usage_auth_sync_source_label(source: UsageAuthSyncSource) -> &'static str {
    prodex_mojo_core::quota::quota_usage_auth_sync_source_label(source as i64)
        .expect("Mojo quota auth-sync source label returned invalid output")
}

pub fn usage_auth_changed(expected_current: Option<&UsageAuth>, candidate: &UsageAuth) -> bool {
    expected_current.is_some_and(|current| current != candidate)
}

pub fn usage_auth_sync_outcome(
    auth: UsageAuth,
    source: UsageAuthSyncSource,
    expected_current: Option<&UsageAuth>,
) -> UsageAuthSyncOutcome {
    UsageAuthSyncOutcome {
        auth_changed: usage_auth_changed(expected_current, &auth),
        auth,
        source,
    }
}

#[derive(Debug, Deserialize)]
pub struct JwtExpirationClaims {
    #[serde(default)]
    pub exp: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct JwtAccessTokenClaims {
    #[serde(rename = "https://api.openai.com/auth", default)]
    pub auth: Option<JwtAccessTokenAuthClaims>,
    #[serde(rename = "https://api.openai.com/auth.chatgpt_account_id", default)]
    pub auth_chatgpt_account_id: Option<String>,
    #[serde(default)]
    pub chatgpt_account_id: Option<String>,
}

impl JwtAccessTokenClaims {
    pub fn into_chatgpt_account_id(self) -> Option<String> {
        self.auth
            .and_then(|auth| auth.chatgpt_account_id)
            .or(self.auth_chatgpt_account_id)
            .or(self.chatgpt_account_id)
            .map(|account_id| account_id.trim().to_string())
            .filter(|account_id| !account_id.is_empty())
    }
}

#[derive(Debug, Deserialize)]
pub struct JwtAccessTokenAuthClaims {
    #[serde(default)]
    pub chatgpt_account_id: Option<String>,
}

pub fn parse_jwt_expiration(raw_jwt: &str) -> Result<Option<i64>> {
    let claims: JwtExpirationClaims = parse_jwt_payload(raw_jwt)?;
    Ok(claims.exp)
}

pub fn parse_jwt_chatgpt_account_id(raw_jwt: &str) -> Result<Option<String>> {
    let claims: JwtAccessTokenClaims = parse_jwt_payload(raw_jwt)?;
    Ok(claims.into_chatgpt_account_id())
}

pub fn parse_jwt_payload<T>(raw_jwt: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    prodex_profile_identity::parse_jwt_payload(raw_jwt)
}

#[derive(Deserialize, Serialize)]
pub struct ChatgptRefreshResponse {
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

impl fmt::Debug for ChatgptRefreshResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatgptRefreshResponse")
            .field("id_token", &self.id_token.as_ref().map(|_| "<redacted>"))
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl Zeroize for ChatgptRefreshResponse {
    fn zeroize(&mut self) {
        self.id_token.zeroize();
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}

impl Drop for ChatgptRefreshResponse {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for ChatgptRefreshResponse {}

pub fn apply_chatgpt_refresh(
    auth_json: &mut serde_json::Value,
    mut refreshed: ChatgptRefreshResponse,
    refreshed_at: String,
) -> Result<()> {
    let refreshed_account_id = refreshed
        .access_token
        .as_deref()
        .and_then(|token| parse_jwt_chatgpt_account_id(token).ok().flatten());
    {
        let tokens_object = auth_tokens_object_mut(auth_json)?;
        if let Some(id_token) = refreshed.id_token.take() {
            tokens_object.insert("id_token".to_string(), serde_json::Value::String(id_token));
        }
        if let Some(access_token) = refreshed.access_token.take() {
            tokens_object.insert(
                "access_token".to_string(),
                serde_json::Value::String(access_token),
            );
        }
        if let Some(account_id) = refreshed_account_id {
            tokens_object.insert(
                "account_id".to_string(),
                serde_json::Value::String(account_id),
            );
        }
        if let Some(refresh_token) = refreshed.refresh_token.take() {
            tokens_object.insert(
                "refresh_token".to_string(),
                serde_json::Value::String(refresh_token),
            );
        }
    }

    auth_object_mut(auth_json)?.insert(
        "last_refresh".to_string(),
        serde_json::Value::String(refreshed_at),
    );
    Ok(())
}

fn auth_object_mut(
    auth_json: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>> {
    auth_json
        .as_object_mut()
        .context("stored auth JSON must be an object")
}

fn auth_tokens_object_mut(
    auth_json: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>> {
    let auth_object = auth_object_mut(auth_json)?;
    let tokens_value = auth_object
        .entry("tokens".to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    tokens_value
        .as_object_mut()
        .context("stored auth tokens must be an object")
}

#[cfg(test)]
#[path = "../tests/src/auth.rs"]
mod tests;
