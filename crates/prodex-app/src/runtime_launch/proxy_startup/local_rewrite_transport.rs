use super::anthropic_rewrite::{RuntimeAnthropicAuth, RuntimeAnthropicProviderAuth};
use super::gemini_rewrite::RuntimeGeminiAuth;
use super::local_rewrite::{RuntimeLocalRewriteAsyncResponse, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_transport_copilot::{
    runtime_copilot_initiator_header, runtime_copilot_request_has_vision_input,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_label, runtime_provider_model_from_body,
    runtime_provider_request_ledger_message,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result};
use prodex_provider_core::{ProviderWireFormat, provider_adapter};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

const ANTHROPIC_API_VERSION: &str = "2023-06-01";

#[path = "local_rewrite_transport/urls.rs"]
mod urls;

pub(super) use urls::{
    runtime_anthropic_messages_upstream_url, runtime_deepseek_anthropic_messages_upstream_url,
    runtime_deepseek_upstream_url, runtime_gemini_openai_compatible_upstream_url,
    runtime_local_rewrite_log_url, runtime_local_rewrite_upstream_url,
    runtime_openai_standard_provider_upstream_url,
};

pub(super) enum RuntimeLocalRewritePreparedAuth<'a> {
    Anthropic {
        auth: &'a RuntimeAnthropicAuth,
        native_messages: bool,
    },
    Copilot {
        api_key: Option<&'a str>,
    },
    OpenAiResponses {
        api_key: Option<&'a str>,
    },
    DeepSeek {
        api_key: Option<&'a str>,
        native_messages: bool,
    },
    Gemini {
        auth: &'a RuntimeGeminiAuth,
    },
    GeminiOpenAi {
        api_key: Option<&'a str>,
    },
}

pub(super) struct RuntimeLocalRewriteSelectedAnthropicAuth {
    pub(super) label: String,
    pub(super) auth: RuntimeAnthropicAuth,
}

impl RuntimeLocalRewritePreparedAuth<'_> {
    fn bridge_kind(&self) -> RuntimeProviderBridgeKind {
        match self {
            RuntimeLocalRewritePreparedAuth::Anthropic { .. } => {
                RuntimeProviderBridgeKind::Anthropic
            }
            RuntimeLocalRewritePreparedAuth::Copilot { .. } => RuntimeProviderBridgeKind::Copilot,
            RuntimeLocalRewritePreparedAuth::OpenAiResponses { .. } => {
                RuntimeProviderBridgeKind::OpenAiResponses
            }
            RuntimeLocalRewritePreparedAuth::DeepSeek { .. } => RuntimeProviderBridgeKind::DeepSeek,
            RuntimeLocalRewritePreparedAuth::Gemini { .. }
            | RuntimeLocalRewritePreparedAuth::GeminiOpenAi { .. } => {
                RuntimeProviderBridgeKind::Gemini
            }
        }
    }
}

pub(super) fn send_runtime_local_rewrite_prepared_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    upstream_url: &str,
    body: Vec<u8>,
    auth: RuntimeLocalRewritePreparedAuth<'_>,
) -> Result<RuntimeLocalRewriteAsyncResponse> {
    let provider_kind = auth.bridge_kind();
    let body_bytes = body.len();
    let model = runtime_provider_model_from_body(&body);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime local rewrite",
            request.method
        )
    })?;
    let mut upstream_request = shared
        .runtime_shared
        .async_client
        .request(method, upstream_url);
    upstream_request =
        runtime_local_rewrite_apply_prepared_auth(upstream_request, request, shared, &body, auth)?;
    if !matches!(provider_kind, RuntimeProviderBridgeKind::OpenAiResponses) {
        upstream_request =
            runtime_local_rewrite_copy_openai_headers(request, shared, upstream_request, true);
    }
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_upstream_start",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("method", request.method.as_str()),
                runtime_proxy_log_field("url", runtime_local_rewrite_log_url(upstream_url)),
                runtime_proxy_log_field("provider", runtime_provider_label(provider_kind)),
                runtime_proxy_log_field("model", model.as_deref().unwrap_or("unknown")),
                runtime_proxy_log_field("body_bytes", body_bytes.to_string()),
            ],
        ),
    );
    let started_at = Instant::now();
    let response = shared
        .runtime_shared
        .async_runtime
        .block_on(async move { upstream_request.body(body).send().await })
        .map_err(reqwest::Error::without_url)
        .with_context(|| {
            format!(
                "failed to proxy local provider request to {}",
                runtime_local_rewrite_log_url(upstream_url)
            )
        })?;
    let response = RuntimeLocalRewriteAsyncResponse::new(
        response,
        Arc::clone(&shared.runtime_shared.async_runtime),
        shared
            .runtime_shared
            .runtime_config
            .tuning
            .stream_idle_timeout_ms,
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_upstream_response",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("status", response.status().as_u16().to_string()),
                runtime_proxy_log_field("elapsed_ms", started_at.elapsed().as_millis().to_string()),
            ],
        ),
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_provider_request_ledger_message(
            request_id,
            provider_kind,
            &request.path_and_query,
            model.as_deref(),
            response.status().as_u16(),
            started_at.elapsed().as_millis(),
            body_bytes,
        ),
    );
    Ok(response)
}

fn runtime_local_rewrite_apply_prepared_auth(
    upstream_request: reqwest::RequestBuilder,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
    auth: RuntimeLocalRewritePreparedAuth<'_>,
) -> Result<reqwest::RequestBuilder> {
    match auth {
        RuntimeLocalRewritePreparedAuth::Anthropic {
            auth,
            native_messages,
        } => runtime_local_rewrite_apply_anthropic_auth(
            upstream_request,
            request,
            shared,
            auth,
            native_messages,
        ),
        RuntimeLocalRewritePreparedAuth::Copilot { api_key } => {
            runtime_local_rewrite_apply_copilot_auth(
                upstream_request,
                request,
                shared,
                body,
                api_key,
            )
        }
        RuntimeLocalRewritePreparedAuth::OpenAiResponses { api_key } => Ok(
            runtime_local_rewrite_apply_openai_auth(upstream_request, request, shared, api_key),
        ),
        RuntimeLocalRewritePreparedAuth::DeepSeek {
            api_key,
            native_messages,
        } => runtime_local_rewrite_apply_deepseek_auth(
            upstream_request,
            request,
            shared,
            api_key,
            native_messages,
        ),
        RuntimeLocalRewritePreparedAuth::Gemini { auth } => {
            runtime_local_rewrite_apply_gemini_auth(upstream_request, request, shared, auth)
        }
        RuntimeLocalRewritePreparedAuth::GeminiOpenAi { api_key } => {
            runtime_local_rewrite_apply_gemini_openai_auth(
                upstream_request,
                request,
                shared,
                api_key,
            )
        }
    }
}

fn runtime_local_rewrite_provider_headers(
    request: reqwest::RequestBuilder,
) -> reqwest::RequestBuilder {
    request
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .header(
            reqwest::header::ACCEPT,
            "text/event-stream, application/json",
        )
}

fn runtime_local_rewrite_apply_anthropic_auth(
    upstream_request: reqwest::RequestBuilder,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeAnthropicAuth,
    native_messages: bool,
) -> Result<reqwest::RequestBuilder> {
    let mut upstream_request = runtime_local_rewrite_provider_headers(upstream_request);
    if native_messages {
        upstream_request = upstream_request.header("anthropic-version", ANTHROPIC_API_VERSION);
    }
    upstream_request =
        runtime_local_rewrite_apply_direct_anthropic_auth(upstream_request, auth, native_messages);
    if let Some(user_agent) = runtime_local_rewrite_header_if_allowed(request, shared, "user-agent")
    {
        upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
    }
    Ok(upstream_request)
}

fn runtime_local_rewrite_apply_copilot_auth(
    upstream_request: reqwest::RequestBuilder,
    request: &RuntimeProxyRequest,
    _shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
    api_key: Option<&str>,
) -> Result<reqwest::RequestBuilder> {
    let mut upstream_request = runtime_local_rewrite_provider_headers(upstream_request)
        .header("copilot-integration-id", "copilot-developer-cli")
        .header("openai-intent", "conversation-panel")
        .header("x-github-api-version", "2025-04-01")
        .header("x-request-id", format!("prodex-{}", uuid::Uuid::now_v7()))
        .header("X-Initiator", runtime_copilot_initiator_header(request))
        .header(
            reqwest::header::USER_AGENT,
            "copilot/1.0.65 (client/github/cli)",
        );
    let api_key = api_key.context("Copilot API credential is unavailable")?;
    upstream_request = upstream_request.bearer_auth(api_key);
    if runtime_copilot_request_has_vision_input(body) {
        upstream_request = upstream_request.header("copilot-vision-request", "true");
    }
    Ok(upstream_request)
}

fn runtime_local_rewrite_apply_openai_auth(
    upstream_request: reqwest::RequestBuilder,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    api_key: Option<&str>,
) -> reqwest::RequestBuilder {
    let replacing_openai_auth = api_key.is_some();
    let mut upstream_request = runtime_local_rewrite_copy_openai_headers(
        request,
        shared,
        upstream_request,
        replacing_openai_auth,
    );
    if let Some(api_key) = api_key {
        upstream_request = upstream_request.bearer_auth(api_key);
    }
    upstream_request
}

fn runtime_local_rewrite_apply_deepseek_auth(
    upstream_request: reqwest::RequestBuilder,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    api_key: Option<&str>,
    native_messages: bool,
) -> Result<reqwest::RequestBuilder> {
    let mut upstream_request = runtime_local_rewrite_provider_headers(upstream_request);
    if native_messages {
        upstream_request = upstream_request.header("anthropic-version", ANTHROPIC_API_VERSION);
    }
    let api_key = api_key.context("DeepSeek API credential is unavailable")?;
    upstream_request =
        runtime_local_rewrite_apply_direct_api_key_auth(upstream_request, api_key, native_messages);
    if let Some(user_agent) = runtime_local_rewrite_header_if_allowed(request, shared, "user-agent")
    {
        upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
    }
    Ok(upstream_request)
}

fn runtime_local_rewrite_apply_gemini_auth(
    upstream_request: reqwest::RequestBuilder,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeGeminiAuth,
) -> Result<reqwest::RequestBuilder> {
    let mut upstream_request = runtime_local_rewrite_provider_headers(upstream_request);
    match auth {
        RuntimeGeminiAuth::ApiKey { api_key } => {
            upstream_request = upstream_request.header("x-goog-api-key", api_key);
        }
        RuntimeGeminiAuth::OAuth { access_token, .. } => {
            upstream_request = upstream_request.bearer_auth(access_token);
        }
    }
    if let Some(user_agent) = runtime_local_rewrite_header_if_allowed(request, shared, "user-agent")
    {
        upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
    }
    Ok(upstream_request)
}

fn runtime_local_rewrite_apply_gemini_openai_auth(
    upstream_request: reqwest::RequestBuilder,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    api_key: Option<&str>,
) -> Result<reqwest::RequestBuilder> {
    let mut upstream_request = runtime_local_rewrite_provider_headers(upstream_request);
    let api_key = api_key.context("Gemini API credential is unavailable")?;
    upstream_request = upstream_request.bearer_auth(api_key);
    if let Some(user_agent) = runtime_local_rewrite_header_if_allowed(request, shared, "user-agent")
    {
        upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
    }
    Ok(upstream_request)
}

fn runtime_local_rewrite_apply_direct_anthropic_auth(
    request: reqwest::RequestBuilder,
    auth: &RuntimeAnthropicAuth,
    native_messages: bool,
) -> reqwest::RequestBuilder {
    match auth {
        RuntimeAnthropicAuth::ApiKey { api_key } => {
            runtime_local_rewrite_apply_direct_api_key_auth(request, api_key, native_messages)
        }
        RuntimeAnthropicAuth::OAuth { access_token } => request
            .bearer_auth(access_token)
            .header("anthropic-beta", "oauth-2025-04-20"),
    }
}

fn runtime_local_rewrite_apply_direct_api_key_auth(
    request: reqwest::RequestBuilder,
    api_key: &str,
    native_messages: bool,
) -> reqwest::RequestBuilder {
    if native_messages {
        request.header("x-api-key", api_key)
    } else {
        request.bearer_auth(api_key)
    }
}

#[cfg(test)]
mod anthropic_native_transport_tests {
    use super::*;

    #[test]
    fn native_anthropic_api_key_uses_required_headers_without_bearer() {
        let auth = RuntimeAnthropicAuth::ApiKey {
            api_key: "fixture-anthropic-key".to_string(),
        };
        let request = runtime_local_rewrite_apply_direct_anthropic_auth(
            reqwest::Client::new()
                .post("https://api.anthropic.com/v1/messages")
                .header("anthropic-version", ANTHROPIC_API_VERSION),
            &auth,
            true,
        )
        .build()
        .unwrap();

        assert_eq!(
            request.headers()["anthropic-version"],
            ANTHROPIC_API_VERSION
        );
        assert_eq!(request.headers()["x-api-key"], "fixture-anthropic-key");
        assert!(
            !request
                .headers()
                .contains_key(reqwest::header::AUTHORIZATION)
        );
    }

    #[test]
    fn native_anthropic_oauth_preserves_bearer_and_beta_headers() {
        let auth = RuntimeAnthropicAuth::OAuth {
            access_token: "fixture-oauth-token".to_string(),
        };
        let request = runtime_local_rewrite_apply_direct_anthropic_auth(
            reqwest::Client::new()
                .post("https://api.anthropic.com/v1/messages")
                .header("anthropic-version", ANTHROPIC_API_VERSION),
            &auth,
            true,
        )
        .build()
        .unwrap();

        assert_eq!(
            request.headers()[reqwest::header::AUTHORIZATION],
            "Bearer fixture-oauth-token"
        );
        assert_eq!(request.headers()["anthropic-beta"], "oauth-2025-04-20");
        assert_eq!(
            request.headers()["anthropic-version"],
            ANTHROPIC_API_VERSION
        );
        assert!(!request.headers().contains_key("x-api-key"));
    }
}

fn runtime_local_rewrite_copy_openai_headers(
    request: &RuntimeProxyRequest,
    _shared: &RuntimeLocalRewriteProxyShared,
    mut upstream_request: reqwest::RequestBuilder,
    replacing_openai_auth: bool,
) -> reqwest::RequestBuilder {
    let connection_headers = runtime_proxy_crate::runtime_connection_header_tokens(
        request
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
    );
    for (name, value) in &request.headers {
        if runtime_proxy_crate::runtime_header_name_matches_connection_token(
            name,
            &connection_headers,
        ) || should_skip_runtime_local_rewrite_request_header(name)
        {
            continue;
        }
        if replacing_openai_auth
            && (name.eq_ignore_ascii_case("authorization")
                || name.eq_ignore_ascii_case("chatgpt-account-id"))
        {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    upstream_request
}

pub(super) fn runtime_local_rewrite_api_key_attempts<'a>(
    shared: &RuntimeLocalRewriteProxyShared,
    api_keys: &'a [String],
) -> Vec<(String, &'a str)> {
    if api_keys.is_empty() {
        return Vec::new();
    }
    let start = if api_keys.len() == 1 {
        0
    } else {
        shared.api_key_cursor.fetch_add(1, Ordering::Relaxed) % api_keys.len()
    };
    runtime_local_rewrite_api_key_attempts_from_start(api_keys, start)
}

fn runtime_local_rewrite_api_key_attempts_from_start(
    api_keys: &[String],
    start: usize,
) -> Vec<(String, &str)> {
    (0..api_keys.len())
        .map(|offset| {
            let index = (start + offset) % api_keys.len();
            let label = if api_keys.len() == 1 {
                "api-key".to_string()
            } else {
                format!("api-key-{}", index + 1)
            };
            (label, api_keys[index].as_str())
        })
        .collect()
}

pub(super) fn runtime_local_rewrite_anthropic_auth_attempts(
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeAnthropicProviderAuth,
) -> Vec<RuntimeLocalRewriteSelectedAnthropicAuth> {
    match auth {
        RuntimeAnthropicProviderAuth::ApiKeys { api_keys } => {
            runtime_local_rewrite_api_key_attempts(shared, api_keys)
                .into_iter()
                .map(
                    |(label, api_key)| RuntimeLocalRewriteSelectedAnthropicAuth {
                        label,
                        auth: RuntimeAnthropicAuth::ApiKey {
                            api_key: api_key.to_string(),
                        },
                    },
                )
                .collect()
        }
        RuntimeAnthropicProviderAuth::OAuthProfiles { profiles } => {
            if profiles.is_empty() {
                return Vec::new();
            }
            let start = if profiles.len() == 1 {
                0
            } else {
                shared.api_key_cursor.fetch_add(1, Ordering::Relaxed) % profiles.len()
            };
            (0..profiles.len())
                .map(|offset| {
                    let index = (start + offset) % profiles.len();
                    let profile = profiles[index].clone();
                    RuntimeLocalRewriteSelectedAnthropicAuth {
                        label: profile.profile_name.clone(),
                        auth: profile.auth(),
                    }
                })
                .collect()
        }
    }
}

fn runtime_local_rewrite_header<'a>(
    request: &'a RuntimeProxyRequest,
    expected_name: &str,
) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(expected_name))
        .map(|(_, value)| value.as_str())
}

fn runtime_local_rewrite_header_if_allowed<'a>(
    request: &'a RuntimeProxyRequest,
    _shared: &RuntimeLocalRewriteProxyShared,
    expected_name: &str,
) -> Option<&'a str> {
    runtime_local_rewrite_header(request, expected_name)
}

fn should_skip_runtime_local_rewrite_request_header(name: &str) -> bool {
    runtime_proxy_crate::is_runtime_transport_local_request_header(name)
        || runtime_proxy_crate::is_prodex_internal_request_header(name)
}

#[cfg(test)]
#[path = "local_rewrite_transport/tests/url.rs"]
mod tests;
