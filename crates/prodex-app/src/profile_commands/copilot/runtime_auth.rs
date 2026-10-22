use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::fmt;
use std::time::Duration;

use super::resolve_copilot_account_token;
use crate::{
    QUOTA_HTTP_CONNECT_TIMEOUT_MS, QUOTA_HTTP_READ_TIMEOUT_MS,
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, format_response_body,
    read_blocking_response_body_with_limit,
};
use prodex_profile_export::{copilot_user_api_origin, default_copilot_models_api_url};

pub(super) const COPILOT_RUNTIME_INTEGRATION_ID: &str = "copilot-developer-cli";
pub(super) const COPILOT_RUNTIME_API_VERSION: &str = "2025-04-01";
const COPILOT_RUNTIME_USER_AGENT: &str = "copilot/1.0.65 (client/github/cli)";

#[derive(Clone)]
pub(crate) struct CopilotRuntimeApiAuth {
    pub(crate) api_key: String,
    pub(crate) model_catalog: Vec<serde_json::Value>,
}

impl fmt::Debug for CopilotRuntimeApiAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CopilotRuntimeApiAuth")
            .field("api_key", &"<redacted>")
            .field("model_catalog", &redacted_len(self.model_catalog.len()))
            .finish()
    }
}

fn redacted_len(len: usize) -> String {
    format!("<redacted:{len}>")
}

pub(crate) fn resolve_copilot_runtime_api_auth(
    host: &str,
    login: &str,
) -> Result<CopilotRuntimeApiAuth> {
    let access_token = resolve_copilot_account_token(host, login)?;
    refresh_copilot_runtime_api_auth(host, &access_token)
}

fn refresh_copilot_runtime_api_auth(
    host: &str,
    access_token: &str,
) -> Result<CopilotRuntimeApiAuth> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build Copilot runtime auth HTTP client")?;
    let token_url = format!(
        "{}/copilot_internal/v2/token",
        copilot_user_api_origin(host)?
    );
    let api_url = default_copilot_models_api_url(host);
    refresh_copilot_runtime_api_auth_with_urls(&client, &token_url, &api_url, access_token)
}

pub(super) fn refresh_copilot_runtime_api_auth_with_urls(
    client: &Client,
    token_url: &str,
    api_url: &str,
    access_token: &str,
) -> Result<CopilotRuntimeApiAuth> {
    // Copilot CLI >= 1.0.65 no longer exchanges the GitHub OAuth token through
    // /copilot_internal/v2/token.  It sends the OAuth token directly as the
    // Bearer credential to the Copilot API.  Prefer that path first so a removed
    // or blocked legacy exchange endpoint cannot prevent launch.
    match fetch_copilot_runtime_models_with_oauth(client, api_url, access_token) {
        Ok(Some(auth)) => Ok(auth),
        direct_result => {
            match fetch_copilot_runtime_legacy_token(client, token_url, access_token) {
                Ok(auth) => Ok(auth),
                Err(legacy_err) => {
                    if let Err(oauth_err) = direct_result {
                        bail!(
                            "Copilot runtime auth failed: direct OAuth request failed ({:#}); legacy token exchange failed ({:#})",
                            oauth_err,
                            legacy_err
                        );
                    }
                    Ok(CopilotRuntimeApiAuth {
                        api_key: access_token.to_string(),
                        model_catalog: Vec::new(),
                    })
                }
            }
        }
    }
}

fn fetch_copilot_runtime_models_with_oauth(
    client: &Client,
    api_url: &str,
    access_token: &str,
) -> Result<Option<CopilotRuntimeApiAuth>> {
    let models_url = format!("{}/models", api_url.trim_end_matches('/'));
    let Ok(models_resp) = client
        .get(&models_url)
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Copilot-Integration-Id", COPILOT_RUNTIME_INTEGRATION_ID)
        .header("x-github-api-version", COPILOT_RUNTIME_API_VERSION)
        .header("User-Agent", COPILOT_RUNTIME_USER_AGENT)
        .send()
    else {
        return Ok(None);
    };
    let models_status = models_resp.status();
    let Ok(models_body) = read_blocking_response_body_with_limit(
        models_resp,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        &format!("failed to read {models_url}"),
    ) else {
        return Ok(None);
    };
    if !models_status.is_success() {
        if matches!(models_status.as_u16(), 401 | 403) {
            bail!(
                "Copilot models endpoint rejected direct OAuth with HTTP {}",
                models_status.as_u16()
            );
        }
        return Ok(None);
    }
    let Ok(models_value) = serde_json::from_slice(&models_body) else {
        return Ok(None);
    };
    let Ok(model_catalog) = copilot_runtime_model_catalog_from_token(&models_value) else {
        return Ok(None);
    };
    Ok(Some(CopilotRuntimeApiAuth {
        api_key: access_token.to_string(),
        model_catalog,
    }))
}

fn fetch_copilot_runtime_legacy_token(
    client: &Client,
    token_url: &str,
    access_token: &str,
) -> Result<CopilotRuntimeApiAuth> {
    let response = client
        .get(token_url)
        .header("Authorization", format!("token {access_token}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Editor-Version", "vscode/1.85.1")
        .header("Editor-Plugin-Version", "copilot/1.155.0")
        .header("User-Agent", "GithubCopilot/1.155.0")
        .send()
        .with_context(|| format!("failed to query {}", token_url))?;
    let status = response.status();
    if status.is_success() {
        let body = read_blocking_response_body_with_limit(
            response,
            RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
            &format!("failed to read {}", token_url),
        )?;
        let value: serde_json::Value = serde_json::from_slice(&body)
            .with_context(|| format!("failed to parse {token_url}"))?;
        let api_key = value
            .get("token")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .context("Copilot runtime token response did not contain token")?;
        let model_catalog = copilot_runtime_model_catalog_from_token(&value).unwrap_or_default();
        return Ok(CopilotRuntimeApiAuth {
            api_key,
            model_catalog,
        });
    }
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        &format!("failed to read {}", token_url),
    )?;
    let body_text = format_response_body(&body);
    if body_text.is_empty() {
        bail!(
            "Copilot runtime token refresh failed (HTTP {}) at {}",
            status.as_u16(),
            token_url
        );
    }
    bail!(
        "Copilot runtime token refresh failed (HTTP {}) at {}: {}",
        status.as_u16(),
        token_url,
        body_text
    )
}

pub(super) fn copilot_runtime_model_catalog_from_token(
    value: &serde_json::Value,
) -> Result<Vec<serde_json::Value>> {
    prodex_provider_core::normalize_copilot_model_catalog(value).map_err(anyhow::Error::new)
}
