use super::provider_models::{
    runtime_provider_model_catalog_json, runtime_provider_model_json_for,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_provider_core::{
    ProviderAdapterContract, ProviderEndpoint, ProviderErrorClass, ProviderErrorClassification,
    ProviderId, ProviderModelCost, ProviderTransformInput, ProviderTransformLoss,
    ProviderTransformResult, ProviderWireFormat, provider_adapter, provider_model_cost,
    provider_model_fallback_chain, provider_translator,
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
    Kiro,
}

impl RuntimeProviderBridgeKind {
    pub(super) fn provider_id(self) -> ProviderId {
        match self {
            Self::Anthropic => ProviderId::Anthropic,
            Self::Copilot => ProviderId::Copilot,
            Self::OpenAiResponses => ProviderId::OpenAi,
            Self::DeepSeek => ProviderId::DeepSeek,
            Self::Gemini => ProviderId::Gemini,
            Self::Kiro => ProviderId::Kiro,
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
        RuntimeProviderBridgeKind::Kiro => {
            !(path.ends_with("/responses")
                || path.ends_with("/responses/compact")
                || path.ends_with("/chat/completions")
                || runtime_provider_models_path_suffix(path).is_some())
        }
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

pub(super) fn runtime_provider_request_conformance_result(
    kind: RuntimeProviderBridgeKind,
    request: &crate::RuntimeProxyRequest,
    body: &[u8],
) -> Option<ProviderTransformResult> {
    let path = path_without_query(&request.path_and_query);
    if !(path.ends_with("/responses") || path.ends_with("/responses/compact")) {
        return None;
    }
    let mut input = ProviderTransformInput::new(ProviderEndpoint::Responses, body.to_vec());
    input.model = runtime_provider_model_from_body(body);
    input.headers = request.headers.iter().cloned().collect();
    Some(provider_translator(kind.provider_id()).transform_request(input))
}

pub(super) fn runtime_provider_log_request_conformance(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    result: &ProviderTransformResult,
) {
    let (loss, reason) = match &result.loss {
        ProviderTransformLoss::Lossless => return,
        ProviderTransformLoss::DegradedButSafe { reason, .. } => ("degraded", reason.as_str()),
        ProviderTransformLoss::Rejected { reason } => ("rejected", reason.as_str()),
        ProviderTransformLoss::UnsupportedUpstream { reason } => ("unsupported", reason.as_str()),
    };
    crate::runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_conformance_request",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", runtime_provider_label(kind)),
                runtime_proxy_log_field("endpoint", result.endpoint.label()),
                runtime_proxy_log_field("loss", loss),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
}

pub(super) fn runtime_provider_response_conformance_result(
    kind: RuntimeProviderBridgeKind,
    status: u16,
    body: &[u8],
) -> Option<ProviderTransformResult> {
    if !(200..300).contains(&status) {
        return None;
    }
    let mut input = ProviderTransformInput::new(ProviderEndpoint::Responses, body.to_vec());
    input.status = Some(status);
    Some(provider_translator(kind.provider_id()).transform_response(input))
}

pub(super) fn runtime_provider_log_response_conformance(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    result: &ProviderTransformResult,
) {
    let (loss, reason) = match &result.loss {
        ProviderTransformLoss::Lossless => return,
        ProviderTransformLoss::DegradedButSafe { reason, .. } => ("degraded", reason.as_str()),
        ProviderTransformLoss::Rejected { reason } => ("rejected", reason.as_str()),
        ProviderTransformLoss::UnsupportedUpstream { reason } => ("unsupported", reason.as_str()),
    };
    crate::runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_conformance_response",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", runtime_provider_label(kind)),
                runtime_proxy_log_field("endpoint", result.endpoint.label()),
                runtime_proxy_log_field("loss", loss),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
}

pub(super) fn runtime_provider_stream_event_conformance_result(
    kind: RuntimeProviderBridgeKind,
    body: &[u8],
) -> ProviderTransformResult {
    provider_translator(kind.provider_id()).transform_stream_event(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        body.to_vec(),
    ))
}

pub(super) fn runtime_provider_stream_text_delta_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Option<(String, serde_json::Value)> {
    let mut event =
        runtime_provider_stream_base_event(kind, upstream_value, "response.output_text.delta")?;
    let object = event.1.as_object_mut()?;
    object.insert(
        "sequence_number".to_string(),
        serde_json::Value::from(sequence_number),
    );
    object.insert(
        "created_at".to_string(),
        serde_json::Value::from(created_at),
    );
    object.insert(
        "response_id".to_string(),
        serde_json::Value::String(response_id.to_string()),
    );
    Some(event)
}

pub(super) fn runtime_provider_stream_function_call_arguments_delta_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    sequence_number: u64,
) -> Option<(String, serde_json::Value)> {
    let mut event = runtime_provider_stream_base_event(
        kind,
        upstream_value,
        "response.function_call_arguments.delta",
    )?;
    event.1.as_object_mut()?.insert(
        "sequence_number".to_string(),
        serde_json::Value::from(sequence_number),
    );
    Some(event)
}

pub(super) fn runtime_provider_stream_reasoning_summary_text_delta_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    sequence_number: u64,
    response_id: &str,
    summary_index: u64,
) -> Option<(String, serde_json::Value)> {
    let mut event = runtime_provider_stream_base_event(
        kind,
        upstream_value,
        "response.reasoning_summary_text.delta",
    )?;
    let object = event.1.as_object_mut()?;
    object.insert(
        "sequence_number".to_string(),
        serde_json::Value::from(sequence_number),
    );
    object.insert(
        "response_id".to_string(),
        serde_json::Value::String(response_id.to_string()),
    );
    object.insert(
        "summary_index".to_string(),
        serde_json::Value::from(summary_index),
    );
    Some(event)
}

fn runtime_provider_stream_base_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    expected_event_name: &str,
) -> Option<(String, serde_json::Value)> {
    let body = format!("data: {upstream_value}\n\n");
    let result = runtime_provider_stream_event_conformance_result(kind, body.as_bytes());
    if !matches!(result.loss, ProviderTransformLoss::Lossless) {
        return None;
    }
    let body = result.body.as_ref()?;
    let event = String::from_utf8_lossy(body);
    let event_name = event
        .strip_prefix("event: ")?
        .split_once('\n')?
        .0
        .trim()
        .to_string();
    if event_name != expected_event_name {
        return None;
    }
    let payload = event
        .split_once("data: ")?
        .1
        .trim()
        .strip_suffix('\n')
        .unwrap_or(event.split_once("data: ")?.1.trim());
    let data = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    Some((event_name, data))
}

pub(super) fn runtime_provider_log_stream_conformance(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    result: &ProviderTransformResult,
) {
    let (loss, reason) = match &result.loss {
        ProviderTransformLoss::Lossless => return,
        ProviderTransformLoss::DegradedButSafe { reason, .. } => ("degraded", reason.as_str()),
        ProviderTransformLoss::Rejected { reason } => ("rejected", reason.as_str()),
        ProviderTransformLoss::UnsupportedUpstream { reason } => ("unsupported", reason.as_str()),
    };
    crate::runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_conformance_stream",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", runtime_provider_label(kind)),
                runtime_proxy_log_field("endpoint", result.endpoint.label()),
                runtime_proxy_log_field("loss", loss),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
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
    runtime_provider_error_classification(kind, status, body).0
}

pub(super) fn runtime_provider_error_cooldown_ms(
    class: RuntimeProviderErrorClass,
    status: u16,
    body: &[u8],
) -> u64 {
    let (derived_class, classification) =
        runtime_provider_error_classification(kind_from_class_hint(class), status, body);
    if derived_class == class {
        classification.cooldown_ms
    } else {
        match class {
            RuntimeProviderErrorClass::Quota => 300_000,
            RuntimeProviderErrorClass::RateLimit => 60_000,
            RuntimeProviderErrorClass::Transient => 10_000,
            RuntimeProviderErrorClass::Auth
            | RuntimeProviderErrorClass::NotFound
            | RuntimeProviderErrorClass::Fatal => 0,
        }
    }
}

fn kind_from_class_hint(class: RuntimeProviderErrorClass) -> RuntimeProviderBridgeKind {
    match class {
        RuntimeProviderErrorClass::Auth
        | RuntimeProviderErrorClass::Quota
        | RuntimeProviderErrorClass::RateLimit
        | RuntimeProviderErrorClass::Transient
        | RuntimeProviderErrorClass::NotFound
        | RuntimeProviderErrorClass::Fatal => RuntimeProviderBridgeKind::OpenAiResponses,
    }
}

fn runtime_provider_error_classification(
    kind: RuntimeProviderBridgeKind,
    status: u16,
    body: &[u8],
) -> (RuntimeProviderErrorClass, ProviderErrorClassification) {
    let translator = provider_translator(kind.provider_id());
    let text = std::str::from_utf8(body).ok();
    let mut best = translator.classify_error(Some(status), None, text);
    for token in runtime_provider_error_tokens(body) {
        let candidate = translator.classify_error(Some(status), Some(&token), Some(&token));
        if runtime_provider_error_classification_rank(candidate.class)
            < runtime_provider_error_classification_rank(best.class)
        {
            best = candidate;
        }
    }
    (
        runtime_provider_error_class_from_core(best.class, status),
        best,
    )
}

fn runtime_provider_error_class_from_core(
    class: ProviderErrorClass,
    status: u16,
) -> RuntimeProviderErrorClass {
    match class {
        ProviderErrorClass::Auth => RuntimeProviderErrorClass::Auth,
        ProviderErrorClass::Quota => RuntimeProviderErrorClass::Quota,
        ProviderErrorClass::RateLimit => RuntimeProviderErrorClass::RateLimit,
        ProviderErrorClass::Transient => RuntimeProviderErrorClass::Transient,
        ProviderErrorClass::NotFound => RuntimeProviderErrorClass::NotFound,
        ProviderErrorClass::Other => {
            if status >= 500 {
                RuntimeProviderErrorClass::Transient
            } else {
                RuntimeProviderErrorClass::Fatal
            }
        }
    }
}

fn runtime_provider_error_classification_rank(class: ProviderErrorClass) -> u8 {
    match class {
        ProviderErrorClass::Auth => 0,
        ProviderErrorClass::Quota => 1,
        ProviderErrorClass::RateLimit => 2,
        ProviderErrorClass::NotFound => 3,
        ProviderErrorClass::Transient => 4,
        ProviderErrorClass::Other => 5,
    }
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
