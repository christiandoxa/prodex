use serde::Serialize;

pub const PRODEX_ANTHROPIC_DEFAULT_MODEL: &str = "claude-sonnet-4-6";
pub const PRODEX_COPILOT_DEFAULT_MODEL: &str = "gpt-5.1-codex";
pub const PRODEX_GEMINI_DEFAULT_MODEL: &str = "auto";
pub const PRODEX_GEMINI_CHAT_COMPRESSION_MODEL: &str = "chat-compression-default";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderId {
    OpenAi,
    Anthropic,
    Copilot,
    DeepSeek,
    Gemini,
    Local,
}

impl ProviderId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Copilot => "copilot",
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
            Self::Local => "local",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" | "openai-responses" | "openai_compatible" | "openai-compatible" => {
                Some(Self::OpenAi)
            }
            "anthropic" | "claude" => Some(Self::Anthropic),
            "copilot" | "github-copilot" | "github_copilot" => Some(Self::Copilot),
            "deepseek" => Some(Self::DeepSeek),
            "gemini" | "google" => Some(Self::Gemini),
            "local" | "local-openai" | "local_openai" => Some(Self::Local),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderWireFormat {
    OpenAiResponses,
    OpenAiChatCompletions,
    AnthropicMessages,
    GeminiGenerateContent,
    Passthrough,
}

impl ProviderWireFormat {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai-responses",
            Self::OpenAiChatCompletions => "openai-chat-completions",
            Self::AnthropicMessages => "anthropic-messages",
            Self::GeminiGenerateContent => "gemini-generate-content",
            Self::Passthrough => "passthrough",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderEndpoint {
    Responses,
    ChatCompletions,
    Messages,
    Models,
    Embeddings,
    Images,
    Audio,
    Batches,
    Rerank,
    A2a,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ProviderModelCost {
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
}

impl ProviderModelCost {
    pub const fn any(self) -> bool {
        self.input_cost_per_million_microusd.is_some()
            || self.output_cost_per_million_microusd.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProviderModelSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub provider: ProviderId,
    pub owned_by: &'static str,
    pub context_window_tokens: Option<u64>,
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
    pub endpoints: &'static [ProviderEndpoint],
    pub aliases: &'static [&'static str],
}

impl ProviderModelSpec {
    pub const fn cost(self) -> ProviderModelCost {
        ProviderModelCost {
            input_cost_per_million_microusd: self.input_cost_per_million_microusd,
            output_cost_per_million_microusd: self.output_cost_per_million_microusd,
        }
    }

    pub fn matches_id_or_alias(self, model: &str) -> bool {
        let model = model.trim();
        self.id.eq_ignore_ascii_case(model)
            || self
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(model))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderErrorClass {
    Auth,
    Quota,
    RateLimit,
    Transient,
    NotFound,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderErrorClassification {
    pub class: ProviderErrorClass,
    pub cooldown_ms: u64,
}

pub trait ProviderAdapterContract {
    fn provider(&self) -> ProviderId;
    fn client_request_format(&self) -> ProviderWireFormat;
    fn upstream_request_format(&self) -> ProviderWireFormat;
    fn response_format(&self) -> ProviderWireFormat;
    fn canonical_client_endpoint(&self) -> &'static str;
    fn model_list_endpoint(&self) -> &'static str;
    fn supports_streaming(&self) -> bool;
    fn supports_model_fallback(&self) -> bool;
    fn supported_endpoints(&self) -> &'static [ProviderEndpoint];
    fn model_catalog(&self) -> &'static [ProviderModelSpec];

    fn fallback_chain(&self, model: &str) -> Vec<String> {
        provider_model_fallback_chain(self.provider(), model)
    }

    fn classify_error(
        &self,
        status: Option<u16>,
        code: Option<&str>,
        text: Option<&str>,
    ) -> ProviderErrorClassification {
        classify_provider_error(status, code, text)
    }

    fn estimate_input_tokens(&self, body: &[u8]) -> u64 {
        estimate_request_input_tokens(body)
    }

    fn transform_request_body(&self, body: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::ClientRequestToUpstream,
            provider: self.provider(),
            from_format: self.client_request_format(),
            to_format: self.upstream_request_format(),
            body: body.to_vec(),
            lossy: false,
        }
    }

    fn transform_response_body(&self, body: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::UpstreamResponseToClient,
            provider: self.provider(),
            from_format: self.upstream_request_format(),
            to_format: self.response_format(),
            body: body.to_vec(),
            lossy: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTransformPhase {
    ClientRequestToUpstream,
    UpstreamResponseToClient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderBodyTransform {
    pub phase: ProviderTransformPhase,
    pub provider: ProviderId,
    pub from_format: ProviderWireFormat,
    pub to_format: ProviderWireFormat,
    pub body: Vec<u8>,
    pub lossy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticProviderAdapter {
    provider: ProviderId,
}

impl StaticProviderAdapter {
    pub const fn new(provider: ProviderId) -> Self {
        Self { provider }
    }
}

impl ProviderAdapterContract for StaticProviderAdapter {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    fn client_request_format(&self) -> ProviderWireFormat {
        match self.provider {
            ProviderId::OpenAi | ProviderId::Local => ProviderWireFormat::OpenAiResponses,
            ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => {
                ProviderWireFormat::OpenAiResponses
            }
            ProviderId::Gemini => ProviderWireFormat::OpenAiResponses,
        }
    }

    fn upstream_request_format(&self) -> ProviderWireFormat {
        match self.provider {
            ProviderId::OpenAi | ProviderId::Local => ProviderWireFormat::OpenAiResponses,
            ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => {
                ProviderWireFormat::OpenAiChatCompletions
            }
            ProviderId::Gemini => ProviderWireFormat::GeminiGenerateContent,
        }
    }

    fn response_format(&self) -> ProviderWireFormat {
        match self.provider {
            ProviderId::OpenAi | ProviderId::Local => ProviderWireFormat::OpenAiResponses,
            ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => {
                ProviderWireFormat::OpenAiResponses
            }
            ProviderId::Gemini => ProviderWireFormat::OpenAiResponses,
        }
    }

    fn canonical_client_endpoint(&self) -> &'static str {
        "/v1/responses"
    }

    fn model_list_endpoint(&self) -> &'static str {
        "/v1/models"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_model_fallback(&self) -> bool {
        !matches!(self.provider, ProviderId::OpenAi | ProviderId::Local)
    }

    fn supported_endpoints(&self) -> &'static [ProviderEndpoint] {
        provider_supported_endpoints(self.provider)
    }

    fn model_catalog(&self) -> &'static [ProviderModelSpec] {
        provider_model_catalog(self.provider)
    }
}

pub fn provider_adapter(provider: ProviderId) -> StaticProviderAdapter {
    StaticProviderAdapter::new(provider)
}

const CORE_TEXT_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Models,
];

const OPENAI_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Models,
    ProviderEndpoint::Embeddings,
    ProviderEndpoint::Images,
    ProviderEndpoint::Audio,
    ProviderEndpoint::Batches,
    ProviderEndpoint::Rerank,
    ProviderEndpoint::A2a,
];

const GEMINI_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Models,
    ProviderEndpoint::Embeddings,
];

const OPENAI_CONTEXT_WINDOW_TOKENS: u64 = 400_000;
const ANTHROPIC_CONTEXT_WINDOW_TOKENS: u64 = 200_000;
const COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS: u64 = 400_000;
const COPILOT_ANTHROPIC_CONTEXT_WINDOW_TOKENS: u64 = 200_000;
const COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS: u64 = 1_048_576;
const DEEPSEEK_CONTEXT_WINDOW_TOKENS: u64 = 128_000;
const GEMINI_CONTEXT_WINDOW_TOKENS: u64 = 1_048_576;

pub fn provider_supported_endpoints(provider: ProviderId) -> &'static [ProviderEndpoint] {
    match provider {
        ProviderId::OpenAi | ProviderId::Local => OPENAI_ENDPOINTS,
        ProviderId::Gemini => GEMINI_ENDPOINTS,
        ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => CORE_TEXT_ENDPOINTS,
    }
}

macro_rules! model {
    ($provider:expr, $owned_by:expr, $id:expr, $display:expr, $description:expr, $ctx:expr, $in_cost:expr, $out_cost:expr, $endpoints:expr, [$($alias:expr),* $(,)?]) => {
        ProviderModelSpec {
            id: $id,
            display_name: $display,
            description: $description,
            provider: $provider,
            owned_by: $owned_by,
            context_window_tokens: $ctx,
            input_cost_per_million_microusd: $in_cost,
            output_cost_per_million_microusd: $out_cost,
            endpoints: $endpoints,
            aliases: &[$($alias),*],
        }
    };
}

const OPENAI_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.4",
        "GPT-5.4",
        "Frontier agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["opus", "best", "default"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.4-mini",
        "GPT-5.4 Mini",
        "Smaller frontier agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["haiku", "mini"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.3-codex",
        "GPT-5.3 Codex",
        "Codex-optimized agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["codex", "sonnet"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.2",
        "GPT-5.2",
        "General frontier model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        []
    ),
];

const ANTHROPIC_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "auto",
        "Anthropic Auto",
        "Anthropic alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "opus",
        "Claude Opus",
        "Anthropic Opus alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["best"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "sonnet",
        "Claude Sonnet",
        "Anthropic Sonnet alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["pro"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "haiku",
        "Claude Haiku",
        "Anthropic Haiku alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["flash"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-8",
        "Claude Opus 4.8",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-sonnet-4-6",
        "Claude Sonnet 4.6",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-haiku-4-5",
        "Claude Haiku 4.5",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-6",
        "Claude Opus 4.6",
        "Anthropic compatibility model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-20250514",
        "Claude Opus 4",
        "Anthropic compatibility model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
];

const COPILOT_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "auto",
        "Copilot Auto",
        "GitHub Copilot alias routed through provider fallback.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "codex",
        "Copilot Codex",
        "GitHub Copilot Codex alias routed through provider fallback.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["pro"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.1-codex",
        "GPT-5.1 Codex",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.4",
        "GPT-5.4",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.3-codex",
        "GPT-5.3 Codex",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-sonnet-4-6",
        "Claude Sonnet 4.6",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["claude", "sonnet"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gemini-3.1-pro-preview",
        "Gemini 3.1 Pro Preview",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["gemini"]
    ),
];

const DEEPSEEK_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-v4-pro",
        "DeepSeek V4 Pro",
        "DeepSeek Pro alias routed through provider fallback.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["pro", "auto"]
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-v4-flash",
        "DeepSeek V4 Flash",
        "DeepSeek Flash alias routed through provider fallback.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["flash"]
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-chat",
        "DeepSeek Chat",
        "DeepSeek chat-completions model.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-reasoner",
        "DeepSeek Reasoner",
        "DeepSeek reasoning model.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
];

const GEMINI_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Gemini,
        "google",
        "auto",
        "Gemini Auto",
        "Gemini CLI-style auto routing through preview, pro, and flash fallback models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "auto-gemini-3",
        "Gemini 3 Auto",
        "Gemini CLI preview auto alias routed through Gemini 3 and stable fallback models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "auto-gemini-2.5",
        "Gemini 2.5 Auto",
        "Gemini CLI stable auto alias routed through Gemini 2.5 Pro and Flash models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "pro",
        "Gemini Pro",
        "Gemini Pro alias routed through preview and stable Pro models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "flash",
        "Gemini Flash",
        "Gemini Flash alias routed through preview and stable Flash models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "flash-lite",
        "Gemini Flash Lite",
        "Gemini Flash-Lite alias routed through Flash-Lite models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        PRODEX_GEMINI_CHAT_COMPRESSION_MODEL,
        "Gemini Chat Compression",
        "Gemini CLI semantic compaction alias routed through chat-compression-default.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-pro-preview",
        "Gemini 3.1 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-pro-preview-customtools",
        "Gemini 3.1 Pro Preview Custom Tools",
        "Gemini CLI preview Pro variant for custom tools routed through Prodex.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-pro-preview",
        "Gemini 3 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-flash-preview",
        "Gemini 3 Flash Preview",
        "Gemini CLI preview Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-flash",
        "Gemini 3 Flash",
        "Gemini CLI secondary Flash model name routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.5-flash",
        "Gemini 3.5 Flash",
        "Gemini CLI Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-flash-lite",
        "Gemini 3.1 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-pro",
        "Gemini 2.5 Pro",
        "Gemini CLI stable Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-flash",
        "Gemini 2.5 Flash",
        "Gemini CLI stable Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-flash-lite",
        "Gemini 2.5 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemma-4-31b-it",
        "Gemma 4 31B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemma-4-26b-a4b-it",
        "Gemma 4 26B A4B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
];

const LOCAL_MODELS: &[ProviderModelSpec] = &[model!(
    ProviderId::Local,
    "local",
    "local",
    "Local OpenAI Compatible",
    "Local OpenAI-compatible server default model.",
    None,
    None,
    None,
    OPENAI_ENDPOINTS,
    ["default"]
)];

pub fn provider_model_catalog(provider: ProviderId) -> &'static [ProviderModelSpec] {
    match provider {
        ProviderId::OpenAi => OPENAI_MODELS,
        ProviderId::Anthropic => ANTHROPIC_MODELS,
        ProviderId::Copilot => COPILOT_MODELS,
        ProviderId::DeepSeek => DEEPSEEK_MODELS,
        ProviderId::Gemini => GEMINI_MODELS,
        ProviderId::Local => LOCAL_MODELS,
    }
}

pub fn provider_model_spec(
    provider: ProviderId,
    model: &str,
) -> Option<&'static ProviderModelSpec> {
    let model = model.trim();
    provider_model_catalog(provider)
        .iter()
        .find(|spec| spec.matches_id_or_alias(model))
}

pub fn provider_model_cost(provider: ProviderId, model: &str) -> ProviderModelCost {
    provider_model_spec(provider, model)
        .map(|spec| spec.cost())
        .unwrap_or_default()
}

pub fn provider_model_json(provider: ProviderId, model: &str) -> Option<serde_json::Value> {
    provider_model_spec(provider, model).map(|model| {
        serde_json::json!({
            "id": model.id,
            "object": "model",
            "owned_by": model.owned_by,
            "display_name": model.display_name,
            "description": model.description,
            "context_window": model.context_window_tokens,
            "input_cost_per_million_microusd": model.input_cost_per_million_microusd,
            "output_cost_per_million_microusd": model.output_cost_per_million_microusd,
        })
    })
}

pub fn provider_model_catalog_json(provider: ProviderId) -> Vec<serde_json::Value> {
    provider_model_catalog(provider)
        .iter()
        .map(|model| {
            serde_json::json!({
                "id": model.id,
                "object": "model",
                "owned_by": model.owned_by,
                "display_name": model.display_name,
                "description": model.description,
                "context_window": model.context_window_tokens,
                "input_cost_per_million_microusd": model.input_cost_per_million_microusd,
                "output_cost_per_million_microusd": model.output_cost_per_million_microusd,
            })
        })
        .collect()
}

pub fn provider_model_fallback_chain(provider: ProviderId, model: &str) -> Vec<String> {
    let model = model.trim();
    if let Some(chain) = combo_chain(model) {
        return chain;
    }
    let lower = model.to_ascii_lowercase();
    let chain: &[&str] = match provider {
        ProviderId::Anthropic => match lower.as_str() {
            "" | "auto" | "default" => &[
                PRODEX_ANTHROPIC_DEFAULT_MODEL,
                "claude-opus-4-8",
                "claude-haiku-4-5",
            ],
            "opus" | "best" => &["claude-opus-4-8", "claude-sonnet-4-6"],
            "sonnet" | "pro" => &["claude-sonnet-4-6", "claude-opus-4-8"],
            "haiku" | "flash" => &["claude-haiku-4-5", "claude-sonnet-4-6"],
            _ => return non_empty_single(model),
        },
        ProviderId::Copilot => match lower.as_str() {
            "" | "auto" | "default" => &[PRODEX_COPILOT_DEFAULT_MODEL, "gpt-5.4", "gpt-5.3-codex"],
            "codex" | "pro" => &["gpt-5.1-codex", "gpt-5.3-codex", "gpt-5.4"],
            "claude" | "sonnet" => &["claude-sonnet-4-6", "gpt-5.1-codex"],
            "gemini" => &["gemini-3.1-pro-preview", "gpt-5.1-codex"],
            _ => return non_empty_single(model),
        },
        ProviderId::Gemini => match lower.as_str() {
            "chat-compression-default" => &[
                "gemini-3-pro-preview",
                "gemini-3-flash-preview",
                "gemini-2.5-pro",
                "gemini-2.5-flash",
            ],
            "" | "auto" | "auto-gemini-3" => &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "auto-gemini-2.5" => &["gemini-2.5-pro", "gemini-2.5-flash"],
            "pro" => &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
            ],
            "gemini-3.1-pro-preview-customtools" => &[
                "gemini-3.1-pro-preview-customtools",
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3.1-pro-preview" => &[
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3-pro-preview" => &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3.5-flash" => &[
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-3-flash-preview",
                "gemini-2.5-flash",
            ],
            "gemini-3-flash-preview" => &[
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3-flash" => &["gemini-3-flash", "gemini-3.5-flash", "gemini-2.5-flash"],
            "gemini-3.1-flash-lite" => &[
                "gemini-3.1-flash-lite",
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
            ],
            "flash" => &[
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "flash-lite" => &["gemini-3.1-flash-lite", "gemini-2.5-flash-lite"],
            _ => return non_empty_single(model),
        },
        ProviderId::DeepSeek => match lower.as_str() {
            "" | "auto" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "pro" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "flash" => &["deepseek-v4-flash", "deepseek-v4-pro"],
            _ => return non_empty_single(model),
        },
        ProviderId::OpenAi | ProviderId::Local => return non_empty_single(model),
    };
    dedup_chain(chain.iter().map(|value| (*value).to_string()).collect())
}

fn non_empty_single(model: &str) -> Vec<String> {
    if model.is_empty() {
        Vec::new()
    } else {
        vec![model.to_string()]
    }
}

fn combo_chain(model: &str) -> Option<Vec<String>> {
    let chain = model.trim().strip_prefix("combo:")?;
    let models = chain
        .split([',', ';', '|', '>'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!models.is_empty()).then(|| dedup_chain(models))
}

fn dedup_chain(models: Vec<String>) -> Vec<String> {
    let mut seen = Vec::<String>::new();
    let mut deduped = Vec::new();
    for model in models {
        let key = model.to_ascii_lowercase();
        if seen.iter().any(|value| value == &key) {
            continue;
        }
        seen.push(key);
        deduped.push(model);
    }
    deduped
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderTokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

impl ProviderTokenUsage {
    pub fn merged_total(self) -> Option<u64> {
        self.total_tokens.or_else(|| {
            Some(
                self.input_tokens?
                    .saturating_add(self.output_tokens.unwrap_or_default()),
            )
        })
    }
}

pub fn estimate_request_input_tokens(body: &[u8]) -> u64 {
    if body.is_empty() {
        return 0;
    }
    let parsed = serde_json::from_slice::<serde_json::Value>(body).ok();
    let estimated = parsed
        .as_ref()
        .and_then(estimate_semantic_request_tokens)
        .unwrap_or_else(|| estimate_text_tokens(&String::from_utf8_lossy(body)));
    estimated.max(1)
}

fn estimate_semantic_request_tokens(value: &serde_json::Value) -> Option<u64> {
    let mut total = 0_u64;
    if let Some(object) = value.as_object() {
        for key in [
            "input",
            "messages",
            "contents",
            "content",
            "parts",
            "prompt",
            "system",
            "instructions",
            "tools",
        ] {
            if let Some(value) = object.get(key) {
                total = total.saturating_add(estimate_value_tokens(value));
            }
        }
    }
    (total > 0).then_some(total)
}

fn estimate_value_tokens(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::String(text) => estimate_text_tokens(text),
        serde_json::Value::Array(values) => values.iter().fold(0_u64, |sum, value| {
            sum.saturating_add(estimate_value_tokens(value))
        }),
        serde_json::Value::Object(object) => object.iter().fold(0_u64, |sum, (key, value)| {
            if matches!(
                key.as_str(),
                "model"
                    | "role"
                    | "type"
                    | "id"
                    | "name"
                    | "metadata"
                    | "temperature"
                    | "top_p"
                    | "stream"
            ) {
                sum
            } else {
                sum.saturating_add(estimate_value_tokens(value))
            }
        }),
        _ => 0,
    }
}

pub fn estimate_text_tokens(text: &str) -> u64 {
    let significant = text.chars().filter(|ch| !ch.is_control()).count() as u64;
    significant.saturating_add(3) / 4
}

pub fn extract_usage_tokens(body: &[u8]) -> ProviderTokenUsage {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return ProviderTokenUsage::default();
    };
    extract_usage_from_value(&value)
}

fn extract_usage_from_value(value: &serde_json::Value) -> ProviderTokenUsage {
    let usage = value.get("usage").unwrap_or(value);
    let input_tokens = first_u64(
        usage,
        &[
            "input_tokens",
            "prompt_tokens",
            "promptTokens",
            "inputTokens",
            "cache_creation_input_tokens",
        ],
    )
    .or_else(|| {
        value
            .get("usageMetadata")
            .and_then(|usage| first_u64(usage, &["promptTokenCount"]))
    });
    let output_tokens = first_u64(
        usage,
        &[
            "output_tokens",
            "completion_tokens",
            "completionTokens",
            "outputTokens",
        ],
    )
    .or_else(|| {
        value
            .get("usageMetadata")
            .and_then(|usage| first_u64(usage, &["candidatesTokenCount"]))
    });
    let total_tokens = first_u64(usage, &["total_tokens", "totalTokens"]).or_else(|| {
        value
            .get("usageMetadata")
            .and_then(|usage| first_u64(usage, &["totalTokenCount"]))
    });
    ProviderTokenUsage {
        input_tokens,
        output_tokens,
        total_tokens,
    }
}

fn first_u64(value: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_u64))
}

pub fn calculate_cost_microusd(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost: ProviderModelCost,
) -> Option<u64> {
    let mut total = 0_u64;
    let mut known = false;
    if let (Some(tokens), Some(rate)) = (input_tokens, cost.input_cost_per_million_microusd) {
        total = total.saturating_add(tokens.saturating_mul(rate) / 1_000_000);
        known = true;
    }
    if let (Some(tokens), Some(rate)) = (output_tokens, cost.output_cost_per_million_microusd) {
        total = total.saturating_add(tokens.saturating_mul(rate) / 1_000_000);
        known = true;
    }
    known.then_some(total)
}

pub fn microusd_to_usd(value: u64) -> f64 {
    value as f64 / 1_000_000.0
}

pub fn classify_provider_error(
    status: Option<u16>,
    code: Option<&str>,
    text: Option<&str>,
) -> ProviderErrorClassification {
    let normalized_code = code.unwrap_or_default().trim().to_ascii_lowercase();
    let normalized_text = text.unwrap_or_default().trim().to_ascii_lowercase();
    if matches!(status, Some(401 | 403))
        || matches!(
            normalized_code.as_str(),
            "unauthenticated" | "invalid_api_key" | "authentication_error"
        )
    {
        return ProviderErrorClassification {
            class: ProviderErrorClass::Auth,
            cooldown_ms: 0,
        };
    }
    if matches!(
        normalized_code.as_str(),
        "insufficient_quota" | "quota_exhausted" | "quota_exceeded" | "resource_exhausted"
    ) {
        return ProviderErrorClassification {
            class: ProviderErrorClass::Quota,
            cooldown_ms: 300_000,
        };
    }
    if matches!(
        normalized_code.as_str(),
        "rate_limit_exceeded" | "rate_limit_exceeded_error"
    ) || status == Some(429)
    {
        return ProviderErrorClassification {
            class: ProviderErrorClass::RateLimit,
            cooldown_ms: 60_000,
        };
    }
    if status == Some(404) {
        return ProviderErrorClassification {
            class: ProviderErrorClass::NotFound,
            cooldown_ms: 0,
        };
    }
    if matches!(status, Some(500 | 502 | 503 | 504)) || normalized_text.contains("overloaded") {
        return ProviderErrorClassification {
            class: ProviderErrorClass::Transient,
            cooldown_ms: 10_000,
        };
    }
    ProviderErrorClassification {
        class: ProviderErrorClass::Other,
        cooldown_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_gateway_providers() {
        for provider in [
            ProviderId::OpenAi,
            ProviderId::Anthropic,
            ProviderId::Copilot,
            ProviderId::DeepSeek,
            ProviderId::Gemini,
            ProviderId::Local,
        ] {
            assert!(!provider_model_catalog(provider).is_empty());
            assert!(provider_supported_endpoints(provider).contains(&ProviderEndpoint::Responses));
            assert!(provider_supported_endpoints(provider).contains(&ProviderEndpoint::Models));
        }
    }

    #[test]
    fn fallback_chain_preserves_existing_gemini_aliases() {
        assert_eq!(
            provider_model_fallback_chain(ProviderId::Gemini, "flash")[0],
            "gemini-3-flash-preview"
        );
        assert_eq!(
            provider_model_fallback_chain(ProviderId::Anthropic, "opus"),
            vec!["claude-opus-4-8", "claude-sonnet-4-6"]
        );
        assert_eq!(
            provider_model_fallback_chain(ProviderId::Copilot, "codex"),
            vec!["gpt-5.1-codex", "gpt-5.3-codex", "gpt-5.4"]
        );
    }

    #[test]
    fn semantic_token_estimator_ignores_json_scaffolding() {
        let body = br#"{"model":"x","messages":[{"role":"user","content":"hello world from prodex"}],"stream":true}"#;
        let semantic = estimate_request_input_tokens(body);
        let raw = estimate_text_tokens(&String::from_utf8_lossy(body));
        assert!(semantic < raw);
        assert!(semantic > 0);
    }

    #[test]
    fn usage_parser_reads_openai_and_gemini_shapes() {
        let openai = extract_usage_tokens(
            br#"{"usage":{"input_tokens":10,"output_tokens":20,"total_tokens":30}}"#,
        );
        assert_eq!(openai.input_tokens, Some(10));
        assert_eq!(openai.output_tokens, Some(20));
        assert_eq!(openai.total_tokens, Some(30));

        let gemini = extract_usage_tokens(
            br#"{"usageMetadata":{"promptTokenCount":11,"candidatesTokenCount":22,"totalTokenCount":33}}"#,
        );
        assert_eq!(gemini.input_tokens, Some(11));
        assert_eq!(gemini.output_tokens, Some(22));
        assert_eq!(gemini.total_tokens, Some(33));
    }

    #[test]
    fn cost_calc_uses_micro_usd_rates() {
        let cost = ProviderModelCost {
            input_cost_per_million_microusd: Some(1_000_000),
            output_cost_per_million_microusd: Some(2_000_000),
        };
        assert_eq!(
            calculate_cost_microusd(Some(1_000), Some(2_000), cost),
            Some(5_000)
        );
        assert_eq!(microusd_to_usd(5_000), 0.005);
    }
}
