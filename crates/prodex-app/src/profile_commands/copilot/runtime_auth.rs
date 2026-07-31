use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::collections::BTreeSet;
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
        Ok(auth) => Ok(auth),
        Err(oauth_err) => {
            match fetch_copilot_runtime_legacy_token(client, token_url, access_token) {
                Ok(auth) => Ok(auth),
                Err(legacy_err) => {
                    bail!(
                        "Copilot runtime auth failed: direct OAuth request failed ({:#}); legacy token exchange failed ({:#})",
                        oauth_err,
                        legacy_err
                    )
                }
            }
        }
    }
}

fn fetch_copilot_runtime_models_with_oauth(
    client: &Client,
    api_url: &str,
    access_token: &str,
) -> Result<CopilotRuntimeApiAuth> {
    let models_url = format!("{}/models", api_url.trim_end_matches('/'));
    let models_resp = client
        .get(&models_url)
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Copilot-Integration-Id", COPILOT_RUNTIME_INTEGRATION_ID)
        .header("x-github-api-version", COPILOT_RUNTIME_API_VERSION)
        .header("User-Agent", COPILOT_RUNTIME_USER_AGENT)
        .send()
        .with_context(|| format!("failed to query {models_url}"))?;
    let models_status = models_resp.status();
    let models_body = read_blocking_response_body_with_limit(
        models_resp,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        &format!("failed to read {models_url}"),
    )?;
    if !models_status.is_success() {
        let body_text = format_response_body(&models_body);
        if body_text.is_empty() {
            bail!(
                "models endpoint returned HTTP {} at {}",
                models_status.as_u16(),
                models_url
            );
        }
        bail!(
            "models endpoint returned HTTP {} at {}: {}",
            models_status.as_u16(),
            models_url,
            body_text
        );
    }
    let models_value: serde_json::Value = serde_json::from_slice(&models_body)
        .with_context(|| format!("failed to parse {models_url}"))?;
    let model_catalog = copilot_runtime_model_catalog_from_token(&models_value);
    Ok(CopilotRuntimeApiAuth {
        api_key: access_token.to_string(),
        model_catalog,
    })
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
        let model_catalog = copilot_runtime_model_catalog_from_token(&value);
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
) -> Vec<serde_json::Value> {
    let mut models = Vec::new();
    collect_copilot_runtime_models(value, &mut models);
    let mut seen = BTreeSet::new();
    models
        .into_iter()
        .filter_map(copilot_runtime_model_catalog_entry)
        .filter(|model| {
            model
                .get("id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| !id.is_empty() && seen.insert(id.to_ascii_lowercase()))
        })
        .map(sanitize_copilot_catalog_entry)
        .collect()
}

/// Strip null-valued string fields from a catalog entry so downstream JSON
/// parsers that reject null strings can load the catalog.
fn sanitize_copilot_catalog_entry(mut entry: serde_json::Value) -> serde_json::Value {
    let Some(_object) = entry.as_object_mut() else {
        return entry;
    };
    // Recursively strip null values from nested capabilities before they reach
    // downstream JSON parsers that reject null where a string/object is expected.
    fn strip_nulls(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                map.retain(|_, v| !v.is_null());
                for v in map.values_mut() {
                    strip_nulls(v);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    strip_nulls(v);
                }
            }
            _ => {}
        }
    }
    strip_nulls(&mut entry);
    entry
}

fn collect_copilot_runtime_models<'a>(
    value: &'a serde_json::Value,
    output: &mut Vec<&'a serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, nested) in object {
                if (key.eq_ignore_ascii_case("models")
                    || key.eq_ignore_ascii_case("available_models")
                    || key.eq_ignore_ascii_case("model_catalog")
                    || key.eq_ignore_ascii_case("chat_models")
                    || key.eq_ignore_ascii_case("data"))
                    && let Some(array) = nested.as_array()
                {
                    output.extend(array);
                    continue;
                }
                collect_copilot_runtime_models(nested, output);
            }
        }
        serde_json::Value::Array(values) => {
            for nested in values {
                collect_copilot_runtime_models(nested, output);
            }
        }
        _ => {}
    }
}

fn copilot_runtime_model_catalog_entry(value: &serde_json::Value) -> Option<serde_json::Value> {
    let object = value.as_object()?;
    let id = object
        .get("id")
        .or_else(|| object.get("model"))
        .or_else(|| object.get("slug"))
        .or_else(|| object.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    let display_name = object
        .get("name")
        .or_else(|| object.get("display_name"))
        .or_else(|| object.get("label"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(id);
    let max_context_window = object
        .get("context_window")
        .or_else(|| object.get("context_window_tokens"))
        .or_else(|| object.get("max_context_tokens"))
        .or_else(|| object.get("max_input_tokens"))
        .or_else(|| {
            object
                .get("capabilities")
                .and_then(|c| c.get("limits"))
                .and_then(|l| l.get("max_context_window_tokens"))
        })
        .and_then(serde_json::Value::as_u64)
        .filter(|tokens| *tokens > 1);
    let max_prompt_tokens = object
        .get("max_prompt_tokens")
        .or_else(|| {
            object
                .get("capabilities")
                .and_then(|c| c.get("limits"))
                .and_then(|l| l.get("max_prompt_tokens"))
        })
        .and_then(serde_json::Value::as_u64)
        .filter(|tokens| *tokens > 1);
    // Copilot CLI distinguishes total context from prompt/input limit. Codex's
    // custom model catalog has one effective context budget, so keep it at the
    // prompt limit when available to avoid sending requests Copilot will reject.
    let context_window = max_prompt_tokens.or(max_context_window).unwrap_or(200_000);
    let mut entry = serde_json::json!({
        "id": id,
        "object": "model",
        "owned_by": "github-copilot",
        "display_name": display_name,
        "description": format!("GitHub Copilot model available for this account: {display_name}."),
        "context_window": context_window,
        "input_cost_per_million_microusd": 0,
        "output_cost_per_million_microusd": 0,
    });
    if let Some(max_context_window) = max_context_window {
        entry["max_context_window"] = serde_json::json!(max_context_window);
    }
    if let Some(max_prompt_tokens) = max_prompt_tokens {
        entry["max_prompt_tokens"] = serde_json::json!(max_prompt_tokens);
    }
    if let Some(capabilities) = object.get("capabilities") {
        entry["capabilities"] = capabilities.clone();
    }
    Some(entry)
}
