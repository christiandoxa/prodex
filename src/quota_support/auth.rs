use super::*;

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

fn auth_summary_from_auth_text_result(
    result: std::result::Result<Option<String>, anyhow::Error>,
) -> AuthSummary {
    match result {
        Ok(Some(content)) => auth_summary_from_auth_text(&content),
        Ok(None) => AuthSummaryKind::MissingAuth.into_summary(),
        Err(_) => AuthSummaryKind::UnreadableAuth.into_summary(),
    }
}

fn auth_summary_from_auth_text(content: &str) -> AuthSummary {
    let stored_auth: StoredAuth = match serde_json::from_str(content) {
        Ok(auth) => auth,
        Err(_) => return AuthSummaryKind::InvalidAuth.into_summary(),
    };
    auth_summary_from_stored_auth(stored_auth)
}

fn auth_summary_from_stored_auth(stored_auth: StoredAuth) -> AuthSummary {
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

    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
        return AuthSummaryKind::ApiKey.into_summary();
    }

    AuthSummaryKind::Other(
        stored_auth
            .auth_mode
            .unwrap_or_else(|| "auth-present".to_string()),
    )
    .into_summary()
}

#[derive(Debug, Deserialize)]
struct JwtExpirationClaims {
    #[serde(default)]
    exp: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct JwtAccessTokenClaims {
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<JwtAccessTokenAuthClaims>,
    #[serde(rename = "https://api.openai.com/auth.chatgpt_account_id", default)]
    auth_chatgpt_account_id: Option<String>,
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

impl JwtAccessTokenClaims {
    fn into_chatgpt_account_id(self) -> Option<String> {
        self.auth
            .and_then(|auth| auth.chatgpt_account_id)
            .or(self.auth_chatgpt_account_id)
            .or(self.chatgpt_account_id)
            .map(|account_id| account_id.trim().to_string())
            .filter(|account_id| !account_id.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct JwtAccessTokenAuthClaims {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatgptRefreshRequest<'a> {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatgptRefreshResponse {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn build_usage_http_client(context_label: &'static str) -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS))
        .build()
        .with_context(|| format!("failed to build {context_label} client"))
}

pub(super) struct UsageFetchFlow<'a> {
    codex_home: &'a Path,
    usage_url: String,
    client: Client,
    auth: UsageAuth,
}

impl<'a> UsageFetchFlow<'a> {
    pub(super) fn new(codex_home: &'a Path, base_url: Option<&str>) -> Result<Self> {
        let mut auth = read_usage_auth(codex_home)?;
        if usage_auth_needs_proactive_refresh(&auth, Local::now().timestamp())
            && let Ok(outcome) = sync_usage_auth_from_disk_or_refresh(codex_home, Some(&auth))
        {
            auth = outcome.auth;
        }

        Ok(Self {
            codex_home,
            usage_url: usage_url(&quota_base_url(base_url)),
            client: build_usage_http_client("quota HTTP")?,
            auth,
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
        let initial = self.send()?;
        if initial.0.as_u16() != 401 {
            return Ok(initial);
        }

        if let Some(retried) = self.retry_after_unauthorized()? {
            return Ok(retried);
        }

        Ok(initial)
    }

    fn send(&self) -> Result<(reqwest::StatusCode, Vec<u8>)> {
        send_usage_request(&self.client, &self.usage_url, &self.auth)
    }

    fn retry_after_unauthorized(&mut self) -> Result<Option<(reqwest::StatusCode, Vec<u8>)>> {
        let Ok(outcome) = sync_usage_auth_from_disk_or_refresh(self.codex_home, Some(&self.auth))
        else {
            return Ok(None);
        };

        self.auth = outcome.auth;
        self.send().map(Some)
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
    auth_summary_from_auth_text_result(read_auth_json_text(codex_home))
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

    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());
    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
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

pub(crate) fn usage_auth_needs_proactive_refresh(auth: &UsageAuth, now: i64) -> bool {
    if let Some(expires_at) = auth.expires_at {
        return expires_at <= now.saturating_add(CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS);
    }

    auth.last_refresh.is_some_and(|last_refresh| {
        now - last_refresh >= CHATGPT_AUTH_REFRESH_INTERVAL_DAYS * 86_400
    })
}

pub(crate) fn usage_auth_sync_source_label(source: UsageAuthSyncSource) -> &'static str {
    match source {
        UsageAuthSyncSource::Reloaded => "reloaded",
        UsageAuthSyncSource::Refreshed => "refreshed",
    }
}

pub(crate) fn sync_usage_auth_from_disk_or_refresh(
    codex_home: &Path,
    expected_current: Option<&UsageAuth>,
) -> Result<UsageAuthSyncOutcome> {
    let latest = read_usage_auth(codex_home)?;
    if usage_auth_changed(expected_current, &latest) {
        return Ok(usage_auth_sync_outcome(
            latest,
            UsageAuthSyncSource::Reloaded,
            expected_current,
        ));
    }

    let refresh_token = latest
        .refresh_token
        .as_deref()
        .context("refresh token not found in the stored auth secret")?;
    refresh_usage_auth_file(codex_home, refresh_token)?;

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

fn parse_jwt_expiration(raw_jwt: &str) -> Result<Option<i64>> {
    let claims: JwtExpirationClaims = parse_jwt_payload(raw_jwt)?;
    Ok(claims.exp)
}

fn parse_jwt_chatgpt_account_id(raw_jwt: &str) -> Result<Option<String>> {
    let claims: JwtAccessTokenClaims = parse_jwt_payload(raw_jwt)?;
    Ok(claims.into_chatgpt_account_id())
}

fn parse_jwt_payload<T>(raw_jwt: &str) -> Result<T>
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

fn refresh_usage_auth_endpoint() -> String {
    env::var(CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV)
        .unwrap_or_else(|_| CHATGPT_AUTH_REFRESH_URL.to_string())
}

fn refresh_usage_auth_file(codex_home: &Path, refresh_token: &str) -> Result<()> {
    let mut auth_json = read_auth_json_value(codex_home)?;
    let refreshed = request_chatgpt_auth_refresh(refresh_token)?;
    apply_chatgpt_refresh(&mut auth_json, refreshed)?;
    write_auth_json_value(codex_home, &auth_json)
}

fn usage_auth_changed(expected_current: Option<&UsageAuth>, candidate: &UsageAuth) -> bool {
    expected_current.is_some_and(|current| current != candidate)
}

fn usage_auth_sync_outcome(
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

fn request_chatgpt_auth_refresh(refresh_token: &str) -> Result<ChatgptRefreshResponse> {
    let client = build_usage_http_client("auth refresh HTTP")?;
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
    refreshed: ChatgptRefreshResponse,
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
        serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
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

fn write_auth_json_value(codex_home: &Path, auth_json: &serde_json::Value) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(
            &secret_store::auth_json_location(codex_home),
            serde_json::to_string_pretty(auth_json).context("failed to serialize auth JSON")?,
        )
        .map_err(anyhow::Error::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_openai_model_provider_disables_quota_summary() {
        let root = temp_dir("non-openai-model-provider");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            "model_provider = 'amazon-bedrock'\n",
        )
        .unwrap();
        fs::write(
            secret_store::auth_json_path(&root),
            r#"{"tokens":{"access_token":"test-token"}}"#,
        )
        .unwrap();

        let summary = read_auth_summary(&root);

        assert_eq!(summary.label, "model-provider:amazon-bedrock");
        assert!(!summary.quota_compatible);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "prodex-auth-summary-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        dir
    }
}
