use super::anthropic_rewrite::{RuntimeAnthropicAuth, RuntimeAnthropicProviderAuth};
use super::gemini_rewrite::RuntimeGeminiAuth;
use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, runtime_gateway_try_reserve_background_task,
    schedule_runtime_gateway_billing_ledger_reconcile,
};
use super::local_rewrite_transport_copilot::{
    runtime_copilot_initiator_header, runtime_copilot_request_has_vision_input,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderGatewaySpendEvent, RuntimeProviderWireFormat,
    runtime_provider_gateway_cost_for_request, runtime_provider_gateway_spend_apply_admission_ids,
    runtime_provider_gateway_spend_event, runtime_provider_label, runtime_provider_model_from_body,
    runtime_provider_openai_contract, runtime_provider_request_ledger_message,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result};
use prodex_domain::RequestId;
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::{
    local_bridge_authorization_bearer_token, path_without_query, runtime_proxy_log_field,
    runtime_proxy_structured_log_message,
};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const RUNTIME_GATEWAY_OBSERVABILITY_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) enum RuntimeLocalRewritePreparedAuth<'a> {
    Anthropic { auth: &'a RuntimeAnthropicAuth },
    Copilot { api_key: &'a str },
    OpenAiResponses { api_key: Option<&'a str> },
    DeepSeek { api_key: &'a str },
    Gemini { auth: &'a RuntimeGeminiAuth },
    GeminiOpenAi { api_key: &'a str },
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
                .header("copilot-integration-id", "copilot-developer-cli")
                .header("openai-intent", "conversation-panel")
                .header("x-github-api-version", "2025-04-01")
                .header("x-request-id", format!("prodex-{}", RequestId::new()))
                .header("X-Initiator", runtime_copilot_initiator_header(request))
                .header(
                    reqwest::header::USER_AGENT,
                    "copilot/1.0.65 (client/github/cli)",
                );
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
                ) {
                    continue;
                }
                if should_skip_runtime_local_rewrite_request_header(name) {
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
            if let Some(api_key) = api_key {
                upstream_request = upstream_request.bearer_auth(api_key);
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
        RuntimeLocalRewritePreparedAuth::GeminiOpenAi { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                )
                .bearer_auth(api_key);
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
    let cost = runtime_provider_gateway_cost_for_request(
        provider_kind,
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &request_body_for_spend,
        model.as_deref().unwrap_or("unknown"),
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

pub(super) fn emit_runtime_gateway_spend_event(
    shared: &RuntimeLocalRewriteProxyShared,
    mut event: RuntimeProviderGatewaySpendEvent,
) {
    let typed_request_id = shared
        .gateway_usage
        .typed_request_ids
        .lock()
        .ok()
        .and_then(|typed_request_ids| typed_request_ids.get(&event.request).cloned());
    let call_id = shared
        .gateway_usage
        .call_ids
        .lock()
        .ok()
        .and_then(|call_ids| call_ids.get(&event.request).cloned());
    runtime_provider_gateway_spend_apply_admission_ids(
        &mut event,
        typed_request_id.as_deref(),
        call_id.as_deref(),
    );
    runtime_proxy_log(&shared.runtime_shared, event.log_message());
    schedule_runtime_gateway_billing_ledger_reconcile(shared, event.clone());
    if !shared.gateway_observability.sink_enabled("jsonl")
        && !shared.gateway_observability.sink_enabled("http")
    {
        return;
    }
    let Some(permit) =
        runtime_gateway_try_reserve_background_task(&shared.gateway_observability_slots)
    else {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_observability_dropped",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field("reason", "task_limit"),
                ],
            ),
        );
        return;
    };
    let shared = shared.clone();
    drop(
        shared
            .runtime_shared
            .async_runtime
            .clone()
            .spawn_blocking(move || {
                let _permit = permit;
                emit_runtime_gateway_observability_sinks(&shared, &event);
            }),
    );
}

fn emit_runtime_gateway_observability_sinks(
    shared: &RuntimeLocalRewriteProxyShared,
    event: &RuntimeProviderGatewaySpendEvent,
) {
    if shared.gateway_observability.sink_enabled("jsonl")
        && let Some(path) = shared.gateway_observability.jsonl_path.as_ref()
    {
        if let Err(err) = runtime_gateway_observability_write_jsonl(path, event) {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_observability_jsonl_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field(
                            "path",
                            runtime_gateway_observability_redacted_log_text(
                                &path.display().to_string(),
                            ),
                        ),
                        runtime_proxy_log_field(
                            "error",
                            runtime_gateway_observability_redacted_log_text(&err.to_string()),
                        ),
                    ],
                ),
            );
        }
    }
    if shared.gateway_observability.sink_enabled("http")
        && let Some(endpoint) = shared.gateway_observability.http_endpoint.as_deref()
    {
        let payload = runtime_gateway_observability_http_payload(
            event,
            shared.gateway_observability.http_schema.as_str(),
        );
        let mut request = shared
            .client
            .post(endpoint)
            .timeout(RUNTIME_GATEWAY_OBSERVABILITY_HTTP_TIMEOUT)
            .json(&payload);
        if let Some(token) = shared.gateway_observability.http_bearer_token.as_deref() {
            request = request.bearer_auth(token);
        }
        if let Err(err) = request.send() {
            let error = runtime_gateway_observability_redacted_log_text(
                &reqwest::Error::without_url(err).to_string(),
            );
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_observability_http_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field(
                            "endpoint",
                            runtime_gateway_observability_redacted_log_text(
                                &runtime_local_rewrite_log_url(endpoint),
                            ),
                        ),
                        runtime_proxy_log_field("error", error),
                    ],
                ),
            );
        }
    }
}

fn runtime_gateway_observability_redacted_log_text(value: &str) -> String {
    redaction_redact_secret_like_text(value)
}

fn runtime_gateway_observability_write_jsonl(
    path: &Path,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let payload = serde_json::to_string(event).map_err(std::io::Error::other)?;
    writeln!(file, "{payload}")
}

fn runtime_gateway_observability_http_payload(
    event: &RuntimeProviderGatewaySpendEvent,
    schema: &str,
) -> serde_json::Value {
    match schema.trim().to_ascii_lowercase().as_str() {
        "otel" | "opentelemetry" => serde_json::json!({
            "resourceLogs": [{
                "resource": {"attributes": [
                    {"key": "service.name", "value": {"stringValue": "prodex-gateway"}}
                ]},
                "scopeLogs": [{
                    "scope": {"name": "prodex.gateway"},
                    "logRecords": [{
                        "severityText": "INFO",
                        "body": {"stringValue": event.event},
                        "attributes": runtime_gateway_otel_attributes(event)
                    }]
                }]
            }]
        }),
        "datadog" => serde_json::json!([{
            "ddsource": "prodex",
            "service": "prodex-gateway",
            "message": event.event,
            "status": "info",
            "prodex": event
        }]),
        "langfuse" => serde_json::json!({
            "batch": [{
                "id": event.call_id,
                "type": "trace-create",
                "body": {
                    "id": event.call_id,
                    "name": "prodex-gateway-call",
                    "metadata": event
                }
            }]
        }),
        _ => serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({})),
    }
}

fn runtime_gateway_otel_attributes(
    event: &RuntimeProviderGatewaySpendEvent,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"key": "prodex.request_id", "value": {"stringValue": event.request_id}}),
        serde_json::json!({"key": "prodex.call_id", "value": {"stringValue": event.call_id}}),
        serde_json::json!({"key": "prodex.phase", "value": {"stringValue": event.phase}}),
        serde_json::json!({"key": "prodex.provider", "value": {"stringValue": event.provider}}),
        serde_json::json!({"key": "prodex.path", "value": {"stringValue": event.path}}),
        serde_json::json!({"key": "prodex.model", "value": {"stringValue": event.model}}),
        serde_json::json!({"key": "http.response.status_code", "value": {"intValue": event.status.to_string()}}),
        serde_json::json!({"key": "prodex.elapsed_ms", "value": {"intValue": event.elapsed_ms.to_string()}}),
        serde_json::json!({"key": "prodex.request_bytes", "value": {"intValue": event.request_bytes.to_string()}}),
    ]
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
    let contract = runtime_provider_openai_contract(provider_kind);
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses")
        && matches!(
            contract.upstream_request_format,
            RuntimeProviderWireFormat::OpenAiChatCompletions
        )
    {
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
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
    let lower = name.trim().to_ascii_lowercase();
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
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn openai_standard_provider_upstream_url_uses_contract_formats() {
        assert_eq!(
            runtime_openai_standard_provider_upstream_url(
                RuntimeProviderBridgeKind::OpenAiResponses,
                "https://upstream.test/v1",
                "/v1",
                "/v1/responses"
            ),
            "https://upstream.test/v1/responses"
        );
        assert_eq!(
            runtime_openai_standard_provider_upstream_url(
                RuntimeProviderBridgeKind::DeepSeek,
                "https://upstream.test/v1",
                "/v1",
                "/v1/responses"
            ),
            "https://upstream.test/v1/chat/completions"
        );
        assert_eq!(
            runtime_openai_standard_provider_upstream_url(
                RuntimeProviderBridgeKind::Copilot,
                "https://upstream.test/v1",
                "/v1",
                "/v1/chat/completions"
            ),
            "https://upstream.test/v1/chat/completions"
        );
        for path in [
            "/v1/embeddings",
            "/v1/images/generations",
            "/v1/audio/transcriptions",
            "/v1/batches",
            "/v1/rerank",
            "/v1/a2a",
            "/v1/messages",
        ] {
            assert_eq!(
                runtime_openai_standard_provider_upstream_url(
                    RuntimeProviderBridgeKind::OpenAiResponses,
                    "https://upstream.test/v1",
                    "/v1",
                    path
                ),
                format!("https://upstream.test{path}")
            );
        }
    }

    #[test]
    fn local_rewrite_upstream_url_neutralizes_dot_segments_before_url_parsing_can_escape_base() {
        assert_eq!(
            runtime_local_rewrite_upstream_url(
                "https://upstream.test/v1",
                "/v1",
                "/v1/../admin?x=1"
            ),
            "https://upstream.test/v1/%252e%252e/admin?x=1"
        );
        assert_eq!(
            runtime_local_rewrite_upstream_url(
                "https://upstream.test/v1",
                "/v1",
                "/v1/%2e%2e/admin"
            ),
            "https://upstream.test/v1/%252e%252e/admin"
        );
    }

    #[test]
    fn gemini_openai_compatible_url_uses_documented_chat_completions_endpoint() {
        assert_eq!(
            runtime_gemini_openai_compatible_upstream_url(
                "https://generativelanguage.googleapis.com/v1beta"
            ),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
        assert_eq!(
            runtime_gemini_openai_compatible_upstream_url(
                "https://generativelanguage.googleapis.com/v1beta/openai/"
            ),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
    }

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

    #[test]
    fn gateway_observability_http_payload_supports_vendor_schemas() {
        let event = runtime_provider_gateway_spend_event(
            7,
            RuntimeProviderBridgeKind::OpenAiResponses,
            "/v1/responses",
            Some("gpt-5-mini"),
            200,
            42,
            128,
            br#"{"model":"gpt-5-mini","input":"hello from prodex"}"#,
            prodex_provider_core::ProviderModelCost::default(),
        );

        let generic = runtime_gateway_observability_http_payload(&event, "generic");
        assert_eq!(generic["event"], "gateway_spend");
        let call_id = generic["call_id"].as_str().expect("call_id is a string");
        let uuid = call_id
            .strip_prefix("prodex-")
            .and_then(|value| prodex_domain::CallId::from_str(value).ok())
            .expect("call_id uses a prodex-scoped uuidv7");
        assert_eq!(uuid.as_uuid().get_version_num(), 7);

        let otel = runtime_gateway_observability_http_payload(&event, "otel");
        assert_eq!(
            otel["resourceLogs"][0]["scopeLogs"][0]["scope"]["name"],
            "prodex.gateway"
        );

        let datadog = runtime_gateway_observability_http_payload(&event, "datadog");
        assert_eq!(datadog[0]["service"], "prodex-gateway");

        let langfuse = runtime_gateway_observability_http_payload(&event, "langfuse");
        assert_eq!(langfuse["batch"][0]["type"], "trace-create");
    }

    #[test]
    fn gateway_observability_jsonl_write_preserves_event_payload() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-jsonl-{}",
            prodex_domain::RequestId::new()
        ));
        let path = root.join("spend.jsonl");
        let event = runtime_provider_gateway_spend_event(
            7,
            RuntimeProviderBridgeKind::OpenAiResponses,
            "/v1/responses",
            Some("gpt-5-mini"),
            200,
            42,
            128,
            br#"{"model":"gpt-5-mini","input":"hello from prodex"}"#,
            prodex_provider_core::ProviderModelCost::default(),
        );

        runtime_gateway_observability_write_jsonl(&path, &event).unwrap();

        let line = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(json["event"], "gateway_spend");
        assert_eq!(json["legacy_request_sequence"], 7);
        assert!(json.get("request").is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_observability_failure_log_text_redacts_endpoint_secrets() {
        let endpoint =
            "https://telemetry.example.test/ingest?api_key=sk-live-fixture-notreal-123456";
        let error = "failed to send Authorization: Bearer fixture_http_notreal_12345";
        let path = "/tmp/prodex/sk-live-fixture-notreal-path/spend.jsonl";

        let redacted_endpoint = runtime_gateway_observability_redacted_log_text(endpoint);
        let redacted_error = runtime_gateway_observability_redacted_log_text(error);
        let redacted_path = runtime_gateway_observability_redacted_log_text(path);

        assert!(redacted_endpoint.contains("api_key=<redacted>"));
        assert!(!redacted_endpoint.contains("sk-live-fixture-notreal-123456"));
        assert!(redacted_error.contains("Authorization: Bearer <redacted>"));
        assert!(!redacted_error.contains("fixture_http_notreal_12345"));
        assert!(redacted_path.contains("sk-live-<redacted>"));
        assert!(!redacted_path.contains("sk-live-fixture-notreal-path"));
    }

    #[test]
    fn log_url_strips_query_fragment_and_userinfo() {
        let url = runtime_local_rewrite_log_url(
            "https://user:secret@example.test/v1/responses?access_token=secret#frag",
        );
        assert_eq!(url, "https://example.test/v1/responses");
        assert_eq!(
            runtime_local_rewrite_log_url("/v1/responses?key=secret"),
            "/v1/responses"
        );
    }
}
