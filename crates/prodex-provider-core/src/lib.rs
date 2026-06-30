use serde::Serialize;

mod adapter;
mod errors;
mod fallback;
mod models;
mod replay_cases;
#[cfg(test)]
mod tests;
mod usage;

pub use adapter::{StaticProviderAdapter, provider_adapter};
pub use errors::{ProviderErrorClass, ProviderErrorClassification, classify_provider_error};
pub use fallback::provider_model_fallback_chain;
pub use models::{
    provider_model_catalog, provider_model_catalog_json, provider_model_cost, provider_model_json,
    provider_model_spec,
};
use replay_cases::provider_replay_case_count;
pub use replay_cases::{ProviderReplayCase, provider_replay_cases};
pub use usage::{
    ProviderTokenUsage, calculate_cost_microusd, estimate_request_input_tokens,
    estimate_text_tokens, extract_usage_tokens, microusd_to_usd,
};

pub const PRODEX_ANTHROPIC_DEFAULT_MODEL: &str = "claude-sonnet-4-6";
pub const PRODEX_COPILOT_DEFAULT_MODEL: &str = "gpt-5.3-codex";
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderCapabilityStatus {
    Native,
    Translated,
    Passthrough,
    Emulated,
    Partial,
    Unsupported,
    Untested,
}

impl ProviderCapabilityStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Translated => "translated",
            Self::Passthrough => "passthrough",
            Self::Emulated => "emulated",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
            Self::Untested => "untested",
        }
    }
}

impl ProviderEndpoint {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat-completions",
            Self::Messages => "messages",
            Self::Models => "models",
            Self::Embeddings => "embeddings",
            Self::Images => "images",
            Self::Audio => "audio",
            Self::Batches => "batches",
            Self::Rerank => "rerank",
            Self::A2a => "a2a",
        }
    }
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
    fn capability_status(&self, endpoint: ProviderEndpoint) -> ProviderCapabilityStatus;

    fn transform_status(&self) -> ProviderCapabilityStatus {
        if self.client_request_format() == self.upstream_request_format()
            && self.upstream_request_format() == self.response_format()
        {
            ProviderCapabilityStatus::Passthrough
        } else {
            ProviderCapabilityStatus::Translated
        }
    }

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

    fn transform_stream_event(&self, event: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::UpstreamStreamEventToClient,
            provider: self.provider(),
            from_format: self.upstream_request_format(),
            to_format: self.response_format(),
            body: event.to_vec(),
            lossy: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProviderAdapterContractSpec {
    pub provider: &'static str,
    pub client_request_format: &'static str,
    pub upstream_request_format: &'static str,
    pub response_format: &'static str,
    pub canonical_client_endpoint: &'static str,
    pub model_list_endpoint: &'static str,
    pub supports_streaming: bool,
    pub supports_model_fallback: bool,
    pub transform_status: &'static str,
    pub supported_endpoints: Vec<&'static str>,
    pub endpoint_status: Vec<ProviderEndpointContractSpec>,
    pub model_count: usize,
    pub replay_case_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProviderEndpointContractSpec {
    pub endpoint: &'static str,
    pub status: &'static str,
    pub streaming: bool,
    pub tested: bool,
}

pub const PROVIDER_CONTRACT_PROVIDERS: &[ProviderId] = &[
    ProviderId::OpenAi,
    ProviderId::Anthropic,
    ProviderId::Copilot,
    ProviderId::DeepSeek,
    ProviderId::Gemini,
    ProviderId::Local,
];

pub fn provider_adapter_contract_spec(provider: ProviderId) -> ProviderAdapterContractSpec {
    let adapter = provider_adapter(provider);
    ProviderAdapterContractSpec {
        provider: adapter.provider().label(),
        client_request_format: adapter.client_request_format().label(),
        upstream_request_format: adapter.upstream_request_format().label(),
        response_format: adapter.response_format().label(),
        canonical_client_endpoint: adapter.canonical_client_endpoint(),
        model_list_endpoint: adapter.model_list_endpoint(),
        supports_streaming: adapter.supports_streaming(),
        supports_model_fallback: adapter.supports_model_fallback(),
        transform_status: adapter.transform_status().label(),
        supported_endpoints: adapter
            .supported_endpoints()
            .iter()
            .map(|endpoint| endpoint.label())
            .collect(),
        endpoint_status: ALL_PROVIDER_ENDPOINTS
            .iter()
            .copied()
            .map(|endpoint| ProviderEndpointContractSpec {
                endpoint: endpoint.label(),
                status: adapter.capability_status(endpoint).label(),
                streaming: adapter.supports_streaming()
                    && matches!(
                        endpoint,
                        ProviderEndpoint::Responses
                            | ProviderEndpoint::ChatCompletions
                            | ProviderEndpoint::Messages
                    ),
                tested: adapter.supported_endpoints().contains(&endpoint)
                    && provider_replay_case_count(provider) > 0,
            })
            .collect(),
        model_count: adapter.model_catalog().len(),
        replay_case_count: provider_replay_case_count(provider),
    }
}

pub fn provider_adapter_contract_matrix() -> Vec<ProviderAdapterContractSpec> {
    PROVIDER_CONTRACT_PROVIDERS
        .iter()
        .copied()
        .map(provider_adapter_contract_spec)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTransformPhase {
    ClientRequestToUpstream,
    UpstreamResponseToClient,
    UpstreamStreamEventToClient,
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

pub const ALL_PROVIDER_ENDPOINTS: &[ProviderEndpoint] = &[
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

pub fn provider_supported_endpoints(provider: ProviderId) -> &'static [ProviderEndpoint] {
    match provider {
        ProviderId::OpenAi | ProviderId::Local => OPENAI_ENDPOINTS,
        ProviderId::Gemini => GEMINI_ENDPOINTS,
        ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => CORE_TEXT_ENDPOINTS,
    }
}
