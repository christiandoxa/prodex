use super::provider_models::{
    runtime_provider_model_catalog_json, runtime_provider_model_json_for,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_provider_core::{
    ProviderAdapterContract, ProviderId, ProviderModelCost, ProviderWireFormat,
    calculate_cost_microusd, estimate_request_input_tokens, estimate_text_tokens,
    extract_usage_tokens, microusd_to_usd, provider_adapter, provider_model_cost,
    provider_model_fallback_chain,
};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeProviderBridgeKind {
    Anthropic,
    Copilot,
    OpenAiResponses,
    DeepSeek,
    Gemini,
}

impl RuntimeProviderBridgeKind {
    pub(super) fn provider_id(self) -> ProviderId {
        match self {
            Self::Anthropic => ProviderId::Anthropic,
            Self::Copilot => ProviderId::Copilot,
            Self::OpenAiResponses => ProviderId::OpenAi,
            Self::DeepSeek => ProviderId::DeepSeek,
            Self::Gemini => ProviderId::Gemini,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeProviderWireFormat {
    OpenAiResponses,
    OpenAiChatCompletions,
    GeminiGenerateContent,
}

impl RuntimeProviderWireFormat {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai-responses",
            Self::OpenAiChatCompletions => "openai-chat-completions",
            Self::GeminiGenerateContent => "gemini-generate-content",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RuntimeProviderOpenAiContract {
    pub(super) client_request_format: RuntimeProviderWireFormat,
    pub(super) upstream_request_format: RuntimeProviderWireFormat,
    pub(super) response_format: RuntimeProviderWireFormat,
    pub(super) canonical_client_endpoint: &'static str,
    pub(super) model_list_endpoint: &'static str,
    pub(super) supports_streaming: bool,
    pub(super) supports_model_fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeProviderErrorClass {
    Auth,
    Quota,
    RateLimit,
    Transient,
    NotFound,
    Fatal,
}

#[derive(Clone, Copy)]
struct RuntimeProviderErrorRule {
    status: Option<u16>,
    code: Option<&'static str>,
    text: Option<&'static str>,
    class: RuntimeProviderErrorClass,
    cooldown_ms: u64,
}

const RUNTIME_PROVIDER_ERROR_RULES: &[RuntimeProviderErrorRule] = &[
    RuntimeProviderErrorRule {
        status: Some(401),
        code: None,
        text: None,
        class: RuntimeProviderErrorClass::Auth,
        cooldown_ms: 0,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("unauthenticated"),
        text: None,
        class: RuntimeProviderErrorClass::Auth,
        cooldown_ms: 0,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("invalid_api_key"),
        text: None,
        class: RuntimeProviderErrorClass::Auth,
        cooldown_ms: 0,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("insufficient_quota"),
        text: None,
        class: RuntimeProviderErrorClass::Quota,
        cooldown_ms: 300_000,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("quota_exhausted"),
        text: None,
        class: RuntimeProviderErrorClass::Quota,
        cooldown_ms: 300_000,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("quota_exceeded"),
        text: None,
        class: RuntimeProviderErrorClass::Quota,
        cooldown_ms: 300_000,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("resource_exhausted"),
        text: None,
        class: RuntimeProviderErrorClass::Quota,
        cooldown_ms: 300_000,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("rate_limit_exceeded"),
        text: None,
        class: RuntimeProviderErrorClass::RateLimit,
        cooldown_ms: 60_000,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: Some("rate_limit_exceeded_error"),
        text: None,
        class: RuntimeProviderErrorClass::RateLimit,
        cooldown_ms: 60_000,
    },
    RuntimeProviderErrorRule {
        status: Some(429),
        code: None,
        text: None,
        class: RuntimeProviderErrorClass::RateLimit,
        cooldown_ms: 60_000,
    },
    RuntimeProviderErrorRule {
        status: Some(404),
        code: None,
        text: None,
        class: RuntimeProviderErrorClass::NotFound,
        cooldown_ms: 0,
    },
    RuntimeProviderErrorRule {
        status: Some(500),
        code: None,
        text: None,
        class: RuntimeProviderErrorClass::Transient,
        cooldown_ms: 10_000,
    },
    RuntimeProviderErrorRule {
        status: Some(502),
        code: None,
        text: None,
        class: RuntimeProviderErrorClass::Transient,
        cooldown_ms: 10_000,
    },
    RuntimeProviderErrorRule {
        status: Some(503),
        code: None,
        text: None,
        class: RuntimeProviderErrorClass::Transient,
        cooldown_ms: 10_000,
    },
    RuntimeProviderErrorRule {
        status: Some(504),
        code: None,
        text: None,
        class: RuntimeProviderErrorClass::Transient,
        cooldown_ms: 10_000,
    },
    RuntimeProviderErrorRule {
        status: None,
        code: None,
        text: Some("overloaded"),
        class: RuntimeProviderErrorClass::Transient,
        cooldown_ms: 10_000,
    },
];

pub(super) fn runtime_provider_label(kind: RuntimeProviderBridgeKind) -> &'static str {
    kind.provider_id().label()
}

pub(super) fn runtime_provider_openai_contract(
    kind: RuntimeProviderBridgeKind,
) -> RuntimeProviderOpenAiContract {
    let adapter = provider_adapter(kind.provider_id());
    RuntimeProviderOpenAiContract {
        client_request_format: runtime_provider_wire_format_from_core(
            adapter.client_request_format(),
        ),
        upstream_request_format: runtime_provider_wire_format_from_core(
            adapter.upstream_request_format(),
        ),
        response_format: runtime_provider_wire_format_from_core(adapter.response_format()),
        canonical_client_endpoint: adapter.canonical_client_endpoint(),
        model_list_endpoint: adapter.model_list_endpoint(),
        supports_streaming: adapter.supports_streaming(),
        supports_model_fallback: adapter.supports_model_fallback(),
    }
}

fn runtime_provider_wire_format_from_core(format: ProviderWireFormat) -> RuntimeProviderWireFormat {
    match format {
        ProviderWireFormat::OpenAiResponses => RuntimeProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::OpenAiChatCompletions => {
            RuntimeProviderWireFormat::OpenAiChatCompletions
        }
        ProviderWireFormat::GeminiGenerateContent => {
            RuntimeProviderWireFormat::GeminiGenerateContent
        }
        ProviderWireFormat::AnthropicMessages | ProviderWireFormat::Passthrough => {
            RuntimeProviderWireFormat::OpenAiResponses
        }
    }
}

pub(super) fn runtime_provider_native_passthrough(
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
) -> bool {
    let path = path_without_query(path_and_query);
    match kind {
        RuntimeProviderBridgeKind::OpenAiResponses => true,
        RuntimeProviderBridgeKind::Copilot => {
            !(path.ends_with("/responses")
                || path.ends_with("/responses/compact")
                || runtime_provider_models_path_suffix(path).is_some())
        }
        RuntimeProviderBridgeKind::DeepSeek | RuntimeProviderBridgeKind::Gemini => {
            !(path.ends_with("/responses")
                || path.ends_with("/responses/compact")
                || runtime_provider_models_path_suffix(path).is_some())
        }
        RuntimeProviderBridgeKind::Anthropic => {
            !(path.ends_with("/responses")
                || path.ends_with("/responses/compact")
                || runtime_provider_models_path_suffix(path).is_some())
        }
    }
}

pub(super) fn runtime_provider_models_buffered_response(
    kind: RuntimeProviderBridgeKind,
    method: &str,
    path_and_query: &str,
) -> Option<RuntimeHeapTrimmedBufferedResponseParts> {
    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }
    let models = runtime_provider_model_catalog_json(kind);
    if models.is_empty() {
        return None;
    }
    let path = path_without_query(path_and_query);
    match runtime_provider_models_path_suffix(path)? {
        RuntimeProviderModelsPath::List => {
            let body = serde_json::json!({
                "object": "list",
                "data": models,
            });
            Some(runtime_provider_json_response(200, body))
        }
        RuntimeProviderModelsPath::Single(model_id) => {
            let model = runtime_provider_model_json_for(kind, model_id);
            let status = if model.is_some() { 200 } else { 404 };
            let body = model.unwrap_or_else(|| {
                    serde_json::json!({
                        "error": {
                            "message": format!("model '{model_id}' is not available for {}", runtime_provider_label(kind)),
                            "type": "invalid_request_error",
                            "code": "model_not_found"
                        }
                    })
            });
            Some(runtime_provider_json_response(status, body))
        }
    }
}

pub(super) fn runtime_provider_request_body_with_model(body: &[u8], model: &str) -> Vec<u8> {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return body.to_vec();
    };
    let Some(object) = value.as_object_mut() else {
        return body.to_vec();
    };
    object.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    serde_json::to_vec(&value).unwrap_or_else(|_| body.to_vec())
}

pub(super) fn runtime_provider_model_from_body(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
}

pub(super) fn runtime_provider_model_fallback_chain(
    kind: RuntimeProviderBridgeKind,
    model: &str,
) -> Vec<String> {
    provider_model_fallback_chain(kind.provider_id(), model)
}

pub(super) fn runtime_provider_canonical_model(
    kind: RuntimeProviderBridgeKind,
    model: &str,
) -> String {
    runtime_provider_model_fallback_chain(kind, model)
        .into_iter()
        .next()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| model.to_string())
}

pub(super) fn runtime_provider_gateway_cost_for_request(
    kind: RuntimeProviderBridgeKind,
    aliases: &[runtime_proxy_crate::RuntimeGatewayRouteAlias],
    model_state: &BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>,
    request_id: u64,
    body: &[u8],
    model: &str,
) -> ProviderModelCost {
    if let Some(rewrite) = runtime_proxy_crate::runtime_gateway_rewrite_route_alias_with_state(
        body,
        aliases,
        request_id,
        model_state,
    ) && let Some(alias) = aliases
        .iter()
        .find(|alias| alias.alias.eq_ignore_ascii_case(model))
    {
        if let Some(metrics) = alias.model_metrics.get(&rewrite.model) {
            return ProviderModelCost {
                input_cost_per_million_microusd: metrics.input_cost_per_million_microusd,
                output_cost_per_million_microusd: metrics.output_cost_per_million_microusd,
            };
        }
        if matches!(
            rewrite.strategy,
            runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback
        ) && let Some(first_model) = alias.models.first()
            && let Some(metrics) = alias.model_metrics.get(first_model)
        {
            return ProviderModelCost {
                input_cost_per_million_microusd: metrics.input_cost_per_million_microusd,
                output_cost_per_million_microusd: metrics.output_cost_per_million_microusd,
            };
        }
    }
    provider_model_cost(kind.provider_id(), model)
}

pub(super) fn runtime_provider_error_class(
    kind: RuntimeProviderBridgeKind,
    status: u16,
    body: &[u8],
) -> RuntimeProviderErrorClass {
    let tokens = runtime_provider_error_tokens(body);
    if kind == RuntimeProviderBridgeKind::Gemini
        && status == 403
        && tokens
            .iter()
            .any(|token| token == "permission_denied" || token == "iam_permission_denied")
    {
        return RuntimeProviderErrorClass::Auth;
    }
    for rule in RUNTIME_PROVIDER_ERROR_RULES {
        let status_matches = rule.status.is_none_or(|expected| expected == status);
        if !status_matches {
            continue;
        }
        let code_matches = rule
            .code
            .is_none_or(|code| tokens.iter().any(|token| token == code));
        let text_matches = rule
            .text
            .is_none_or(|text| tokens.iter().any(|token| token.contains(text)));
        if code_matches && text_matches {
            return rule.class;
        }
    }
    if status >= 500 {
        RuntimeProviderErrorClass::Transient
    } else {
        RuntimeProviderErrorClass::Fatal
    }
}

pub(super) fn runtime_provider_error_cooldown_ms(
    class: RuntimeProviderErrorClass,
    status: u16,
    body: &[u8],
) -> u64 {
    let tokens = runtime_provider_error_tokens(body);
    RUNTIME_PROVIDER_ERROR_RULES
        .iter()
        .find(|rule| {
            rule.class == class
                && rule.status.is_none_or(|expected| expected == status)
                && rule
                    .code
                    .is_none_or(|code| tokens.iter().any(|token| token == code))
                && rule
                    .text
                    .is_none_or(|text| tokens.iter().any(|token| token.contains(text)))
        })
        .map(|rule| rule.cooldown_ms)
        .unwrap_or(match class {
            RuntimeProviderErrorClass::Quota => 300_000,
            RuntimeProviderErrorClass::RateLimit => 60_000,
            RuntimeProviderErrorClass::Transient => 10_000,
            RuntimeProviderErrorClass::Auth
            | RuntimeProviderErrorClass::NotFound
            | RuntimeProviderErrorClass::Fatal => 0,
        })
}

pub(super) fn runtime_provider_request_ledger_message(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    body_bytes: usize,
) -> String {
    let contract = runtime_provider_openai_contract(kind);
    runtime_proxy_structured_log_message(
        "local_rewrite_request_detail",
        [
            runtime_proxy_log_field("request", request_id.to_string()),
            runtime_proxy_log_field("provider", runtime_provider_label(kind)),
            runtime_proxy_log_field("path", path_without_query(path_and_query)),
            runtime_proxy_log_field(
                "model",
                model
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("unknown"),
            ),
            runtime_proxy_log_field("status", status.to_string()),
            runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
            runtime_proxy_log_field("body_bytes", body_bytes.to_string()),
            runtime_proxy_log_field(
                "native_passthrough",
                runtime_provider_native_passthrough(kind, path_and_query).to_string(),
            ),
            runtime_proxy_log_field("client_format", contract.client_request_format.label()),
            runtime_proxy_log_field("upstream_format", contract.upstream_request_format.label()),
            runtime_proxy_log_field("response_format", contract.response_format.label()),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_provider_gateway_spend_event(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    request_bytes: usize,
    request_body: &[u8],
    cost: ProviderModelCost,
) -> RuntimeProviderGatewaySpendEvent {
    let model = model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let input_tokens = Some(estimate_request_input_tokens(request_body));
    let cost_usd = calculate_cost_microusd(input_tokens, None, cost).map(microusd_to_usd);
    RuntimeProviderGatewaySpendEvent {
        event: "gateway_spend",
        phase: "request",
        request: request_id,
        call_id: format!("prodex-{request_id}"),
        provider: runtime_provider_label(kind).to_string(),
        path: path_without_query(path_and_query).to_string(),
        model,
        status,
        elapsed_ms,
        request_bytes,
        response_bytes: None,
        input_tokens,
        output_tokens: None,
        cost_usd,
        sink: "runtime-log".to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_provider_gateway_response_spend_event(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    request_body: &[u8],
    response_body: &[u8],
    cost: ProviderModelCost,
) -> RuntimeProviderGatewaySpendEvent {
    let usage = extract_usage_tokens(response_body);
    let output_tokens = usage.output_tokens.or_else(|| {
        (!response_body.is_empty())
            .then(|| estimate_text_tokens(&String::from_utf8_lossy(response_body)))
    });
    runtime_provider_gateway_response_spend_event_from_tokens(
        request_id,
        kind,
        path_and_query,
        model,
        status,
        elapsed_ms,
        request_body,
        response_body.len(),
        usage.input_tokens,
        output_tokens,
        cost,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_provider_gateway_response_spend_event_from_tokens(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    request_body: &[u8],
    response_bytes: usize,
    observed_input_tokens: Option<u64>,
    observed_output_tokens: Option<u64>,
    cost: ProviderModelCost,
) -> RuntimeProviderGatewaySpendEvent {
    let model = model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let input_tokens =
        observed_input_tokens.or_else(|| Some(estimate_request_input_tokens(request_body)));
    let output_tokens = observed_output_tokens;
    let cost_usd = calculate_cost_microusd(input_tokens, output_tokens, cost).map(microusd_to_usd);
    RuntimeProviderGatewaySpendEvent {
        event: "gateway_spend",
        phase: "response",
        request: request_id,
        call_id: format!("prodex-{request_id}"),
        provider: runtime_provider_label(kind).to_string(),
        path: path_without_query(path_and_query).to_string(),
        model,
        status,
        elapsed_ms,
        request_bytes: request_body.len(),
        response_bytes: Some(response_bytes),
        input_tokens,
        output_tokens,
        cost_usd,
        sink: "runtime-log".to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimeProviderGatewaySpendEvent {
    pub(super) event: &'static str,
    pub(super) phase: &'static str,
    pub(super) request: u64,
    pub(super) call_id: String,
    pub(super) provider: String,
    pub(super) path: String,
    pub(super) model: String,
    pub(super) status: u16,
    pub(super) elapsed_ms: u128,
    pub(super) request_bytes: usize,
    pub(super) response_bytes: Option<usize>,
    pub(super) input_tokens: Option<u64>,
    pub(super) output_tokens: Option<u64>,
    pub(super) cost_usd: Option<f64>,
    pub(super) sink: String,
}

impl RuntimeProviderGatewaySpendEvent {
    pub(super) fn log_message(&self) -> String {
        runtime_proxy_structured_log_message(
            "gateway_spend",
            [
                runtime_proxy_log_field("phase", self.phase),
                runtime_proxy_log_field("request", self.request.to_string()),
                runtime_proxy_log_field("call_id", self.call_id.as_str()),
                runtime_proxy_log_field("provider", self.provider.as_str()),
                runtime_proxy_log_field("path", self.path.as_str()),
                runtime_proxy_log_field("model", self.model.as_str()),
                runtime_proxy_log_field("status", self.status.to_string()),
                runtime_proxy_log_field("elapsed_ms", self.elapsed_ms.to_string()),
                runtime_proxy_log_field("request_bytes", self.request_bytes.to_string()),
                runtime_proxy_log_field(
                    "response_bytes",
                    optional_u64_label(self.response_bytes.map(|value| value as u64)),
                ),
                runtime_proxy_log_field("input_tokens", optional_u64_label(self.input_tokens)),
                runtime_proxy_log_field("output_tokens", optional_u64_label(self.output_tokens)),
                runtime_proxy_log_field("cost_usd", optional_f64_label(self.cost_usd)),
                runtime_proxy_log_field("sink", self.sink.as_str()),
            ],
        )
    }
}

fn optional_u64_label(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn optional_f64_label(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.8}"))
        .unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn runtime_provider_should_retry_with_next_model(
    class: RuntimeProviderErrorClass,
) -> bool {
    matches!(
        class,
        RuntimeProviderErrorClass::Quota
            | RuntimeProviderErrorClass::RateLimit
            | RuntimeProviderErrorClass::Transient
            | RuntimeProviderErrorClass::NotFound
    )
}

pub(super) fn runtime_provider_should_rotate_auth_after_response(
    class: RuntimeProviderErrorClass,
) -> bool {
    matches!(
        class,
        RuntimeProviderErrorClass::Auth
            | RuntimeProviderErrorClass::Quota
            | RuntimeProviderErrorClass::RateLimit
            | RuntimeProviderErrorClass::Transient
    )
}

enum RuntimeProviderModelsPath<'a> {
    List,
    Single(&'a str),
}

fn runtime_provider_models_path_suffix(path: &str) -> Option<RuntimeProviderModelsPath<'_>> {
    let path = path.trim_end_matches('/');
    for prefix in ["/v1/models", "/models"] {
        if path == prefix {
            return Some(RuntimeProviderModelsPath::List);
        }
        if let Some(model_id) = path.strip_prefix(&format!("{prefix}/"))
            && !model_id.trim().is_empty()
        {
            return Some(RuntimeProviderModelsPath::Single(model_id));
        }
    }
    None
}

fn runtime_provider_json_response(
    status: u16,
    body: serde_json::Value,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}

fn runtime_provider_error_tokens(body: &[u8]) -> Vec<String> {
    let mut tokens = Vec::new();
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        runtime_provider_error_tokens_from_value(&value, &mut tokens);
    } else if let Ok(text) = std::str::from_utf8(body) {
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(payload) = trimmed.strip_prefix("data:")
                && let Ok(value) = serde_json::from_str::<serde_json::Value>(payload.trim())
            {
                runtime_provider_error_tokens_from_value(&value, &mut tokens);
                continue;
            }
            runtime_provider_push_error_token(&mut tokens, trimmed);
        }
    }
    tokens
}

fn runtime_provider_error_tokens_from_value(value: &serde_json::Value, output: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "code" | "status" | "reason" | "type" | "message" | "detail" | "error"
                ) {
                    match value {
                        serde_json::Value::String(text) => {
                            runtime_provider_push_error_token(output, text)
                        }
                        serde_json::Value::Number(number) => {
                            runtime_provider_push_error_token(output, &number.to_string())
                        }
                        _ => runtime_provider_error_tokens_from_value(value, output),
                    }
                } else {
                    runtime_provider_error_tokens_from_value(value, output);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                runtime_provider_error_tokens_from_value(value, output);
            }
        }
        serde_json::Value::String(text) => runtime_provider_push_error_token(output, text),
        _ => {}
    }
}

fn runtime_provider_push_error_token(output: &mut Vec<String>, value: &str) {
    let token = value.trim().to_ascii_lowercase();
    if token.is_empty() {
        return;
    }
    output.push(token);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_models_endpoint_exposes_catalog_from_gemini_cli() {
        let parts = runtime_provider_models_buffered_response(
            RuntimeProviderBridgeKind::Gemini,
            "GET",
            "/v1/models",
        )
        .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let models = body["data"].as_array().unwrap();

        assert!(models.len() > 1);
        assert!(models.iter().any(|model| model["id"] == "auto"));
        assert!(models.iter().any(|model| model["id"] == "gemini-2.5-pro"));
        assert!(
            models
                .iter()
                .any(|model| model["id"] == "gemini-3.1-pro-preview")
        );
        assert!(models.iter().any(|model| model["id"] == "gemini-3.5-flash"));
        assert!(models.iter().any(|model| model["id"] == "flash"));
    }

    #[test]
    fn deepseek_models_endpoint_exposes_current_and_compat_models() {
        let parts = runtime_provider_models_buffered_response(
            RuntimeProviderBridgeKind::DeepSeek,
            "GET",
            "/models",
        )
        .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let models = body["data"].as_array().unwrap();

        assert!(models.iter().any(|model| model["id"] == "deepseek-v4-pro"));
        assert!(
            models
                .iter()
                .any(|model| model["id"] == "deepseek-v4-flash")
        );
        assert!(models.iter().any(|model| model["id"] == "deepseek-chat"));
    }

    #[test]
    fn anthropic_and_copilot_models_endpoint_expose_provider_catalogs() {
        let anthropic = runtime_provider_models_buffered_response(
            RuntimeProviderBridgeKind::Anthropic,
            "GET",
            "/v1/models",
        )
        .unwrap();
        let anthropic_body: serde_json::Value = serde_json::from_slice(&anthropic.body).unwrap();
        let anthropic_models = anthropic_body["data"].as_array().unwrap();

        assert!(
            anthropic_models
                .iter()
                .any(|model| model["id"] == prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL)
        );
        assert!(anthropic_models.iter().any(|model| model["id"] == "opus"));

        let copilot = runtime_provider_models_buffered_response(
            RuntimeProviderBridgeKind::Copilot,
            "GET",
            "/models",
        )
        .unwrap();
        let copilot_body: serde_json::Value = serde_json::from_slice(&copilot.body).unwrap();
        let copilot_models = copilot_body["data"].as_array().unwrap();

        assert!(
            copilot_models
                .iter()
                .any(|model| model["id"] == prodex_cli::SUPER_COPILOT_DEFAULT_MODEL)
        );
        assert!(copilot_models.iter().any(|model| model["id"] == "codex"));
    }

    #[test]
    fn every_provider_bridge_exposes_openai_client_contract() {
        for kind in [
            RuntimeProviderBridgeKind::OpenAiResponses,
            RuntimeProviderBridgeKind::Anthropic,
            RuntimeProviderBridgeKind::Copilot,
            RuntimeProviderBridgeKind::DeepSeek,
            RuntimeProviderBridgeKind::Gemini,
        ] {
            let contract = runtime_provider_openai_contract(kind);
            assert_eq!(
                contract.client_request_format,
                RuntimeProviderWireFormat::OpenAiResponses
            );
            assert_eq!(
                contract.response_format,
                RuntimeProviderWireFormat::OpenAiResponses
            );
            assert_eq!(contract.canonical_client_endpoint, "/v1/responses");
            assert_eq!(contract.model_list_endpoint, "/v1/models");
            assert!(contract.supports_streaming);
        }
    }

    #[test]
    fn provider_bridge_contract_records_translation_boundary() {
        assert_eq!(
            runtime_provider_openai_contract(RuntimeProviderBridgeKind::OpenAiResponses)
                .upstream_request_format,
            RuntimeProviderWireFormat::OpenAiResponses
        );
        for kind in [
            RuntimeProviderBridgeKind::Anthropic,
            RuntimeProviderBridgeKind::Copilot,
            RuntimeProviderBridgeKind::DeepSeek,
        ] {
            assert_eq!(
                runtime_provider_openai_contract(kind).upstream_request_format,
                RuntimeProviderWireFormat::OpenAiChatCompletions
            );
            assert!(runtime_provider_openai_contract(kind).supports_model_fallback);
        }
        assert_eq!(
            runtime_provider_openai_contract(RuntimeProviderBridgeKind::Gemini)
                .upstream_request_format,
            RuntimeProviderWireFormat::GeminiGenerateContent
        );
    }

    #[test]
    fn provider_model_fallback_supports_aliases_and_combo() {
        assert_eq!(
            runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Gemini, "auto"),
            vec![
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash"
            ]
        );
        assert_eq!(
            runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Gemini, "flash"),
            vec![
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash"
            ]
        );
        assert_eq!(
            runtime_provider_model_fallback_chain(
                RuntimeProviderBridgeKind::Gemini,
                "gemini-3.1-pro-preview-customtools"
            ),
            vec![
                "gemini-3.1-pro-preview-customtools",
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ]
        );
        assert_eq!(
            runtime_provider_model_fallback_chain(
                RuntimeProviderBridgeKind::Gemini,
                "gemini-3-flash-preview"
            ),
            vec![
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash"
            ]
        );
        assert_eq!(
            runtime_provider_model_fallback_chain(
                RuntimeProviderBridgeKind::DeepSeek,
                "combo:deepseek-v4-pro,deepseek-v4-flash,deepseek-v4-pro"
            ),
            vec!["deepseek-v4-pro", "deepseek-v4-flash"]
        );
        assert_eq!(
            runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Anthropic, "sonnet"),
            vec!["claude-sonnet-4-6", "claude-opus-4-8"]
        );
        assert_eq!(
            runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Copilot, "codex"),
            vec!["gpt-5.1-codex", "gpt-5.3-codex", "gpt-5.4"]
        );
    }

    #[test]
    fn provider_error_rules_do_not_treat_generic_429_as_quota() {
        assert_eq!(
            runtime_provider_error_class(
                RuntimeProviderBridgeKind::Gemini,
                429,
                b"too many requests"
            ),
            RuntimeProviderErrorClass::RateLimit
        );
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "status": "RESOURCE_EXHAUSTED",
                "message": "Quota exceeded."
            }
        }))
        .unwrap();
        assert_eq!(
            runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, 429, &body),
            RuntimeProviderErrorClass::Quota
        );
        assert_eq!(
            runtime_provider_error_class(RuntimeProviderBridgeKind::DeepSeek, 401, b"{}"),
            RuntimeProviderErrorClass::Auth
        );
        let permission_body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": "permission_denied",
                "details": [{
                    "reason": "IAM_PERMISSION_DENIED"
                }]
            }
        }))
        .unwrap();
        assert_eq!(
            runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, 403, &permission_body),
            RuntimeProviderErrorClass::Auth
        );
    }

    #[test]
    fn provider_native_passthrough_is_explicit() {
        assert!(runtime_provider_native_passthrough(
            RuntimeProviderBridgeKind::OpenAiResponses,
            "/v1/responses"
        ));
        assert!(!runtime_provider_native_passthrough(
            RuntimeProviderBridgeKind::Gemini,
            "/v1/responses"
        ));
        assert!(runtime_provider_native_passthrough(
            RuntimeProviderBridgeKind::DeepSeek,
            "/v1/chat/completions"
        ));
        assert!(!runtime_provider_native_passthrough(
            RuntimeProviderBridgeKind::Anthropic,
            "/v1/responses"
        ));
        assert!(!runtime_provider_native_passthrough(
            RuntimeProviderBridgeKind::Copilot,
            "/v1/responses"
        ));
        assert!(runtime_provider_native_passthrough(
            RuntimeProviderBridgeKind::Copilot,
            "/v1/chat/completions"
        ));
    }

    #[test]
    fn gateway_spend_message_includes_stable_call_fields() {
        let event = runtime_provider_gateway_spend_event(
            42,
            RuntimeProviderBridgeKind::Gemini,
            "/v1/responses?trace=1",
            Some("prodex-fast"),
            200,
            123,
            456,
            br#"{"model":"prodex-fast","input":"hello from prodex"}"#,
            ProviderModelCost::default(),
        );
        let message = event.log_message();

        let fields = runtime_proxy_crate::runtime_proxy_log_fields(&message);
        assert_eq!(
            runtime_proxy_crate::runtime_proxy_log_event(&message),
            Some("gateway_spend")
        );
        assert_eq!(fields.get("phase").map(String::as_str), Some("request"));
        assert_eq!(fields.get("call_id").map(String::as_str), Some("prodex-42"));
        assert_eq!(fields.get("provider").map(String::as_str), Some("gemini"));
        assert_eq!(
            fields.get("path").map(String::as_str),
            Some("/v1/responses")
        );
        assert_eq!(fields.get("model").map(String::as_str), Some("prodex-fast"));
        assert_eq!(fields.get("status").map(String::as_str), Some("200"));
        assert_eq!(fields.get("request_bytes").map(String::as_str), Some("456"));
        assert_ne!(
            fields.get("input_tokens").map(String::as_str),
            Some("unknown")
        );
        assert_eq!(fields.get("cost_usd").map(String::as_str), Some("unknown"));

        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "gateway_spend");
        assert_eq!(json["phase"], "request");
        assert_eq!(json["call_id"], "prodex-42");
        assert_eq!(json["response_bytes"], serde_json::Value::Null);
    }

    #[test]
    fn gateway_response_spend_uses_response_usage_and_total_cost() {
        let response_body =
            br#"{"id":"resp","usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#;
        let event = runtime_provider_gateway_response_spend_event(
            9,
            RuntimeProviderBridgeKind::OpenAiResponses,
            "/v1/responses",
            Some("gpt-5.4"),
            200,
            12,
            br#"{"model":"gpt-5.4","input":"hello"}"#,
            response_body,
            ProviderModelCost {
                input_cost_per_million_microusd: Some(1_000_000),
                output_cost_per_million_microusd: Some(2_000_000),
            },
        );

        assert_eq!(event.phase, "response");
        assert_eq!(event.response_bytes, Some(response_body.len()));
        assert_eq!(event.input_tokens, Some(7));
        assert_eq!(event.output_tokens, Some(11));
        assert_eq!(event.cost_usd, Some(0.000029));
    }
}
