use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::{Deserialize, Serialize};

use super::{AuthSummary, StoredAuth, UsageAuth, UsageAuthSyncOutcome, UsageAuthSyncSource};

const CHATGPT_AUTH_REFRESH_INTERVAL_DAYS: i64 = 8;
const CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS: i64 = if cfg!(test) { 30 } else { 5 * 60 };

#[derive(Debug)]
enum AuthSummaryKind {
    UnreadableAuth,
    MissingAuth,
    InvalidAuth,
    Chatgpt,
    ApiKey,
    Other(String),
}

impl AuthSummaryKind {
    fn into_summary(self) -> AuthSummary {
        match self {
            Self::UnreadableAuth => AuthSummary {
                label: "unreadable-auth".to_string(),
                quota_compatible: false,
            },
            Self::MissingAuth => AuthSummary {
                label: "no-auth".to_string(),
                quota_compatible: false,
            },
            Self::InvalidAuth => AuthSummary {
                label: "invalid-auth".to_string(),
                quota_compatible: false,
            },
            Self::Chatgpt => AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            Self::ApiKey => AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            Self::Other(label) => AuthSummary {
                label,
                quota_compatible: false,
            },
        }
    }
}

pub fn auth_summary_from_auth_text_result<E>(
    result: std::result::Result<Option<String>, E>,
) -> AuthSummary {
    match result {
        Ok(Some(content)) => auth_summary_from_auth_text(&content),
        Ok(None) => AuthSummaryKind::MissingAuth.into_summary(),
        Err(_) => AuthSummaryKind::UnreadableAuth.into_summary(),
    }
}

pub fn auth_summary_from_auth_text(content: &str) -> AuthSummary {
    let stored_auth: StoredAuth = match serde_json::from_str(content) {
        Ok(auth) => auth,
        Err(_) => return AuthSummaryKind::InvalidAuth.into_summary(),
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

    if has_chatgpt_token {
        return AuthSummaryKind::Chatgpt.into_summary();
    }

    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key" | "apikey")) || has_api_key {
        return AuthSummaryKind::ApiKey.into_summary();
    }

    AuthSummaryKind::Other(
        stored_auth
            .auth_mode
            .clone()
            .unwrap_or_else(|| "auth-present".to_string()),
    )
    .into_summary()
}

pub fn usage_auth_from_auth_text(content: &str) -> Result<UsageAuth> {
    let stored_auth: StoredAuth =
        serde_json::from_str(content).context("failed to parse stored auth JSON")?;
    usage_auth_from_stored_auth(&stored_auth)
}

pub fn usage_auth_from_stored_auth(stored_auth: &StoredAuth) -> Result<UsageAuth> {
    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());
    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key" | "apikey")) || has_api_key {
        bail!("quota endpoint requires a ChatGPT access token. Run `codex login` first.");
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
    if let Some(expires_at) = auth.expires_at {
        return expires_at <= now.saturating_add(expiry_skew_seconds);
    }

    auth.last_refresh
        .is_some_and(|last_refresh| now - last_refresh >= refresh_interval_days * 86_400)
}

pub fn usage_auth_sync_source_label(source: UsageAuthSyncSource) -> &'static str {
    match source {
        UsageAuthSyncSource::Reloaded => "reloaded",
        UsageAuthSyncSource::Refreshed => "refreshed",
    }
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
    let mut parts = raw_jwt.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(payload), Some(signature))
            if !header.is_empty() && !payload.is_empty() && !signature.is_empty() =>
        {
            (header, payload, signature)
        }
        _ => bail!("invalid JWT format"),
    };

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload_b64))
        .context("failed to decode JWT payload")?;
    serde_json::from_slice(&payload_bytes).context("failed to parse JWT payload JSON")
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatgptRefreshResponse {
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

pub fn apply_chatgpt_refresh(
    auth_json: &mut serde_json::Value,
    refreshed: ChatgptRefreshResponse,
    refreshed_at: String,
) -> Result<()> {
    let ChatgptRefreshResponse {
        id_token,
        access_token,
        refresh_token,
    } = refreshed;
    let refreshed_account_id = access_token
        .as_deref()
        .and_then(|token| parse_jwt_chatgpt_account_id(token).ok().flatten());
    {
        let tokens_object = auth_tokens_object_mut(auth_json)?;
        if let Some(id_token) = id_token {
            tokens_object.insert("id_token".to_string(), serde_json::Value::String(id_token));
        }
        if let Some(access_token) = access_token {
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
        if let Some(refresh_token) = refresh_token {
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
