use super::codex_openai_auth::codex_openai_auth_headers_for_home;
#[cfg(test)]
use super::codex_openai_auth::{
    codex_openai_auth_headers, codex_openai_auth_originator,
    codex_openai_auth_user_agent_for_version, parse_codex_cli_version_output,
};
use super::{
    CHATGPT_AUTH_REFRESH_CLIENT_ID, CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS,
    CHATGPT_AUTH_REFRESH_INTERVAL_DAYS, CHATGPT_AUTH_REFRESH_URL,
    CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV, build_upstream_blocking_http_client,
    format_response_body, quota_base_url, rate_limit_reset_credit_consume_url, read_auth_json_text,
    usage_url,
};
use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_body_with_limit,
    read_blocking_response_text_with_limit,
};
use anyhow::{Context, Result, bail};
use chrono::Local;
use codex_config::codex_non_openai_model_provider;
use prodex_core::AppPaths;
use prodex_quota::{AuthSummary, UsageAuth, UsageAuthSyncOutcome, UsageAuthSyncSource};
use prodex_shared_types::StoredAuth;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fmt;
use std::path::Path;
use zeroize::{Zeroize, Zeroizing};

#[derive(Serialize)]
struct ChatgptRefreshRequest {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: String,
}

impl fmt::Debug for ChatgptRefreshRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatgptRefreshRequest")
            .field("client_id", &self.client_id)
            .field("grant_type", &self.grant_type)
            .field("refresh_token", &"<redacted>")
            .finish()
    }
}

impl Drop for ChatgptRefreshRequest {
    fn drop(&mut self) {
        self.refresh_token.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RateLimitResetCreditConsumeOutcome {
    Reset,
    NothingToReset,
    NoCredit,
    AlreadyRedeemed,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RateLimitResetCreditConsumeResponse {
    #[serde(default = "default_rate_limit_reset_credit_consume_outcome")]
    pub(crate) outcome: RateLimitResetCreditConsumeOutcome,
}

#[derive(Debug, Clone)]
pub(crate) struct ChatgptWorkspaceSummary {
    pub(crate) account_id: String,
    pub(crate) name: Option<String>,
}

fn default_rate_limit_reset_credit_consume_outcome() -> RateLimitResetCreditConsumeOutcome {
    RateLimitResetCreditConsumeOutcome::Reset
}

#[derive(Debug, Serialize)]
struct RateLimitResetCreditConsumeRequest<'a> {
    redeem_request_id: &'a str,
}

pub(crate) struct RateLimitResetCreditConsumeFlow<'a> {
    codex_home: &'a Path,
    consume_url: String,
    client: Client,
    auth: UsageAuth,
    upstream_no_proxy: bool,
}

pub(super) struct UsageFetchFlow<'a> {
    codex_home: &'a Path,
    usage_url: String,
    client: Client,
    auth: UsageAuth,
    upstream_no_proxy: bool,
}

impl<'a> RateLimitResetCreditConsumeFlow<'a> {
    pub(crate) fn new_with_proxy_policy(
        codex_home: &'a Path,
        base_url: Option<&str>,
        upstream_no_proxy: bool,
    ) -> Result<Self> {
        let consume_url = rate_limit_reset_credit_consume_url(&quota_base_url(base_url)?);
        let mut auth = read_usage_auth(codex_home)?;
        if usage_auth_needs_proactive_refresh(&auth, Local::now().timestamp())
            && let Ok(outcome) = sync_usage_auth_from_disk_or_refresh_with_proxy_policy(
                codex_home,
                Some(&auth),
                upstream_no_proxy,
            )
        {
            auth = outcome.auth;
        }

        Ok(Self {
            codex_home,
            consume_url,
            client: build_upstream_blocking_http_client(
                "rate-limit reset credit HTTP",
                upstream_no_proxy,
            )?,
            auth,
            upstream_no_proxy,
        })
    }

    pub(crate) fn execute(
        mut self,
        redeem_request_id: &str,
    ) -> Result<RateLimitResetCreditConsumeResponse> {
        let (status, body) = self.send_with_unauthorized_retry(redeem_request_id)?;
        self.ensure_success(status, &body)?;
        serde_json::from_slice(&body).with_context(|| {
            format!(
                "invalid JSON returned by reset-credit backend for {}",
                self.codex_home.display()
            )
        })
    }

    fn send_with_unauthorized_retry(
        &mut self,
        redeem_request_id: &str,
    ) -> Result<(reqwest::StatusCode, Vec<u8>)> {
        let mut response = self.send(redeem_request_id)?;
        if response.0.as_u16() != 401 {
            return Ok(response);
        }

        if self.reload_auth_after_unauthorized() {
            response = self.send(redeem_request_id)?;
            if response.0.as_u16() != 401 {
                return Ok(response);
            }
        }

        if self.refresh_auth_after_unauthorized() {
            response = self.send(redeem_request_id)?;
        }

        Ok(response)
    }

    fn send(&self, redeem_request_id: &str) -> Result<(reqwest::StatusCode, Vec<u8>)> {
        send_rate_limit_reset_credit_consume_request(
            &self.client,
            self.codex_home,
            &self.consume_url,
            &self.auth,
            redeem_request_id,
        )
    }

    fn reload_auth_after_unauthorized(&mut self) -> bool {
        let Ok(latest) = read_usage_auth(self.codex_home) else {
            return false;
        };
        if !usage_auth_changed(Some(&self.auth), &latest) {
            return false;
        }

        self.auth = latest;
        true
    }

    fn refresh_auth_after_unauthorized(&mut self) -> bool {
        let Ok(outcome) = refresh_usage_auth_from_disk_with_proxy_policy(
            self.codex_home,
            Some(&self.auth),
            self.upstream_no_proxy,
        ) else {
            return false;
        };

        self.auth = outcome.auth;
        true
    }

    fn ensure_success(&self, status: reqwest::StatusCode, body: &[u8]) -> Result<()> {
        if status.is_success() {
            return Ok(());
        }

        let body_text = format_response_body(body);
        if body_text.is_empty() {
            bail!(
                "request failed (HTTP {}) to {}",
                status.as_u16(),
                self.consume_url
            );
        }
        bail!(
            "request failed (HTTP {}) to {}: {}",
            status.as_u16(),
            self.consume_url,
            body_text
        );
    }
}

impl<'a> UsageFetchFlow<'a> {
    pub(super) fn new(codex_home: &'a Path, base_url: Option<&str>) -> Result<Self> {
        Self::new_with_proxy_policy(codex_home, base_url, false)
    }

    pub(super) fn new_with_proxy_policy(
        codex_home: &'a Path,
        base_url: Option<&str>,
        upstream_no_proxy: bool,
    ) -> Result<Self> {
        let usage_url = usage_url(&quota_base_url(base_url)?);
        let mut auth = read_usage_auth(codex_home)?;
        if usage_auth_needs_proactive_refresh(&auth, Local::now().timestamp())
            && let Ok(outcome) = sync_usage_auth_from_disk_or_refresh_with_proxy_policy(
                codex_home,
                Some(&auth),
                upstream_no_proxy,
            )
        {
            auth = outcome.auth;
        }

        Ok(Self {
            codex_home,
            usage_url,
            client: build_upstream_blocking_http_client("quota HTTP", upstream_no_proxy)?,
            auth,
            upstream_no_proxy,
        })
    }

    pub(super) fn execute(mut self) -> Result<serde_json::Value> {
        let (status, body) = self.send_with_unauthorized_retry()?;
        self.ensure_success(status, &body)?;
        serde_json::from_slice(&body).with_context(|| {
            format!(
                "invalid JSON returned by quota backend for {}",
                self.codex_home.display()
            )
        })
    }

    fn send_with_unauthorized_retry(&mut self) -> Result<(reqwest::StatusCode, Vec<u8>)> {
        let mut response = self.send()?;
        if response.0.as_u16() != 401 {
            return Ok(response);
        }

        if self.reload_auth_after_unauthorized() {
            response = self.send()?;
            if response.0.as_u16() != 401 {
                return Ok(response);
            }
        }

        if self.refresh_auth_after_unauthorized() {
            response = self.send()?;
        }

        Ok(response)
    }

    fn send(&self) -> Result<(reqwest::StatusCode, Vec<u8>)> {
        send_usage_request(&self.client, self.codex_home, &self.usage_url, &self.auth)
    }

    fn reload_auth_after_unauthorized(&mut self) -> bool {
        let Ok(latest) = read_usage_auth(self.codex_home) else {
            return false;
        };
        if !usage_auth_changed(Some(&self.auth), &latest) {
            return false;
        }

        self.auth = latest;
        true
    }

    fn refresh_auth_after_unauthorized(&mut self) -> bool {
        let Ok(outcome) = refresh_usage_auth_from_disk_with_proxy_policy(
            self.codex_home,
            Some(&self.auth),
            self.upstream_no_proxy,
        ) else {
            return false;
        };

        self.auth = outcome.auth;
        true
    }

    fn ensure_success(&self, status: reqwest::StatusCode, body: &[u8]) -> Result<()> {
        if status.is_success() {
            return Ok(());
        }

        let body_text = format_response_body(body);
        if body_text.is_empty() {
            bail!(
                "request failed (HTTP {}) to {}",
                status.as_u16(),
                self.usage_url
            );
        }
        bail!(
            "request failed (HTTP {}) to {}: {}",
            status.as_u16(),
            self.usage_url,
            body_text
        );
    }
}

pub(crate) fn read_auth_summary(codex_home: &Path) -> AuthSummary {
    if let Some(model_provider) = codex_non_openai_model_provider(codex_home, None) {
        return AuthSummary {
            label: format!("model-provider:{}", model_provider.provider_id),
            quota_compatible: false,
        };
    }
    prodex_quota::auth_summary_from_auth_text_result(read_auth_json_text(codex_home))
}

pub(crate) fn read_usage_auth(codex_home: &Path) -> Result<UsageAuth> {
    let auth_location = secret_store::auth_json_path(codex_home);
    let Some(content) = read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_location.display()))?
    else {
        bail!(
            "auth secret not found at {}. Run `codex login` first.",
            auth_location.display()
        );
    };
    let content = Zeroizing::new(content);
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_location.display()))?;
    prodex_quota::usage_auth_from_stored_auth(&stored_auth)
}

pub(crate) fn read_profile_workspace_from_auth(
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<Option<ChatgptWorkspaceSummary>> {
    let accounts_url = chatgpt_accounts_check_url(&quota_base_url(base_url)?);
    let auth = read_usage_auth(codex_home)?;
    let Some(account_id) = auth
        .account_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
    else {
        return Ok(None);
    };
    let fallback = || {
        Some(ChatgptWorkspaceSummary {
            account_id: account_id.clone(),
            name: None,
        })
    };
    let Ok(client) = build_upstream_blocking_http_client("accounts HTTP", false) else {
        return Ok(fallback());
    };
    let mut request = codex_openai_auth_headers_for_home(client.get(&accounts_url), codex_home)
        .header("Authorization", format!("Bearer {}", auth.access_token));
    request = request.header("ChatGPT-Account-Id", &account_id);
    let Ok(response) = request.send() else {
        return Ok(fallback());
    };
    let status = response.status();
    let Ok(body) = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read accounts response body",
    ) else {
        return Ok(fallback());
    };
    if !status.is_success() {
        return Ok(fallback());
    }
    let Ok(accounts) = serde_json::from_slice::<ChatgptAccountsResponse>(&body) else {
        return Ok(fallback());
    };
    Ok(accounts
        .accounts()
        .into_iter()
        .find(|account| account.id == account_id.as_str())
        .map(|account| ChatgptWorkspaceSummary {
            name: account.display_name(),
            account_id: account.id,
        })
        .or_else(fallback))
}

fn chatgpt_accounts_check_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/accounts/check")
    } else {
        format!("{base_url}/api/codex/accounts/check")
    }
}

#[derive(Debug, Deserialize)]
struct ChatgptAccountsResponse {
    #[serde(default)]
    accounts: ChatgptAccounts,
    #[serde(default)]
    account_ordering: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ChatgptAccounts {
    List(Vec<ChatgptAccountEntry>),
    Map(std::collections::HashMap<String, ChatgptAccountWrapper>),
}

impl Default for ChatgptAccounts {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}

#[derive(Debug, Deserialize)]
struct ChatgptAccountWrapper {
    account: ChatgptAccountInfo,
}

#[derive(Debug, Deserialize)]
struct ChatgptAccountInfo {
    account_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    structure: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatgptAccountEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    structure: Option<String>,
}

impl ChatgptAccountsResponse {
    fn accounts(self) -> Vec<ChatgptAccountEntry> {
        match self.accounts {
            ChatgptAccounts::List(accounts) => accounts,
            ChatgptAccounts::Map(mut accounts) => {
                let ordered = self
                    .account_ordering
                    .iter()
                    .filter_map(|account_id| accounts.remove(account_id))
                    .collect::<Vec<_>>();
                let entries = if ordered.is_empty() {
                    accounts.into_values().collect()
                } else {
                    ordered
                };
                entries
                    .into_iter()
                    .filter_map(|entry| {
                        let account = entry.account;
                        Some(ChatgptAccountEntry {
                            id: account.account_id?,
                            name: account.name,
                            structure: account.structure,
                        })
                    })
                    .collect()
            }
        }
    }
}

impl ChatgptAccountEntry {
    fn display_name(&self) -> Option<String> {
        self.name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                self.structure
                    .as_deref()
                    .map(str::trim)
                    .filter(|structure| structure.eq_ignore_ascii_case("personal"))
                    .map(|_| "Personal".to_string())
            })
    }
}

pub(crate) fn usage_auth_needs_proactive_refresh(auth: &UsageAuth, now: i64) -> bool {
    prodex_quota::usage_auth_needs_proactive_refresh_with_policy(
        auth,
        now,
        CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS,
        CHATGPT_AUTH_REFRESH_INTERVAL_DAYS,
    )
}

pub(crate) fn usage_auth_sync_source_label(source: UsageAuthSyncSource) -> &'static str {
    prodex_quota::usage_auth_sync_source_label(source)
}

pub(crate) fn sync_usage_auth_from_disk_or_refresh_with_proxy_policy(
    codex_home: &Path,
    expected_current: Option<&UsageAuth>,
    upstream_no_proxy: bool,
) -> Result<UsageAuthSyncOutcome> {
    let latest = read_usage_auth(codex_home)?;
    if usage_auth_changed(expected_current, &latest) {
        return Ok(usage_auth_sync_outcome(
            latest,
            UsageAuthSyncSource::Reloaded,
            expected_current,
        ));
    }

    refresh_usage_auth_from_disk_with_proxy_policy(codex_home, expected_current, upstream_no_proxy)
}

fn refresh_usage_auth_from_disk_with_proxy_policy(
    codex_home: &Path,
    expected_current: Option<&UsageAuth>,
    upstream_no_proxy: bool,
) -> Result<UsageAuthSyncOutcome> {
    let latest = read_usage_auth(codex_home)?;
    let refresh_token = latest
        .refresh_token
        .as_deref()
        .context("refresh token not found in the stored auth secret")?;
    refresh_usage_auth_file(codex_home, refresh_token, upstream_no_proxy)?;

    let refreshed = read_usage_auth(codex_home)?;
    Ok(usage_auth_sync_outcome(
        refreshed,
        UsageAuthSyncSource::Refreshed,
        expected_current,
    ))
}

fn send_usage_request(
    client: &Client,
    codex_home: &Path,
    usage_url: &str,
    auth: &UsageAuth,
) -> Result<(reqwest::StatusCode, Vec<u8>)> {
    let mut request = codex_openai_auth_headers_for_home(client.get(usage_url), codex_home)
        .header("Authorization", format!("Bearer {}", auth.access_token));

    if let Some(account_id) = auth.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .with_context(|| format!("failed to request quota endpoint {}", usage_url))?;
    let status = response.status();
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read quota response body",
    )?;
    Ok((status, body))
}

fn send_rate_limit_reset_credit_consume_request(
    client: &Client,
    codex_home: &Path,
    consume_url: &str,
    auth: &UsageAuth,
    redeem_request_id: &str,
) -> Result<(reqwest::StatusCode, Vec<u8>)> {
    let mut request = codex_openai_auth_headers_for_home(client.post(consume_url), codex_home)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&RateLimitResetCreditConsumeRequest { redeem_request_id });

    if let Some(account_id) = auth.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .with_context(|| format!("failed to request reset-credit endpoint {consume_url}"))?;
    let status = response.status();
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read reset-credit response body",
    )?;
    Ok((status, body))
}

fn refresh_usage_auth_endpoint() -> String {
    env::var(CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV)
        .unwrap_or_else(|_| CHATGPT_AUTH_REFRESH_URL.to_string())
}

fn refresh_usage_auth_file(
    codex_home: &Path,
    refresh_token: &str,
    upstream_no_proxy: bool,
) -> Result<()> {
    let mut auth_json = read_auth_json_value(codex_home)?;
    let refreshed =
        request_chatgpt_auth_refresh_with_lease(codex_home, refresh_token, upstream_no_proxy)?;
    apply_chatgpt_refresh(&mut auth_json, refreshed)?;
    write_auth_json_value(codex_home, &auth_json)
}

fn usage_auth_changed(expected_current: Option<&UsageAuth>, candidate: &UsageAuth) -> bool {
    prodex_quota::usage_auth_changed(expected_current, candidate)
}

fn usage_auth_sync_outcome(
    auth: UsageAuth,
    source: UsageAuthSyncSource,
    expected_current: Option<&UsageAuth>,
) -> UsageAuthSyncOutcome {
    prodex_quota::usage_auth_sync_outcome(auth, source, expected_current)
}

fn read_auth_json_value(codex_home: &Path) -> Result<serde_json::Value> {
    let auth_location = secret_store::auth_json_path(codex_home);
    let Some(content) = read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_location.display()))?
    else {
        bail!(
            "auth secret not found at {}. Run `codex login` first.",
            auth_location.display()
        );
    };
    let content = Zeroizing::new(content);
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_location.display()))
}

fn request_chatgpt_auth_refresh_with_lease(
    codex_home: &Path,
    refresh_token: &str,
    upstream_no_proxy: bool,
) -> Result<prodex_quota::ChatgptRefreshResponse> {
    let coordinator = usage_auth_refresh_lease_coordinator()?;
    match coordinator
        .acquire(refresh_token.as_bytes())
        .map_err(anyhow::Error::new)?
    {
        secret_store::RefreshLeaseDecision::Follower { result_json } => {
            let result_json = Zeroizing::new(result_json);
            serde_json::from_str(&result_json).context("failed to parse shared auth refresh JSON")
        }
        secret_store::RefreshLeaseDecision::Owner(owner) => {
            let refreshed =
                request_chatgpt_auth_refresh_direct(codex_home, refresh_token, upstream_no_proxy)?;
            let result_json = Zeroizing::new(
                serde_json::to_string(&refreshed)
                    .context("failed to serialize shared auth refresh JSON")?,
            );
            let _ = owner.commit_result(&result_json);
            Ok(refreshed)
        }
        secret_store::RefreshLeaseDecision::Bypass { .. } => {
            request_chatgpt_auth_refresh_direct(codex_home, refresh_token, upstream_no_proxy)
        }
    }
}

fn usage_auth_refresh_lease_coordinator() -> Result<secret_store::RefreshLeaseCoordinator> {
    Ok(secret_store::RefreshLeaseCoordinator::new(
        AppPaths::discover()?.root.join("auth-refresh-leases"),
    )
    .with_namespace("quota-auth-refresh"))
}

fn request_chatgpt_auth_refresh_direct(
    codex_home: &Path,
    refresh_token: &str,
    upstream_no_proxy: bool,
) -> Result<prodex_quota::ChatgptRefreshResponse> {
    let client = build_upstream_blocking_http_client("auth refresh HTTP", upstream_no_proxy)?;
    let response =
        codex_openai_auth_headers_for_home(client.post(refresh_usage_auth_endpoint()), codex_home)
            .header("Content-Type", "application/json")
            .json(&ChatgptRefreshRequest {
                client_id: CHATGPT_AUTH_REFRESH_CLIENT_ID,
                grant_type: "refresh_token",
                refresh_token: refresh_token.to_string(),
            })
            .send()
            .context("failed to request ChatGPT auth refresh")?;
    let status = response.status();
    let body = Zeroizing::new(read_blocking_response_text_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read auth refresh body",
    )?);
    if !status.is_success() {
        bail!(
            "failed to refresh ChatGPT auth (HTTP {}): {}",
            status.as_u16(),
            body.as_str()
        );
    }

    serde_json::from_str(&body).context("failed to parse auth refresh JSON")
}

fn apply_chatgpt_refresh(
    auth_json: &mut serde_json::Value,
    refreshed: prodex_quota::ChatgptRefreshResponse,
) -> Result<()> {
    prodex_quota::apply_chatgpt_refresh(auth_json, refreshed, chrono::Utc::now().to_rfc3339())
}

fn write_auth_json_value(codex_home: &Path, auth_json: &serde_json::Value) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(
            &secret_store::auth_json_location(codex_home),
            serde_json::to_string_pretty(auth_json).context("failed to serialize auth JSON")?,
        )
        .map_err(anyhow::Error::new)
}

#[cfg(test)]
#[path = "../../tests/src/quota_support/auth.rs"]
mod tests;
