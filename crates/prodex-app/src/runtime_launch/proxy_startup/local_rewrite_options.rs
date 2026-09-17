use super::anthropic_rewrite::RuntimeAnthropicProviderAuth;
use super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode;
use super::gemini_rewrite::RuntimeGeminiProviderAuth;
use super::local_rewrite_copilot::RuntimeCopilotProviderAuth;
use super::local_rewrite_kiro::RuntimeKiroProfileAuth;
use super::provider_bridge::RuntimeProviderBridgeKind;
use prodex_core::AppPaths;
use prodex_state::AppState;

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
    DeepSeek {
        api_keys: Vec<String>,
        strict_tools: bool,
        beta_base_url: String,
        web_search_mode: RuntimeDeepSeekWebSearchMode,
    },
    Gemini {
        auth: RuntimeGeminiProviderAuth,
        thinking_budget_tokens: Option<u64>,
        model_resolution: crate::RuntimeGeminiModelResolution,
    },
    Kiro {
        auth: RuntimeKiroProfileAuth,
    },
}

impl RuntimeLocalRewriteProviderOptions {
    pub(super) fn bridge_kind(&self) -> RuntimeProviderBridgeKind {
        match self {
            Self::Anthropic { .. } => RuntimeProviderBridgeKind::Anthropic,
            Self::Copilot { .. } => RuntimeProviderBridgeKind::Copilot,
            Self::OpenAiResponses { .. } => RuntimeProviderBridgeKind::OpenAiResponses,
            Self::DeepSeek { .. } => RuntimeProviderBridgeKind::DeepSeek,
            Self::Gemini { .. } => RuntimeProviderBridgeKind::Gemini,
            Self::Kiro { .. } => RuntimeProviderBridgeKind::Kiro,
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
}
