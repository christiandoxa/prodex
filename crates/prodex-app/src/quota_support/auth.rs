use super::*;

#[derive(Debug, Serialize)]
struct ChatgptRefreshRequest<'a> {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: &'a str,
}

pub(super) struct UsageFetchFlow<'a> {
    codex_home: &'a Path,
    usage_url: String,
    client: Client,
    auth: UsageAuth,
    upstream_no_proxy: bool,
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
            usage_url: usage_url(&quota_base_url(base_url)),
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
        send_usage_request(&self.client, &self.usage_url, &self.auth)
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
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_location.display()))?;
    prodex_quota::usage_auth_from_stored_auth(&stored_auth)
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
    usage_url: &str,
    auth: &UsageAuth,
) -> Result<(reqwest::StatusCode, Vec<u8>)> {
    let mut request = client
        .get(usage_url)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("User-Agent", "codex-cli");

    if let Some(account_id) = auth.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .with_context(|| format!("failed to request quota endpoint {}", usage_url))?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read quota response body")?
        .to_vec();
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
    let refreshed = request_chatgpt_auth_refresh(refresh_token, upstream_no_proxy)?;
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
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_location.display()))
}

fn request_chatgpt_auth_refresh(
    refresh_token: &str,
    upstream_no_proxy: bool,
) -> Result<prodex_quota::ChatgptRefreshResponse> {
    let client = build_upstream_blocking_http_client("auth refresh HTTP", upstream_no_proxy)?;
    let response = client
        .post(refresh_usage_auth_endpoint())
        .header("Content-Type", "application/json")
        .header("User-Agent", "codex-cli")
        .json(&ChatgptRefreshRequest {
            client_id: CHATGPT_AUTH_REFRESH_CLIENT_ID,
            grant_type: "refresh_token",
            refresh_token,
        })
        .send()
        .context("failed to request ChatGPT auth refresh")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read auth refresh body")?;
    if !status.is_success() {
        bail!(
            "failed to refresh ChatGPT auth (HTTP {}): {}",
            status.as_u16(),
            body
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
