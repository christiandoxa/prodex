use super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
};
use super::provider_bridge::RuntimeProviderBridgeKind;
use super::*;

#[derive(Clone)]
pub(crate) enum RuntimeLocalRewriteProviderOptions {
    Anthropic {
        auth: RuntimeAnthropicProviderAuth,
    },
    Copilot {
        auth: RuntimeCopilotProviderAuth,
    },
    OpenAiResponses {
        api_keys: Vec<String>,
    },
    LocalEmbeddingsOnly {
        embedding_model: String,
    },
    DeepSeek {
        api_keys: Vec<String>,
    },
    Gemini {
        auth: RuntimeGeminiProviderAuth,
        thinking_budget_tokens: Option<u64>,
        model_resolution: crate::RuntimeGeminiModelResolution,
    },
}

impl RuntimeLocalRewriteProviderOptions {
    pub(super) fn bridge_kind(&self) -> RuntimeProviderBridgeKind {
        match self {
            RuntimeLocalRewriteProviderOptions::Anthropic { .. } => {
                RuntimeProviderBridgeKind::Anthropic
            }
            RuntimeLocalRewriteProviderOptions::Copilot { .. } => {
                RuntimeProviderBridgeKind::Copilot
            }
            RuntimeLocalRewriteProviderOptions::OpenAiResponses { .. }
            | RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly { .. } => {
                RuntimeProviderBridgeKind::OpenAiResponses
            }
            RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => {
                RuntimeProviderBridgeKind::DeepSeek
            }
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => RuntimeProviderBridgeKind::Gemini,
        }
    }
}

pub(crate) struct RuntimeLocalRewriteProxyStartOptions<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) state: &'a AppState,
    pub(crate) upstream_base_url: String,
    pub(crate) provider: RuntimeLocalRewriteProviderOptions,
    pub(crate) upstream_no_proxy: bool,
    pub(crate) smart_context_enabled: bool,
    pub(crate) presidio_redaction_enabled: bool,
    pub(crate) model_context_window_tokens: Option<u64>,
    pub(crate) preferred_listen_addr: Option<&'a str>,
    pub(crate) gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(crate) gateway_sso: RuntimeGatewaySsoConfig,
    pub(crate) gateway_state_store: RuntimeGatewayStateStore,
    pub(crate) gateway_virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    pub(crate) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(crate) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(crate) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(crate) gateway_call_id_header: Option<String>,
    pub(crate) gateway_observability: RuntimeGatewayObservabilityConfig,
}
