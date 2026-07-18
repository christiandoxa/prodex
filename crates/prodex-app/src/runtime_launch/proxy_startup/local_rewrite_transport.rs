use super::anthropic_rewrite::{RuntimeAnthropicAuth, RuntimeAnthropicProviderAuth};
use super::gemini_rewrite::RuntimeGeminiAuth;
use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_transport_copilot::{
    runtime_copilot_initiator_header, runtime_copilot_request_has_vision_input,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_gateway_cost_for_request,
    runtime_provider_gateway_pricing_model, runtime_provider_gateway_spend_event,
    runtime_provider_label, runtime_provider_model_from_body,
    runtime_provider_request_ledger_message,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result};
use prodex_domain::RequestId;
use prodex_provider_core::{ProviderAdapterContract, ProviderWireFormat, provider_adapter};
use runtime_proxy_crate::{
    local_bridge_authorization_bearer_token, path_without_query, runtime_proxy_log_field,
    runtime_proxy_structured_log_message,
};
use std::sync::atomic::Ordering;
use std::time::Instant;

const ANTHROPIC_API_VERSION: &str = "2023-06-01";

mod observability;
#[path = "local_rewrite_transport/projected_credential.rs"]
mod projected_credential;

pub(super) use observability::emit_runtime_gateway_spend_event;
pub(super) use projected_credential::{
    runtime_gateway_with_outbound_secret, runtime_local_rewrite_with_projected_provider_secret,
};
use projected_credential::{
    runtime_local_rewrite_apply_projected_bearer, runtime_local_rewrite_apply_projected_header,
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
    OpenAiProjected,
    DeepSeek {
        api_key: Option<&'a str>,
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
            RuntimeLocalRewritePreparedAuth::OpenAiProjected => {
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
) -> Result<reqwest::blocking::Response> {
    let provider_kind = auth.bridge_kind();
    let body_bytes = body.len();
    let model = runtime_provider_model_from_body(&body);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime local rewrite",
            request.method
        )
    })?;
    let request_body_for_spend = body.clone();
    let mut upstream_request = shared.client.request(method, upstream_url);
    match auth {
        RuntimeLocalRewritePreparedAuth::Anthropic {
            auth,
            native_messages,
        } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                );
            if native_messages {
                upstream_request =
                    upstream_request.header("anthropic-version", ANTHROPIC_API_VERSION);
            }
            match auth {
                RuntimeAnthropicAuth::Projected => {
                    upstream_request = if native_messages {
                        runtime_local_rewrite_apply_projected_header(
                            shared,
                            upstream_request,
                            "x-api-key",
                        )?
                    } else {
                        runtime_local_rewrite_apply_projected_bearer(shared, upstream_request)?
                    };
                }
                _ => {
                    upstream_request = runtime_local_rewrite_apply_direct_anthropic_auth(
                        upstream_request,
                        auth,
                        native_messages,
                    )
                    .expect("non-projected Anthropic auth builds request");
                }
            }
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
        RuntimeLocalRewritePreparedAuth::Copilot { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                )
                .header("copilot-integration-id", "copilot-developer-cli")
                .header("openai-intent", "conversation-panel")
                .header("x-github-api-version", "2025-04-01")
                .header("x-request-id", format!("prodex-{}", RequestId::new()))
                .header("X-Initiator", runtime_copilot_initiator_header(request))
                .header(
                    reqwest::header::USER_AGENT,
                    "copilot/1.0.65 (client/github/cli)",
                );
            upstream_request = match api_key {
                Some(api_key) => upstream_request.bearer_auth(api_key),
                None => runtime_local_rewrite_apply_projected_bearer(shared, upstream_request)?,
            };
            if runtime_copilot_request_has_vision_input(&body) {
                upstream_request = upstream_request.header("copilot-vision-request", "true");
            }
        }
        RuntimeLocalRewritePreparedAuth::OpenAiResponses { api_key } => {
            let replacing_openai_auth = api_key.is_some()
                || request.headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("authorization")
                        && runtime_local_rewrite_authorization_is_gateway_credential(shared, value)
                });
            upstream_request = runtime_local_rewrite_copy_openai_headers(
                request,
                upstream_request,
                replacing_openai_auth,
            );
            if let Some(api_key) = api_key {
                upstream_request = upstream_request.bearer_auth(api_key);
            }
        }
        RuntimeLocalRewritePreparedAuth::OpenAiProjected => {
            upstream_request =
                runtime_local_rewrite_copy_openai_headers(request, upstream_request, true);
            upstream_request =
                runtime_local_rewrite_apply_projected_bearer(shared, upstream_request)?;
        }
        RuntimeLocalRewritePreparedAuth::DeepSeek { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity");
            upstream_request = match api_key {
                Some(api_key) => upstream_request.bearer_auth(api_key),
                None => runtime_local_rewrite_apply_projected_bearer(shared, upstream_request)?,
            };
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
        RuntimeLocalRewritePreparedAuth::Gemini { auth } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                );
            match auth {
                RuntimeGeminiAuth::ApiKey { api_key } => {
                    upstream_request = upstream_request.header("x-goog-api-key", api_key);
                }
                RuntimeGeminiAuth::OAuth { access_token, .. } => {
                    upstream_request = upstream_request.bearer_auth(access_token);
                }
                RuntimeGeminiAuth::Projected => {
                    upstream_request = runtime_local_rewrite_apply_projected_header(
                        shared,
                        upstream_request,
                        "x-goog-api-key",
                    )?;
                }
            }
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
        RuntimeLocalRewritePreparedAuth::GeminiOpenAi { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                );
            upstream_request = match api_key {
                Some(api_key) => upstream_request.bearer_auth(api_key),
                None => runtime_local_rewrite_apply_projected_bearer(shared, upstream_request)?,
            };
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
    }
    if !matches!(provider_kind, RuntimeProviderBridgeKind::OpenAiResponses) {
        upstream_request = runtime_local_rewrite_trace_context_headers(upstream_request, request);
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
    let response = upstream_request
        .body(body)
        .send()
        .map_err(reqwest::Error::without_url)
        .with_context(|| {
            format!(
                "failed to proxy local provider request to {}",
                runtime_local_rewrite_log_url(upstream_url)
            )
        })?;
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
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    let pricing_model = runtime_provider_gateway_pricing_model(
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &request_body_for_spend,
        model.as_deref().unwrap_or("unknown"),
    );
    let governed_cost = shared
        .governed_pricing
        .as_ref()
        .and_then(|pricing| pricing.cost_for_model(provider_kind.provider_id(), &pricing_model));
    let cost = runtime_provider_gateway_cost_for_request(
        provider_kind,
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &request_body_for_spend,
        model.as_deref().unwrap_or("unknown"),
        governed_cost,
    );
    emit_runtime_gateway_spend_event(
        shared,
        runtime_provider_gateway_spend_event(
            request_id,
            provider_kind,
            &request.path_and_query,
            model.as_deref(),
            response.status().as_u16(),
            started_at.elapsed().as_millis(),
            body_bytes,
            &request_body_for_spend,
            cost,
        ),
    );
    Ok(response)
}

fn runtime_local_rewrite_apply_direct_anthropic_auth(
    request: reqwest::blocking::RequestBuilder,
    auth: &RuntimeAnthropicAuth,
    native_messages: bool,
) -> Option<reqwest::blocking::RequestBuilder> {
    match auth {
        RuntimeAnthropicAuth::ApiKey { api_key } if native_messages => {
            Some(request.header("x-api-key", api_key))
        }
        RuntimeAnthropicAuth::ApiKey { api_key } => Some(request.bearer_auth(api_key)),
        RuntimeAnthropicAuth::OAuth { access_token } => Some(
            request
                .bearer_auth(access_token)
                .header("anthropic-beta", "oauth-2025-04-20"),
        ),
        RuntimeAnthropicAuth::Projected => None,
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
            reqwest::blocking::Client::new()
                .post("https://api.anthropic.com/v1/messages")
                .header("anthropic-version", ANTHROPIC_API_VERSION),
            &auth,
            true,
        )
        .unwrap()
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
            reqwest::blocking::Client::new()
                .post("https://api.anthropic.com/v1/messages")
                .header("anthropic-version", ANTHROPIC_API_VERSION),
            &auth,
            true,
        )
        .unwrap()
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
    mut upstream_request: reqwest::blocking::RequestBuilder,
    replacing_openai_auth: bool,
) -> reqwest::blocking::RequestBuilder {
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

pub(super) fn runtime_local_rewrite_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let base_url = base_url.trim_end_matches('/');
    let mount_path = mount_path.trim_end_matches('/');
    let path_and_query = runtime_proxy_crate::runtime_escape_url_path_dot_segments(path_and_query);
    let (path, query) = path_and_query
        .as_ref()
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query.as_ref(), None));
    let suffix = path
        .strip_prefix(mount_path)
        .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
        .unwrap_or(path);
    let mut upstream_url = if suffix.is_empty() {
        base_url.to_string()
    } else if suffix.starts_with('/') {
        format!("{base_url}{suffix}")
    } else {
        format!("{base_url}/{suffix}")
    };
    if let Some(query) = query {
        upstream_url.push('?');
        upstream_url.push_str(query);
    }
    upstream_url
}

pub(super) fn runtime_local_rewrite_log_url(value: &str) -> String {
    if let Ok(mut url) = reqwest::Url::parse(value) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        return url.to_string();
    }
    value
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(value)
        .to_string()
}

pub(super) fn runtime_deepseek_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    runtime_openai_standard_provider_upstream_url(
        RuntimeProviderBridgeKind::DeepSeek,
        base_url,
        mount_path,
        path_and_query,
    )
}

pub(super) fn runtime_openai_standard_provider_upstream_url(
    provider_kind: RuntimeProviderBridgeKind,
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let adapter = provider_adapter(provider_kind.provider_id());
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses")
        && matches!(
            adapter.upstream_request_format(),
            ProviderWireFormat::OpenAiChatCompletions
        )
    {
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
}

pub(super) fn runtime_anthropic_messages_upstream_url(base_url: &str, mount_path: &str) -> String {
    runtime_local_rewrite_upstream_url(base_url, mount_path, "/messages")
}

pub(super) fn runtime_gemini_openai_compatible_upstream_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/openai") {
        format!("{base_url}/chat/completions")
    } else {
        format!("{base_url}/openai/chat/completions")
    }
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
        RuntimeAnthropicProviderAuth::Projected => {
            vec![RuntimeLocalRewriteSelectedAnthropicAuth {
                label: "projected".to_string(),
                auth: RuntimeAnthropicAuth::Projected,
            }]
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

fn runtime_local_rewrite_trace_context_headers(
    mut upstream_request: reqwest::blocking::RequestBuilder,
    request: &RuntimeProxyRequest,
) -> reqwest::blocking::RequestBuilder {
    for header_name in ["traceparent", "tracestate", "baggage"] {
        if let Some(header_value) = runtime_local_rewrite_header(request, header_name) {
            upstream_request = upstream_request.header(header_name, header_value);
        }
    }
    upstream_request
}

fn should_skip_runtime_local_rewrite_request_header(name: &str) -> bool {
    runtime_proxy_crate::is_runtime_transport_local_request_header(name)
        || runtime_proxy_crate::is_prodex_internal_request_header(name)
}

fn runtime_local_rewrite_authorization_is_gateway_credential(
    shared: &RuntimeLocalRewriteProxyShared,
    authorization: &str,
) -> bool {
    if shared
        .gateway_auth_token_hash
        .as_ref()
        .is_some_and(|hash| hash.verify_authorization_header(authorization))
    {
        return true;
    }
    let Some(token) = local_bridge_authorization_bearer_token(authorization) else {
        return false;
    };
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| {
            entries
                .iter()
                .any(|entry| !entry.disabled && entry.key.token_hash.verify_bearer_token(token))
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "local_rewrite_transport/tests/url.rs"]
mod tests;
