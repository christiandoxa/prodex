//! Provider adapter contract matrix and capability rendering.

use serde::Serialize;

use crate::{
    ALL_PROVIDER_ENDPOINTS, EffectiveHarnessMode, HarnessMode, HarnessModeSpec,
    ProviderAdapterContract, ProviderCapabilityStatus, ProviderConformanceOperation,
    ProviderEndpoint, ProviderId, harness_mode_catalog, provider_adapter,
    provider_conformance_cases, provider_implementation_registry, provider_replay_case_count,
    provider_translator,
};

#[path = "contract/markdown.rs"]
mod markdown;

pub use self::markdown::provider_capabilities_markdown;

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
    pub unsupported_params: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProviderContractCatalogSpec {
    pub supported_harness_modes: Vec<&'static str>,
    pub default_harness_mode: &'static str,
    pub resolved_harness_mode: &'static str,
    pub harness_modes: &'static [HarnessModeSpec],
    pub providers: Vec<ProviderAdapterContractSpec>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProviderEndpointCoverage {
    request: bool,
    response: bool,
    stream: bool,
}

impl ProviderEndpointCoverage {
    fn any(self) -> bool {
        self.request || self.response || self.stream
    }

    fn request_and_response(self) -> bool {
        self.request && self.response
    }
}

pub const PROVIDER_CONTRACT_PROVIDERS: &[ProviderId] = crate::PROVIDER_IMPLEMENTATION_ORDER;

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
            .map(|endpoint| {
                let coverage = provider_endpoint_conformance_coverage(provider, endpoint);
                let tested = coverage.any();
                let support =
                    provider_translator(provider).supported_params(endpoint, "test-model");
                ProviderEndpointContractSpec {
                    endpoint: endpoint.label(),
                    status: provider_endpoint_contract_status(
                        adapter.capability_status(endpoint),
                        adapter.supported_endpoints().contains(&endpoint),
                        coverage,
                    )
                    .label(),
                    streaming: adapter.supports_streaming()
                        && matches!(
                            endpoint,
                            ProviderEndpoint::Responses
                                | ProviderEndpoint::ChatCompletions
                                | ProviderEndpoint::Messages
                        ),
                    tested,
                    unsupported_params: support
                        .unsupported
                        .into_iter()
                        .map(|reason| reason.field)
                        .collect(),
                }
            })
            .collect(),
        model_count: adapter.model_catalog().len(),
        replay_case_count: provider_replay_case_count(provider),
    }
}

pub fn provider_adapter_contract_matrix() -> Vec<ProviderAdapterContractSpec> {
    provider_implementation_registry()
        .iter()
        .map(|descriptor| provider_adapter_contract_spec(descriptor.provider()))
        .collect()
}

pub fn provider_contract_catalog(
    resolved_harness_mode: EffectiveHarnessMode,
) -> ProviderContractCatalogSpec {
    ProviderContractCatalogSpec {
        supported_harness_modes: harness_mode_catalog()
            .iter()
            .filter(|spec| spec.selectable)
            .map(|spec| spec.id)
            .collect(),
        default_harness_mode: HarnessMode::default().id(),
        resolved_harness_mode: resolved_harness_mode.id(),
        harness_modes: harness_mode_catalog(),
        providers: provider_adapter_contract_matrix(),
    }
}

fn provider_endpoint_conformance_coverage(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
) -> ProviderEndpointCoverage {
    let mut coverage = ProviderEndpointCoverage::default();
    for case in provider_conformance_cases() {
        if case.provider != provider || case.endpoint != endpoint {
            continue;
        }
        match case.operation {
            ProviderConformanceOperation::Request => coverage.request = true,
            ProviderConformanceOperation::Response => coverage.response = true,
            ProviderConformanceOperation::StreamEvent => coverage.stream = true,
        }
    }
    coverage
}

fn provider_endpoint_contract_status(
    base: ProviderCapabilityStatus,
    supported: bool,
    coverage: ProviderEndpointCoverage,
) -> ProviderCapabilityStatus {
    if !supported {
        return ProviderCapabilityStatus::Unsupported;
    }
    if !coverage.any() {
        return ProviderCapabilityStatus::Untested;
    }
    if !coverage.request_and_response() {
        return ProviderCapabilityStatus::Partial;
    }
    match base {
        ProviderCapabilityStatus::Unsupported
        | ProviderCapabilityStatus::Untested
        | ProviderCapabilityStatus::Partial => base,
        other => other,
    }
}
