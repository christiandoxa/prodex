use super::anthropic_rewrite::{RuntimeAnthropicAuth, RuntimeAnthropicProviderAuth};
use super::gemini_rewrite::RuntimeGeminiAuth;
use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite::schedule_runtime_gateway_billing_ledger_reconcile;
use super::local_rewrite_transport_copilot::{
    runtime_copilot_initiator_header, runtime_copilot_request_has_vision_input,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderGatewaySpendEvent, RuntimeProviderWireFormat,
    runtime_provider_gateway_cost_for_request, runtime_provider_gateway_spend_event,
    runtime_provider_label, runtime_provider_model_from_body, runtime_provider_openai_contract,
    runtime_provider_request_ledger_message,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::io::Write;
use std::sync::atomic::Ordering;
use std::time::Instant;

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
        RuntimeLocalRewritePreparedAuth::OpenAiResponses { api_key } => {
            for (name, value) in &request.headers {
                if should_skip_runtime_local_rewrite_request_header(name) {
                    continue;
                }
                if api_key.is_some() && name.eq_ignore_ascii_case("authorization") {
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
    event: RuntimeProviderGatewaySpendEvent,
) {
    runtime_proxy_log(&shared.runtime_shared, event.log_message());
    schedule_runtime_gateway_billing_ledger_reconcile(shared, event.clone());
    if shared.gateway_observability.sink_enabled("jsonl")
        && let Some(path) = shared.gateway_observability.jsonl_path.as_ref()
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(mut file) => {
                if let Ok(payload) = serde_json::to_string(&event) {
                    let _ = writeln!(file, "{payload}");
                }
            }
            Err(err) => runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_observability_jsonl_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("path", path.display().to_string()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            ),
        }
    }
    if shared.gateway_observability.sink_enabled("http")
        && let Some(endpoint) = shared.gateway_observability.http_endpoint.as_deref()
    {
        let payload = runtime_gateway_observability_http_payload(
            &event,
            shared.gateway_observability.http_schema.as_str(),
        );
        let mut request = shared.client.post(endpoint).json(&payload);
        if let Some(token) = shared.gateway_observability.http_bearer_token.as_deref() {
            request = request.bearer_auth(token);
        }
        if let Err(err) = request.send() {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_observability_http_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("endpoint", endpoint),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
        }
    }
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
        assert_eq!(generic["call_id"], "prodex-7");

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
}
