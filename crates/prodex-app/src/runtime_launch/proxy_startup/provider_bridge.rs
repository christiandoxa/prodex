#[path = "provider_bridge_conformance.rs"]
mod provider_bridge_conformance;
#[path = "provider_bridge_error_policy.rs"]
mod provider_bridge_error_policy;
#[path = "provider_bridge_routing.rs"]
mod provider_bridge_routing;
pub(super) use prodex_provider_core::ProviderErrorClass as RuntimeProviderErrorClass;
use prodex_provider_core::ProviderId;

pub(super) use self::provider_bridge_conformance::{
    runtime_provider_log_request_conformance, runtime_provider_log_response_conformance,
    runtime_provider_log_stream_conformance, runtime_provider_request_conformance_result,
    runtime_provider_response_conformance_result, runtime_provider_stream_event_conformance_result,
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
    runtime_provider_model_fallback_chain, runtime_provider_models_buffered_response,
    runtime_provider_request_body_with_model, runtime_provider_request_ledger_message,
    runtime_provider_route_endpoint, runtime_provider_route_kind,
};

#[repr(i64)]
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
        prodex_mojo_core::provider_constraints::provider_bridge_rate_limit_header_prefix(
            self as i64,
        )
        .expect("Mojo provider bridge rate-limit prefix returned invalid output")
    }

    pub(super) fn rate_limit_header_label(self) -> &'static str {
        prodex_mojo_core::provider_constraints::provider_bridge_rate_limit_header_label(self as i64)
            .expect("Mojo provider bridge rate-limit label returned invalid output")
    }

    pub(super) fn chat_compatible_adapter_label(self) -> &'static str {
        prodex_mojo_core::provider_constraints::provider_bridge_chat_compatible_adapter_label(
            self as i64,
        )
        .expect("Mojo provider bridge adapter label returned invalid output")
    }

    pub(super) fn function_tool_name_max_bytes(self) -> usize {
        prodex_mojo_core::provider_constraints::provider_bridge_function_tool_name_max_bytes(
            self as i64,
        )
        .expect("Mojo provider bridge tool-name limit returned invalid output")
    }
}

pub(super) fn runtime_provider_label(kind: RuntimeProviderBridgeKind) -> &'static str {
    kind.provider_id().label()
}

pub(super) fn runtime_provider_model_from_body(body: &[u8]) -> Option<String> {
    prodex_provider_core::provider_model_from_request_body(body)
}

#[cfg(test)]
mod tests;
