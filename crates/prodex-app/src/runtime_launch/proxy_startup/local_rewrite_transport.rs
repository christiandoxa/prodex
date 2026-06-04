use super::anthropic_rewrite::{RuntimeAnthropicAuth, RuntimeAnthropicProviderAuth};
use super::gemini_rewrite::RuntimeGeminiAuth;
use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_label, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body, runtime_provider_request_body_with_model,
    runtime_provider_request_ledger_message,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::sync::atomic::Ordering;
use std::time::Instant;

pub(super) enum RuntimeLocalRewritePreparedAuth<'a> {
    Anthropic { auth: &'a RuntimeAnthropicAuth },
    Copilot { api_key: &'a str },
    OpenAiResponses,
    DeepSeek { api_key: &'a str },
    Gemini { auth: &'a RuntimeGeminiAuth },
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
            RuntimeLocalRewritePreparedAuth::OpenAiResponses => {
                RuntimeProviderBridgeKind::OpenAiResponses
            }
            RuntimeLocalRewritePreparedAuth::DeepSeek { .. } => RuntimeProviderBridgeKind::DeepSeek,
            RuntimeLocalRewritePreparedAuth::Gemini { .. } => RuntimeProviderBridgeKind::Gemini,
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
    let mut upstream_request = shared.client.request(method, upstream_url);
    match auth {
        RuntimeLocalRewritePreparedAuth::Anthropic { auth } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                );
            match auth {
                RuntimeAnthropicAuth::ApiKey { api_key } => {
                    upstream_request = upstream_request.bearer_auth(api_key);
                }
                RuntimeAnthropicAuth::OAuth { access_token } => {
                    upstream_request = upstream_request
                        .bearer_auth(access_token)
                        .header("anthropic-beta", "oauth-2025-04-20");
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
                .bearer_auth(api_key)
                .header("copilot-integration-id", "vscode-chat")
                .header("editor-version", "vscode/1.95.0")
                .header("editor-plugin-version", "copilot-chat/0.26.7")
                .header("openai-intent", "conversation-panel")
                .header("x-github-api-version", "2025-04-01")
                .header("x-request-id", format!("prodex-{request_id}"))
                .header("x-vscode-user-agent-library-version", "electron-fetch")
                .header("X-Initiator", runtime_copilot_initiator_header(request));
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            } else {
                upstream_request = upstream_request
                    .header(reqwest::header::USER_AGENT, "GitHubCopilotChat/0.26.7");
            }
            if runtime_copilot_request_has_vision_input(&body) {
                upstream_request = upstream_request.header("copilot-vision-request", "true");
            }
        }
        RuntimeLocalRewritePreparedAuth::OpenAiResponses => {
            for (name, value) in &request.headers {
                if should_skip_runtime_local_rewrite_request_header(name) {
                    continue;
                }
                upstream_request = upstream_request.header(name.as_str(), value.as_str());
            }
        }
        RuntimeLocalRewritePreparedAuth::DeepSeek { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .bearer_auth(api_key);
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
            }
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
    }
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_upstream_start",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("method", request.method.as_str()),
                runtime_proxy_log_field("url", upstream_url),
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
        .with_context(|| format!("failed to proxy local provider request to {upstream_url}"))?;
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

pub(super) fn runtime_local_rewrite_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let base_url = base_url.trim_end_matches('/');
    let mount_path = mount_path.trim_end_matches('/');
    let (path, query) = path_and_query
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query, None));
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

pub(super) fn runtime_deepseek_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses") {
        return runtime_chat_completions_upstream_url(base_url, mount_path, path_and_query);
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
}

pub(super) fn runtime_chat_completions_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses") {
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
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

pub(super) fn runtime_copilot_request_body_with_canonical_model(body: &[u8]) -> Vec<u8> {
    let Some(model) = runtime_provider_model_from_body(body) else {
        return body.to_vec();
    };
    let canonical =
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Copilot, &model)
            .into_iter()
            .next()
            .unwrap_or(model);
    runtime_provider_request_body_with_model(body, &canonical)
}

fn runtime_copilot_initiator_header(request: &RuntimeProxyRequest) -> &'static str {
    if runtime_copilot_request_has_agent_input(&request.body) {
        "agent"
    } else {
        "user"
    }
}

fn runtime_copilot_request_has_agent_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_object().is_some_and(|object| {
                    object
                        .get("role")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|role| !role.is_empty())
                        .is_none_or(|role| role.eq_ignore_ascii_case("assistant"))
                })
            })
        })
}

fn runtime_copilot_request_has_vision_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    runtime_copilot_value_contains_text(&value, "input_image")
        || runtime_copilot_value_contains_key(&value, "image_url")
}

fn runtime_copilot_value_contains_text(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(needle),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| runtime_copilot_value_contains_text(value, needle)),
        serde_json::Value::Object(object) => object
            .values()
            .any(|value| runtime_copilot_value_contains_text(value, needle)),
        _ => false,
    }
}

fn runtime_copilot_value_contains_key(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| runtime_copilot_value_contains_key(value, needle)),
        serde_json::Value::Object(object) => {
            object.contains_key(needle)
                || object
                    .values()
                    .any(|value| runtime_copilot_value_contains_key(value, needle))
        }
        _ => false,
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

fn should_skip_runtime_local_rewrite_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "connection"
            | "content-length"
            | "host"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
        || lower.starts_with("x-prodex-internal-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_attempts_rotate_start_and_include_all_keys() {
        let api_keys = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];

        let attempts = runtime_local_rewrite_api_key_attempts_from_start(&api_keys, 1);

        assert_eq!(
            attempts,
            vec![
                ("api-key-2".to_string(), "second"),
                ("api-key-3".to_string(), "third"),
                ("api-key-1".to_string(), "first"),
            ]
        );
    }

    #[test]
    fn single_api_key_attempt_uses_generic_label() {
        let api_keys = vec!["only".to_string()];

        let attempts = runtime_local_rewrite_api_key_attempts_from_start(&api_keys, 0);

        assert_eq!(attempts, vec![("api-key".to_string(), "only")]);
    }
}
