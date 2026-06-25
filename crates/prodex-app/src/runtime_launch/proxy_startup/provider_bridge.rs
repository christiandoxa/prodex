use super::provider_models::{
    runtime_provider_model_catalog_json, runtime_provider_model_json_for,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_provider_core::{
    ProviderAdapterContract, ProviderId, ProviderModelCost, ProviderWireFormat, provider_adapter,
    provider_model_cost, provider_model_fallback_chain,
};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::collections::BTreeMap;

pub(super) use super::provider_bridge_spend::{
    RuntimeProviderGatewaySpendEvent, runtime_provider_gateway_response_spend_event,
    runtime_provider_gateway_response_spend_event_from_tokens,
    runtime_provider_gateway_spend_event,
};

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
    dynamic_catalog: Option<&[serde_json::Value]>,
    method: &str,
    path_and_query: &str,
) -> Option<RuntimeHeapTrimmedBufferedResponseParts> {
    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }
    let models = runtime_provider_model_catalog_json(kind, dynamic_catalog);
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
            let model = runtime_provider_model_json_for(kind, dynamic_catalog, model_id);
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
#[path = "provider_bridge_tests.rs"]
mod provider_bridge_tests;
