use super::codex_openai_auth_headers_for_home;
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_body_with_limit};
use anyhow::{Context, Result};
use prodex_quota::UsageAuth;
use reqwest::blocking::Client;
use std::path::Path;

pub(super) fn send_usage_request(
    client: &Client,
    codex_home: &Path,
    usage_url: &str,
    auth: &UsageAuth,
) -> Result<(reqwest::StatusCode, Vec<u8>)> {
    let send = || {
        let mut request = codex_openai_auth_headers_for_home(client.get(usage_url), codex_home)?
            .header("Authorization", format!("Bearer {}", auth.access_token));

        if let Some(account_id) = auth.account_id.as_deref() {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let response = request
            .send()
            .with_context(|| format!("failed to request quota endpoint {usage_url}"))?;
        let status = response.status();
        let body = read_blocking_response_body_with_limit(
            response,
            RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
            "failed to read quota response body",
        )?;
        Ok((status, body))
    };

    let first = send();
    if first.as_ref().is_err_and(error_is_retryable) {
        return send();
    }
    first
}

fn error_is_retryable(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<reqwest::Error>().is_some_and(|error| {
            !error.is_timeout() && (error.is_connect() || error.is_request() || error.is_body())
        })
    })
}
