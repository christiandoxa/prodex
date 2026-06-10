use super::provider_models::{
    runtime_provider_model_catalog_json, runtime_provider_model_json_for,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_runtime_gemini::gemini_model_fallback_chain;
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeProviderBridgeKind {
    Anthropic,
    Copilot,
    OpenAiResponses,
    DeepSeek,
    Gemini,
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
    match kind {
        RuntimeProviderBridgeKind::Anthropic => "anthropic",
        RuntimeProviderBridgeKind::Copilot => "copilot",
        RuntimeProviderBridgeKind::OpenAiResponses => "openai",
        RuntimeProviderBridgeKind::DeepSeek => "deepseek",
        RuntimeProviderBridgeKind::Gemini => "gemini",
    }
}

pub(super) fn runtime_provider_openai_contract(
    kind: RuntimeProviderBridgeKind,
) -> RuntimeProviderOpenAiContract {
    match kind {
        RuntimeProviderBridgeKind::OpenAiResponses => RuntimeProviderOpenAiContract {
            client_request_format: RuntimeProviderWireFormat::OpenAiResponses,
            upstream_request_format: RuntimeProviderWireFormat::OpenAiResponses,
            response_format: RuntimeProviderWireFormat::OpenAiResponses,
            canonical_client_endpoint: "/v1/responses",
            model_list_endpoint: "/v1/models",
            supports_streaming: true,
            supports_model_fallback: false,
        },
        RuntimeProviderBridgeKind::Anthropic
        | RuntimeProviderBridgeKind::Copilot
        | RuntimeProviderBridgeKind::DeepSeek => RuntimeProviderOpenAiContract {
            client_request_format: RuntimeProviderWireFormat::OpenAiResponses,
            upstream_request_format: RuntimeProviderWireFormat::OpenAiChatCompletions,
            response_format: RuntimeProviderWireFormat::OpenAiResponses,
            canonical_client_endpoint: "/v1/responses",
            model_list_endpoint: "/v1/models",
            supports_streaming: true,
            supports_model_fallback: true,
        },
        RuntimeProviderBridgeKind::Gemini => RuntimeProviderOpenAiContract {
            client_request_format: RuntimeProviderWireFormat::OpenAiResponses,
            upstream_request_format: RuntimeProviderWireFormat::GeminiGenerateContent,
            response_format: RuntimeProviderWireFormat::OpenAiResponses,
            canonical_client_endpoint: "/v1/responses",
            model_list_endpoint: "/v1/models",
            supports_streaming: true,
            supports_model_fallback: true,
        },
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
    let model = model.trim();
    if let Some(chain) = runtime_provider_combo_chain(model) {
        return chain;
    }
    let lower = model.to_ascii_lowercase();
    let chain: &[&str] = match kind {
        RuntimeProviderBridgeKind::Anthropic => match lower.as_str() {
            "" | "auto" | "default" => &[
                prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
                "claude-opus-4-8",
                "claude-haiku-4-5",
            ],
            "opus" | "best" => &["claude-opus-4-8", "claude-sonnet-4-6"],
            "sonnet" | "pro" => &["claude-sonnet-4-6", "claude-opus-4-8"],
            "haiku" | "flash" => &["claude-haiku-4-5", "claude-sonnet-4-6"],
            _ => return vec![model.to_string()],
        },
        RuntimeProviderBridgeKind::Copilot => match lower.as_str() {
            "" | "auto" | "default" => &[
                prodex_cli::SUPER_COPILOT_DEFAULT_MODEL,
                "gpt-5.4",
                "gpt-5.3-codex",
            ],
            "codex" | "pro" => &["gpt-5.1-codex", "gpt-5.3-codex", "gpt-5.4"],
            "claude" | "sonnet" => &["claude-sonnet-4-6", "gpt-5.1-codex"],
            "gemini" => &["gemini-3.1-pro-preview", "gpt-5.1-codex"],
            _ => return vec![model.to_string()],
        },
        RuntimeProviderBridgeKind::Gemini => return gemini_model_fallback_chain(model),
        RuntimeProviderBridgeKind::DeepSeek => match lower.as_str() {
            "" | "auto" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "pro" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "flash" => &["deepseek-v4-flash", "deepseek-v4-pro"],
            _ => return vec![model.to_string()],
        },
        RuntimeProviderBridgeKind::OpenAiResponses => {
            if model.is_empty() {
                return Vec::new();
            }
            return vec![model.to_string()];
        }
    };
    runtime_provider_dedup_chain(chain.iter().map(|value| (*value).to_string()).collect())
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

fn runtime_provider_combo_chain(model: &str) -> Option<Vec<String>> {
    let chain = model.trim().strip_prefix("combo:")?;
    let models = chain
        .split([',', ';', '|', '>'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!models.is_empty()).then(|| runtime_provider_dedup_chain(models))
}

fn runtime_provider_dedup_chain(models: Vec<String>) -> Vec<String> {
    let mut output = Vec::new();
    for model in models {
        if !output.iter().any(|existing| existing == &model) {
            output.push(model);
        }
    }
    output
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
}
