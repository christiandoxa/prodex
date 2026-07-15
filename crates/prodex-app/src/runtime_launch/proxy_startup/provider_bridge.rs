#[path = "provider_bridge_conformance.rs"]
mod provider_bridge_conformance;
#[path = "provider_bridge_error_policy.rs"]
mod provider_bridge_error_policy;
#[path = "provider_bridge_routing.rs"]
mod provider_bridge_routing;
pub(super) use prodex_provider_core::ProviderErrorClass as RuntimeProviderErrorClass;
#[cfg(test)]
use prodex_provider_core::ProviderTransformLoss;
use prodex_provider_core::{
    ProviderAdapterContract, ProviderId, ProviderWireFormat, provider_adapter,
};

pub(super) use self::provider_bridge_conformance::{
    runtime_harness_log_provider_policy, runtime_provider_log_request_conformance,
    runtime_provider_log_response_conformance, runtime_provider_log_stream_conformance,
    runtime_provider_request_conformance_result, runtime_provider_response_conformance_result,
    runtime_provider_stream_event_conformance_result,
    runtime_provider_stream_function_call_arguments_delta_event,
    runtime_provider_stream_reasoning_summary_text_delta_event,
    runtime_provider_stream_text_delta_event,
};
pub(super) use self::provider_bridge_error_policy::{
    runtime_provider_error_class, runtime_provider_error_cooldown_ms,
};
#[cfg(test)]
pub(super) use self::provider_bridge_routing::runtime_provider_native_passthrough;
pub(super) use self::provider_bridge_routing::{
    RuntimeProviderRouteKind, runtime_provider_canonical_model,
    runtime_provider_gateway_cost_for_request, runtime_provider_model_fallback_chain,
    runtime_provider_models_buffered_response, runtime_provider_request_body_with_model,
    runtime_provider_request_ledger_message, runtime_provider_route_kind,
};
pub(super) use super::provider_bridge_spend::{
    RuntimeProviderGatewaySpendEvent, runtime_provider_gateway_response_spend_event,
    runtime_provider_gateway_response_spend_event_from_tokens,
    runtime_provider_gateway_spend_apply_admission_ids,
    runtime_provider_gateway_spend_apply_ledger_scope, runtime_provider_gateway_spend_event,
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

    pub(super) fn rate_limit_header_prefix(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Copilot => "copilot",
            Self::OpenAiResponses => "openai",
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
            Self::Kiro => "kiro",
        }
    }

    pub(super) fn rate_limit_header_label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::Copilot => "Copilot",
            Self::OpenAiResponses => "OpenAI",
            Self::DeepSeek => "DeepSeek",
            Self::Gemini => "Google Gemini",
            Self::Kiro => "Kiro",
        }
    }

    pub(super) fn chat_compatible_adapter_label(self) -> &'static str {
        match self {
            Self::Gemini => "Gemini OpenAI-compatible",
            Self::OpenAiResponses => "OpenAI-compatible",
            Self::Anthropic => "Anthropic",
            Self::Copilot => "Copilot",
            Self::DeepSeek => "DeepSeek",
            Self::Kiro => "Kiro",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeProviderWireFormat {
    OpenAiResponses,
    OpenAiChatCompletions,
    AnthropicMessages,
    GeminiGenerateContent,
}

impl RuntimeProviderWireFormat {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai-responses",
            Self::OpenAiChatCompletions => "openai-chat-completions",
            Self::AnthropicMessages => "anthropic-messages",
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
        ProviderWireFormat::AnthropicMessages => RuntimeProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::Passthrough => RuntimeProviderWireFormat::OpenAiResponses,
    }
}

pub(super) fn runtime_provider_model_from_body(body: &[u8]) -> Option<String> {
    prodex_provider_core::provider_model_from_request_body(body)
}

#[cfg(test)]
#[path = "provider_bridge_tests.rs"]
mod provider_bridge_tests;
