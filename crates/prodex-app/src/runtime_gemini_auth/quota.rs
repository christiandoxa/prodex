use super::code_assist::{
    GeminiCodeAssistSetupMode, fetch_gemini_code_assist_plan, gemini_code_assist_endpoint,
    gemini_validation_from_body, handle_gemini_validation,
    resolve_gemini_code_assist_project_with_endpoint,
};
use super::{
    GeminiOAuthSecret, force_refresh_gemini_oauth_secret, gemini_oauth_project_from_env,
    normalize_gemini_project_id, refresh_gemini_oauth_secret_if_needed, write_gemini_oauth_secret,
};
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_text_with_limit};
use anyhow::{Context, Result, bail};
use prodex_quota::GeminiQuotaInfo;
use redaction::redaction_redact_secret_like_text;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

pub(crate) fn fetch_gemini_quota(
    codex_home: &Path,
    provider_project_id: Option<&str>,
) -> Result<GeminiQuotaInfo> {
    let value = fetch_gemini_quota_json(codex_home, provider_project_id)?;
    gemini_quota_info_from_value(codex_home, value)
}

pub(crate) fn fetch_gemini_quota_with_code_assist_endpoint(
    codex_home: &Path,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<GeminiQuotaInfo> {
    let value = fetch_gemini_quota_json_with_code_assist_endpoint(
        codex_home,
        provider_project_id,
        code_assist_endpoint,
    )?;
    gemini_quota_info_from_value(codex_home, value)
}

fn gemini_quota_info_from_value(codex_home: &Path, value: Value) -> Result<GeminiQuotaInfo> {
    serde_json::from_value(value).with_context(|| {
        format!(
            "invalid JSON returned by Gemini quota backend for {}",
            codex_home.display()
        )
    })
}

pub(crate) fn fetch_gemini_quota_json(
    codex_home: &Path,
    provider_project_id: Option<&str>,
) -> Result<Value> {
    let code_assist_endpoint = gemini_code_assist_endpoint();
    fetch_gemini_quota_json_with_code_assist_endpoint(
        codex_home,
        provider_project_id,
        &code_assist_endpoint,
    )
}

fn fetch_gemini_quota_json_with_code_assist_endpoint(
    codex_home: &Path,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<Value> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Gemini quota HTTP client")?;
    let mut secret = refresh_gemini_oauth_secret_if_needed(codex_home)?;
    let mut project_id = resolve_gemini_quota_project_id(
        &client,
        codex_home,
        &mut secret,
        provider_project_id,
        code_assist_endpoint,
    )?;
    let mut value = match retrieve_gemini_user_quota(
        &client,
        &secret,
        &project_id,
        code_assist_endpoint,
        GeminiCodeAssistSetupMode::NonInteractive,
    ) {
        Ok(value) => value,
        Err(err) if gemini_error_is_http_401(&err) => {
            secret = force_refresh_gemini_oauth_secret(codex_home)
                .context("Gemini quota auth failed and OAuth token refresh failed")?;
            project_id = resolve_gemini_quota_project_id(
                &client,
                codex_home,
                &mut secret,
                provider_project_id,
                code_assist_endpoint,
            )?;
            retrieve_gemini_user_quota(
                &client,
                &secret,
                &project_id,
                code_assist_endpoint,
                GeminiCodeAssistSetupMode::NonInteractive,
            )?
        }
        Err(err) => return Err(err),
    };
    let plan = fetch_gemini_code_assist_plan(&client, &secret, &project_id, code_assist_endpoint)
        .ok()
        .flatten();
    if let Some(object) = value.as_object_mut() {
        object.insert("email".to_string(), Value::String(secret.email.clone()));
        if let Some(plan) = plan {
            object.insert("plan".to_string(), Value::String(plan));
        }
        object.insert("project_id".to_string(), Value::String(project_id));
    }
    Ok(value)
}

fn resolve_gemini_quota_project_id(
    client: &Client,
    codex_home: &Path,
    secret: &mut GeminiOAuthSecret,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<String> {
    if let Some(project_id) = normalize_gemini_project_id(provider_project_id) {
        return Ok(project_id);
    }
    if let Some(project_id) = normalize_gemini_project_id(secret.project_id.as_deref()) {
        return Ok(project_id);
    }
    if let Some(project_id) = gemini_oauth_project_from_env() {
        return Ok(project_id);
    }
    if let Some(project_id) = resolve_gemini_code_assist_project_with_endpoint(
        client,
        secret,
        code_assist_endpoint,
        GeminiCodeAssistSetupMode::NonInteractive,
    )? {
        secret.project_id = Some(project_id.clone());
        write_gemini_oauth_secret(codex_home, secret)?;
        return Ok(project_id);
    }
    bail!(
        "Gemini OAuth quota is disabled. The Codex-fronted Gemini bridge accepts API keys; supported Vertex AI authentication belongs to the native `prodex s gemini --cli gemini` path and does not use this OAuth quota route"
    )
}

fn retrieve_gemini_user_quota(
    client: &Client,
    secret: &GeminiOAuthSecret,
    project_id: &str,
    code_assist_endpoint: &str,
    mode: GeminiCodeAssistSetupMode,
) -> Result<Value> {
    loop {
        let response = client
            .post(format!("{code_assist_endpoint}:retrieveUserQuota"))
            .bearer_auth(&secret.access_token)
            .json(&json!({
                "project": project_id,
            }))
            .send()
            .context("failed to fetch Gemini quota")?;
        let status = response.status();
        let body = read_blocking_response_text_with_limit(
            response,
            RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
            "failed to read Gemini quota response",
        )?;
        if status.is_success() {
            return serde_json::from_str(&body).context("failed to parse Gemini quota response");
        }
        if let Some(validation) = gemini_validation_from_body(&body) {
            handle_gemini_validation(&validation, mode)?;
            continue;
        }
        let body = gemini_quota_redacted_error_body(&body);
        bail!(
            "Gemini quota request failed (HTTP {}): {body}",
            status.as_u16()
        );
    }
}

fn gemini_quota_redacted_error_body(body: &str) -> String {
    redaction_redact_secret_like_text(body)
}

fn gemini_error_is_http_401(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("HTTP 401"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_gemini_oauth_quota_fails_before_network_access() {
        let error = fetch_gemini_quota(Path::new("/synthetic/profile"), None)
            .expect_err("disabled Gemini OAuth quota must fail");
        let message = error.to_string();
        assert!(message.contains("unsupported and disabled"));
        assert!(message.contains("Gemini API key"));
        assert!(message.contains("Vertex AI"));
    }
}
