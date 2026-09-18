//! Provider endpoint lists and lookup helpers.

use super::{ProviderEndpoint, ProviderId};

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
    crate::provider_implementation_registry()
        .get(provider)
        .expect("built-in provider implementation must be registered")
        .supported_endpoints()
}
