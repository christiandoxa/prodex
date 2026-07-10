//! Provider endpoint lists and lookup helpers.

use super::{ProviderEndpoint, ProviderId};

pub(crate) const CORE_TEXT_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Models,
];

pub(crate) const COPILOT_TEXT_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ResponsesCompact,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Models,
];

pub(crate) const KIRO_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ResponsesCompact,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Models,
];

pub(crate) const OPENAI_ENDPOINTS: &[ProviderEndpoint] = &[
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

pub(crate) const GEMINI_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Models,
    ProviderEndpoint::Embeddings,
];

pub const ALL_PROVIDER_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::Responses,
    ProviderEndpoint::ResponsesCompact,
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
        ProviderId::Copilot => COPILOT_TEXT_ENDPOINTS,
        ProviderId::Kiro => KIRO_ENDPOINTS,
        ProviderId::Anthropic | ProviderId::DeepSeek => CORE_TEXT_ENDPOINTS,
    }
}
