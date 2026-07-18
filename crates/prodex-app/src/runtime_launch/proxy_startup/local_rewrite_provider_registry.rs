use super::local_rewrite::RuntimeLocalRewriteProviderOptions;
use super::local_rewrite_application_data_plane::{
    runtime_gateway_provider_capability_is_executable, runtime_gateway_provider_credential_ref,
    runtime_gateway_provider_executable_capabilities,
};
use super::local_rewrite_options::RuntimeProjectedProviderCredential;
use super::{
    RuntimeAnthropicProviderAuth, RuntimeCopilotProviderAuth, RuntimeDeepSeekWebSearchMode,
    RuntimeGeminiProviderAuth,
};
use anyhow::{Context, Result};
use prodex_domain::{
    CapabilitySet, DataClassification, PolicySelector, ProviderTrustTier, SecretRef, TenantContext,
    TenantId,
};
use prodex_provider_core::{
    ProviderAdapterContract, ProviderEndpoint, ProviderId, ProviderModelCost, provider_adapter,
    provider_model_catalog,
};
use prodex_provider_spi::{
    GovernedProviderDescriptor, GovernedProviderRegistry, GovernedRoute, GovernedRoutingPlan,
    GovernedRoutingSignals, GovernedRoutingWeights, MAX_GOVERNED_PROVIDER_REGIONS,
    MAX_GOVERNED_ROUTING_CANDIDATES,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

const RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION: u32 = 2;
const RUNTIME_GATEWAY_PROVIDER_REGISTRY_LEGACY_SCHEMA_VERSION: u32 = 1;
const RUNTIME_GATEWAY_ROUTING_SCORES_SCHEMA_VERSION: u32 = 1;
const MAX_RUNTIME_GATEWAY_PROVIDER_PRICED_MODELS: usize = 1_024;
pub(super) const MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_ARTIFACT_BYTES: usize = 1024 * 1024;
const MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_TENANTS: usize = 64;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimeGatewayProviderRegistryArtifact {
    // This authority covers the governed planner contract. Model aliases/context limits,
    // deployment transport limits, and live health/quota/load stay with their existing route,
    // adapter, and runtime-state authorities; they are not duplicated as stale registry facts.
    schema_version: u32,
    revision: u64,
    pricing_revision: u64,
    descriptors: Vec<RuntimeGatewayProviderRegistryDescriptorArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeGatewayProviderRegistryDescriptorArtifact {
    revision: u64,
    pricing_revision: u64,
    provider: ProviderId,
    credential_ref: SecretRef,
    enabled: bool,
    revoked: bool,
    executable: bool,
    #[serde(default)]
    upstream_base_url: Option<String>,
    endpoints: Vec<ProviderEndpoint>,
    capabilities: CapabilitySet,
    regions: Vec<String>,
    local_execution: bool,
    trust_tier: RuntimeGatewayProviderRegistryTrustTier,
    maximum_classification: DataClassification,
    retention_seconds: u32,
    training_use: bool,
    #[serde(default)]
    model_costs: BTreeMap<String, RuntimeGatewayProviderModelCostArtifact>,
    cost: u16,
    latency: u16,
    risk: u16,
    priority: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeGatewayProviderModelCostArtifact {
    input_cost_per_million_microusd: Option<u64>,
    output_cost_per_million_microusd: Option<u64>,
}

impl RuntimeGatewayProviderModelCostArtifact {
    fn runtime_cost(self) -> ProviderModelCost {
        ProviderModelCost {
            input_cost_per_million_microusd: self.input_cost_per_million_microusd,
            output_cost_per_million_microusd: self.output_cost_per_million_microusd,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeGatewayProviderRegistryTrustTier {
    Standard,
    Enterprise,
    RestrictedApproved,
}

impl From<RuntimeGatewayProviderRegistryTrustTier> for ProviderTrustTier {
    fn from(value: RuntimeGatewayProviderRegistryTrustTier) -> Self {
        match value {
            RuntimeGatewayProviderRegistryTrustTier::Standard => Self::Standard,
            RuntimeGatewayProviderRegistryTrustTier::Enterprise => Self::Enterprise,
            RuntimeGatewayProviderRegistryTrustTier::RestrictedApproved => Self::RestrictedApproved,
        }
    }
}

#[derive(Clone)]
struct RuntimeGatewayCompiledProviderDescriptor {
    revision: u64,
    pricing_revision: u64,
    provider: ProviderId,
    credential_ref: SecretRef,
    enabled: bool,
    revoked: bool,
    executable: bool,
    upstream_base_url: Option<String>,
    endpoints: Vec<ProviderEndpoint>,
    capabilities: CapabilitySet,
    regions: Vec<PolicySelector>,
    local_execution: bool,
    trust_tier: ProviderTrustTier,
    maximum_classification: DataClassification,
    retention_seconds: u32,
    training_use: bool,
    model_costs: Arc<BTreeMap<String, ProviderModelCost>>,
    cost: u16,
    latency: u16,
    risk: u16,
    priority: u16,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayGovernedProviderRegistrySnapshot {
    revision: u64,
    authoritative_pricing: bool,
    attached_provider: ProviderId,
    projected_credential: Option<RuntimeProjectedProviderCredential>,
    descriptors: Vec<RuntimeGatewayCompiledProviderDescriptor>,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayProviderPricing {
    provider: ProviderId,
    revision: u64,
    model_costs: Arc<BTreeMap<String, ProviderModelCost>>,
}

impl RuntimeGatewayProviderPricing {
    pub(super) fn cost_for_model(
        &self,
        provider: ProviderId,
        model: &str,
    ) -> Option<ProviderModelCost> {
        if provider != self.provider || self.revision == 0 {
            return None;
        }
        runtime_gateway_model_cost(&self.model_costs, model)
    }
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayProviderExecution {
    pub(super) provider: RuntimeLocalRewriteProviderOptions,
    pub(super) credential: RuntimeProjectedProviderCredential,
    pub(super) upstream_base_url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RuntimeGatewayProviderRuntimeSignals {
    pub(super) health: Option<u16>,
    pub(super) load: u16,
    pub(super) quota_headroom: Option<u16>,
    pub(super) circuit_open: bool,
    pub(super) quota_available: bool,
    pub(super) inflight_cap_reached: bool,
}

impl Default for RuntimeGatewayProviderRuntimeSignals {
    fn default() -> Self {
        Self {
            health: None,
            load: 0,
            quota_headroom: None,
            circuit_open: false,
            quota_available: true,
            inflight_cap_reached: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct RuntimeGatewayProviderRuntimeSnapshot {
    providers: BTreeMap<ProviderId, RuntimeGatewayProviderRuntimeSignals>,
}

impl RuntimeGatewayProviderRuntimeSnapshot {
    pub(super) fn insert(
        &mut self,
        provider: ProviderId,
        signals: RuntimeGatewayProviderRuntimeSignals,
    ) {
        self.providers.insert(provider, signals);
    }

    fn signals_for(&self, provider: ProviderId) -> RuntimeGatewayProviderRuntimeSignals {
        self.providers.get(&provider).copied().unwrap_or_default()
    }
}

impl std::fmt::Debug for RuntimeGatewayGovernedProviderRegistrySnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeGatewayGovernedProviderRegistrySnapshot")
            .field("revision", &"<redacted>")
            .field("attached_provider", &self.attached_provider)
            .field("descriptor_count", &self.descriptors.len())
            .finish()
    }
}

impl RuntimeGatewayGovernedProviderRegistrySnapshot {
    pub(super) fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn provider_ids(&self) -> impl Iterator<Item = ProviderId> + '_ {
        self.descriptors
            .iter()
            .map(|descriptor| descriptor.provider)
    }

    pub(super) fn has_authoritative_pricing(&self) -> bool {
        self.authoritative_pricing
    }

    pub(super) fn reservation_cost_for_model(&self, model: &str) -> Option<ProviderModelCost> {
        self.authoritative_pricing.then_some(())?;
        self.descriptors
            .iter()
            .filter(|descriptor| descriptor.enabled && descriptor.executable && !descriptor.revoked)
            .filter_map(|descriptor| runtime_gateway_model_cost(&descriptor.model_costs, model))
            .reduce(max_provider_model_cost)
    }

    pub(super) fn runtime_profile_name(&self, provider: ProviderId) -> &str {
        if provider == self.attached_provider {
            super::local_rewrite::RUNTIME_LOCAL_REWRITE_PROFILE
        } else {
            provider.label()
        }
    }

    fn credential_available(&self, descriptor: &RuntimeGatewayCompiledProviderDescriptor) -> bool {
        descriptor.provider == self.attached_provider
            || self
                .projected_credential
                .as_ref()
                .is_some_and(|credential| {
                    credential.reference().provider() == descriptor.credential_ref.provider()
                })
    }

    pub(super) fn for_tenant(
        &self,
        tenant: TenantContext,
        endpoint: ProviderEndpoint,
        runtime: &RuntimeGatewayProviderRuntimeSnapshot,
    ) -> GovernedProviderRegistry {
        GovernedProviderRegistry {
            revision: self.revision,
            providers: self
                .descriptors
                .iter()
                .map(|descriptor| {
                    let runtime = runtime.signals_for(descriptor.provider);
                    GovernedProviderDescriptor {
                        revision: descriptor.revision,
                        pricing_revision: descriptor.pricing_revision,
                        tenant,
                        provider: descriptor.provider,
                        credential_ref: descriptor.credential_ref.clone(),
                        credential_available: self.credential_available(descriptor),
                        enabled: descriptor.enabled
                            && descriptor.executable
                            && descriptor.endpoints.contains(&endpoint),
                        revoked: descriptor.revoked,
                        circuit_open: runtime.circuit_open,
                        quota_available: runtime.quota_available,
                        inflight_cap_reached: runtime.inflight_cap_reached,
                        local_execution: descriptor.local_execution,
                        trust_tier: descriptor.trust_tier,
                        maximum_classification: descriptor.maximum_classification,
                        capabilities: descriptor.capabilities.clone(),
                        regions: descriptor.regions.clone(),
                        retention_seconds: descriptor.retention_seconds,
                        training_use: descriptor.training_use,
                        signals: GovernedRoutingSignals {
                            health: runtime.health,
                            load: runtime.load,
                            quota_headroom: runtime.quota_headroom,
                            cost: descriptor.cost,
                            latency: descriptor.latency,
                            risk: descriptor.risk,
                            priority: descriptor.priority,
                        },
                    }
                })
                .collect(),
        }
    }

    pub(super) fn matches_route(
        &self,
        routing: &GovernedRoutingPlan,
        endpoint: ProviderEndpoint,
    ) -> bool {
        self.matches_governed_route(routing.registry_revision, &routing.primary, endpoint)
    }

    pub(super) fn matches_governed_route(
        &self,
        registry_revision: u64,
        route: &GovernedRoute,
        endpoint: ProviderEndpoint,
    ) -> bool {
        registry_revision == self.revision && self.route_descriptor(route, endpoint).is_some()
    }

    fn route_descriptor(
        &self,
        route: &GovernedRoute,
        endpoint: ProviderEndpoint,
    ) -> Option<&RuntimeGatewayCompiledProviderDescriptor> {
        self.descriptors.iter().find(|descriptor| {
            descriptor.provider == route.provider
                && descriptor.revision == route.descriptor_revision
                && descriptor.pricing_revision == route.pricing_revision
                && descriptor.credential_ref == route.credential_ref
                && descriptor.enabled
                && descriptor.executable
                && !descriptor.revoked
                && descriptor.endpoints.contains(&endpoint)
                && self.credential_available(descriptor)
        })
    }

    pub(super) fn execution_for_route(
        &self,
        route: &GovernedRoute,
        endpoint: ProviderEndpoint,
    ) -> Option<RuntimeGatewayProviderExecution> {
        let descriptor = self.route_descriptor(route, endpoint)?;
        if descriptor.provider == self.attached_provider {
            return None;
        }
        let credential = self
            .projected_credential
            .as_ref()?
            .with_reference(descriptor.credential_ref.clone())?;
        let upstream_base_url = descriptor.upstream_base_url.clone()?;
        let provider = runtime_gateway_projected_provider_options(
            descriptor.provider,
            upstream_base_url.as_str(),
        )?;
        Some(RuntimeGatewayProviderExecution {
            provider,
            credential,
            upstream_base_url,
        })
    }

    pub(super) fn pricing_for_route(
        &self,
        route: &GovernedRoute,
        endpoint: ProviderEndpoint,
    ) -> Option<RuntimeGatewayProviderPricing> {
        self.authoritative_pricing.then_some(())?;
        let descriptor = self.route_descriptor(route, endpoint)?;
        (!descriptor.model_costs.is_empty()).then(|| RuntimeGatewayProviderPricing {
            provider: descriptor.provider,
            revision: descriptor.pricing_revision,
            model_costs: Arc::clone(&descriptor.model_costs),
        })
    }
}

fn runtime_gateway_model_cost(
    model_costs: &BTreeMap<String, ProviderModelCost>,
    model: &str,
) -> Option<ProviderModelCost> {
    model_costs
        .iter()
        .find_map(|(configured, cost)| {
            configured
                .eq_ignore_ascii_case(model.trim())
                .then_some(*cost)
        })
        .or_else(|| model_costs.get("*").copied())
}

fn max_provider_model_cost(left: ProviderModelCost, right: ProviderModelCost) -> ProviderModelCost {
    ProviderModelCost {
        input_cost_per_million_microusd: max_optional_cost(
            left.input_cost_per_million_microusd,
            right.input_cost_per_million_microusd,
        ),
        output_cost_per_million_microusd: max_optional_cost(
            left.output_cost_per_million_microusd,
            right.output_cost_per_million_microusd,
        ),
    }
}

fn max_optional_cost(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn runtime_gateway_projected_provider_options(
    provider: ProviderId,
    upstream_base_url: &str,
) -> Option<RuntimeLocalRewriteProviderOptions> {
    match provider {
        ProviderId::OpenAi => Some(RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
        }),
        ProviderId::Anthropic => Some(RuntimeLocalRewriteProviderOptions::Anthropic {
            auth: RuntimeAnthropicProviderAuth::Projected,
        }),
        ProviderId::Copilot => Some(RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Projected,
        }),
        ProviderId::DeepSeek => Some(RuntimeLocalRewriteProviderOptions::DeepSeek {
            api_keys: Vec::new(),
            strict_tools: false,
            beta_base_url: upstream_base_url.to_string(),
            web_search_mode: RuntimeDeepSeekWebSearchMode::default(),
        }),
        ProviderId::Gemini => Some(RuntimeLocalRewriteProviderOptions::Gemini {
            auth: RuntimeGeminiProviderAuth::Projected,
            thinking_budget_tokens: None,
            model_resolution: crate::RuntimeGeminiModelResolution::default(),
        }),
        // Kiro currently requires profile auth; Local has no heterogeneous remote SPI.
        ProviderId::Kiro | ProviderId::Local => None,
    }
}

#[derive(Debug)]
pub(super) struct RuntimeGatewayTenantSnapshotSet<T> {
    tenant_snapshots: BTreeMap<TenantId, Arc<T>>,
    fallback: Option<Arc<T>>,
}

impl<T> Clone for RuntimeGatewayTenantSnapshotSet<T> {
    fn clone(&self) -> Self {
        Self {
            tenant_snapshots: self.tenant_snapshots.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

impl<T> RuntimeGatewayTenantSnapshotSet<T> {
    pub(super) fn bootstrap(snapshot: T, allow_fallback: bool) -> Self {
        Self {
            tenant_snapshots: BTreeMap::new(),
            fallback: allow_fallback.then(|| Arc::new(snapshot)),
        }
    }

    pub(super) fn snapshot_for(&self, tenant_id: TenantId) -> Option<Arc<T>> {
        self.tenant_snapshots
            .get(&tenant_id)
            .cloned()
            .or_else(|| self.fallback.clone())
    }

    pub(super) fn with_tenant_snapshot(&self, tenant_id: TenantId, snapshot: T) -> Result<Self> {
        if !self.tenant_snapshots.contains_key(&tenant_id)
            && self.tenant_snapshots.len() >= MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_TENANTS
        {
            anyhow::bail!("provider registry tenant limit exceeded");
        }
        let mut next = self.clone();
        next.tenant_snapshots.insert(tenant_id, Arc::new(snapshot));
        Ok(next)
    }
}

pub(super) type RuntimeGatewayProviderRegistrySnapshotSet =
    RuntimeGatewayTenantSnapshotSet<RuntimeGatewayGovernedProviderRegistrySnapshot>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RuntimeGatewayRoutingScoresSnapshot {
    pub(super) revision: u64,
    pub(super) weights: GovernedRoutingWeights,
}

pub(super) type RuntimeGatewayRoutingScoresSnapshotSet =
    RuntimeGatewayTenantSnapshotSet<RuntimeGatewayRoutingScoresSnapshot>;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeGatewayRoutingScoresArtifact {
    schema_version: u32,
    revision: u64,
    weights: RuntimeGatewayRoutingWeightsArtifact,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeGatewayRoutingWeightsArtifact {
    health: u16,
    load: u16,
    cost: u16,
    latency: u16,
    risk: u16,
    priority: u16,
    affinity: u16,
}

pub(super) fn runtime_gateway_bootstrap_routing_scores_snapshot(
    settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
) -> RuntimeGatewayRoutingScoresSnapshot {
    RuntimeGatewayRoutingScoresSnapshot {
        revision: settings.routing_score_revision.unwrap_or(1),
        weights: GovernedRoutingWeights::default(),
    }
}

pub(super) fn compile_runtime_gateway_routing_scores_artifact(
    artifact: &[u8],
) -> Result<RuntimeGatewayRoutingScoresSnapshot> {
    if artifact.is_empty() || artifact.len() > MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_ARTIFACT_BYTES
    {
        anyhow::bail!("routing scores artifact size is invalid");
    }
    let artifact = serde_json::from_slice::<RuntimeGatewayRoutingScoresArtifact>(artifact)
        .context("routing scores artifact schema is invalid")?;
    if artifact.schema_version != RUNTIME_GATEWAY_ROUTING_SCORES_SCHEMA_VERSION
        || artifact.revision == 0
    {
        anyhow::bail!("routing scores artifact header is invalid");
    }
    let weights = GovernedRoutingWeights {
        health: artifact.weights.health,
        load: artifact.weights.load,
        cost: artifact.weights.cost,
        latency: artifact.weights.latency,
        risk: artifact.weights.risk,
        priority: artifact.weights.priority,
        affinity: artifact.weights.affinity,
    };
    let values = [
        weights.health,
        weights.load,
        weights.cost,
        weights.latency,
        weights.risk,
        weights.priority,
        weights.affinity,
    ];
    let total = values.into_iter().map(u64::from).sum::<u64>();
    if values
        .into_iter()
        .any(|value| value > prodex_provider_spi::ROUTING_SCORE_SCALE)
        || total == 0
        || total > u64::from(prodex_provider_spi::ROUTING_SCORE_SCALE)
    {
        anyhow::bail!("routing scores weights are invalid");
    }
    Ok(RuntimeGatewayRoutingScoresSnapshot {
        revision: artifact.revision,
        weights,
    })
}

struct RuntimeGatewayAttachedProviderRegistryContext {
    provider: ProviderId,
    credential_ref: SecretRef,
    projected_credential: Option<RuntimeProjectedProviderCredential>,
    endpoints: Vec<ProviderEndpoint>,
    capabilities: CapabilitySet,
}

fn runtime_gateway_attached_provider_registry_context(
    provider_options: &RuntimeLocalRewriteProviderOptions,
    credential: Option<&RuntimeProjectedProviderCredential>,
) -> RuntimeGatewayAttachedProviderRegistryContext {
    let provider = provider_options.bridge_kind().provider_id();
    let adapter = provider_adapter(provider);
    let endpoints = adapter
        .supported_endpoints()
        .iter()
        .copied()
        .filter(|endpoint| {
            runtime_gateway_provider_capability_is_executable(adapter.capability_status(*endpoint))
        })
        .filter(|endpoint| {
            !matches!(
                provider_options,
                RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly { .. }
            ) || *endpoint == ProviderEndpoint::Embeddings
        })
        .collect();
    RuntimeGatewayAttachedProviderRegistryContext {
        provider,
        credential_ref: runtime_gateway_provider_credential_ref(
            credential.map(RuntimeProjectedProviderCredential::reference),
            provider,
        ),
        projected_credential: credential.cloned(),
        endpoints,
        capabilities: runtime_gateway_provider_executable_capabilities(provider),
    }
}

pub(super) fn runtime_gateway_bootstrap_provider_registry_snapshot(
    settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    provider_options: &RuntimeLocalRewriteProviderOptions,
    credential: Option<&RuntimeProjectedProviderCredential>,
) -> Result<RuntimeGatewayGovernedProviderRegistrySnapshot> {
    let context = runtime_gateway_attached_provider_registry_context(provider_options, credential);
    let provider_settings = settings.provider.as_ref();
    let trust_tier = match provider_settings.map(|settings| settings.trust_tier) {
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Enterprise) => {
            RuntimeGatewayProviderRegistryTrustTier::Enterprise
        }
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::RestrictedApproved) => {
            RuntimeGatewayProviderRegistryTrustTier::RestrictedApproved
        }
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Standard) | None => {
            RuntimeGatewayProviderRegistryTrustTier::Standard
        }
    };
    let maximum_classification = match provider_settings
        .map(|settings| settings.maximum_classification)
        .unwrap_or(prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal)
    {
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Public => {
            DataClassification::Public
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal => {
            DataClassification::Internal
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Confidential => {
            DataClassification::Confidential
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Restricted => {
            DataClassification::Restricted
        }
    };
    compile_runtime_gateway_provider_registry_artifact(
        &serde_json::to_vec(&RuntimeGatewayProviderRegistryArtifact {
            schema_version: if runtime_gateway_provider_catalog_has_pricing(context.provider) {
                RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION
            } else {
                RUNTIME_GATEWAY_PROVIDER_REGISTRY_LEGACY_SCHEMA_VERSION
            },
            revision: settings.provider_registry_revision.unwrap_or(1),
            pricing_revision: 1,
            descriptors: vec![RuntimeGatewayProviderRegistryDescriptorArtifact {
                revision: provider_settings
                    .map(|settings| settings.descriptor_revision)
                    .unwrap_or(1),
                pricing_revision: 1,
                provider: context.provider,
                credential_ref: context.credential_ref.clone(),
                enabled: provider_settings.is_none_or(|settings| settings.enabled),
                revoked: provider_settings.is_some_and(|settings| settings.revoked),
                executable: true,
                upstream_base_url: None,
                endpoints: context.endpoints.clone(),
                capabilities: context.capabilities.clone(),
                regions: provider_settings
                    .map(|settings| settings.regions.clone())
                    .filter(|regions| !regions.is_empty())
                    .unwrap_or_else(|| vec!["*".to_string()]),
                local_execution: provider_settings.is_some_and(|settings| settings.local_execution),
                trust_tier,
                maximum_classification,
                retention_seconds: provider_settings
                    .map(|settings| settings.retention_seconds)
                    .unwrap_or(u32::MAX),
                training_use: provider_settings.is_none_or(|settings| settings.training_use),
                model_costs: runtime_gateway_builtin_model_costs(context.provider),
                cost: 5_000,
                latency: 5_000,
                risk: match trust_tier {
                    RuntimeGatewayProviderRegistryTrustTier::Standard => 8_000,
                    RuntimeGatewayProviderRegistryTrustTier::Enterprise => 4_000,
                    RuntimeGatewayProviderRegistryTrustTier::RestrictedApproved => 1_000,
                },
                priority: 5_000,
            }],
        })
        .context("failed to encode bootstrap provider registry")?,
        provider_options,
        credential,
    )
}

pub(super) fn compile_runtime_gateway_provider_registry_artifact(
    artifact: &[u8],
    provider_options: &RuntimeLocalRewriteProviderOptions,
    credential: Option<&RuntimeProjectedProviderCredential>,
) -> Result<RuntimeGatewayGovernedProviderRegistrySnapshot> {
    if artifact.is_empty() || artifact.len() > MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_ARTIFACT_BYTES
    {
        anyhow::bail!("provider registry artifact size is invalid");
    }
    let artifact = serde_json::from_slice::<RuntimeGatewayProviderRegistryArtifact>(artifact)
        .context("provider registry artifact schema is invalid")?;
    let context = runtime_gateway_attached_provider_registry_context(provider_options, credential);
    compile_runtime_gateway_provider_registry(artifact, &context)
}

pub(super) fn compile_runtime_gateway_provider_registry_artifact_for_deployment(
    artifact: &[u8],
    provider_options: &RuntimeLocalRewriteProviderOptions,
    credential: Option<&RuntimeProjectedProviderCredential>,
    deployment_mode: prodex_config::GovernanceMode,
) -> Result<RuntimeGatewayGovernedProviderRegistrySnapshot> {
    let snapshot =
        compile_runtime_gateway_provider_registry_artifact(artifact, provider_options, credential)?;
    if deployment_mode.is_enforcing() && !snapshot.has_authoritative_pricing() {
        anyhow::bail!("enforcing provider registry requires authoritative model pricing");
    }
    Ok(snapshot)
}

fn compile_runtime_gateway_provider_registry(
    artifact: RuntimeGatewayProviderRegistryArtifact,
    context: &RuntimeGatewayAttachedProviderRegistryContext,
) -> Result<RuntimeGatewayGovernedProviderRegistrySnapshot> {
    if !matches!(
        artifact.schema_version,
        RUNTIME_GATEWAY_PROVIDER_REGISTRY_LEGACY_SCHEMA_VERSION
            | RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION
    ) || artifact.revision == 0
        || artifact.pricing_revision == 0
        || artifact.descriptors.is_empty()
        || artifact.descriptors.len() > MAX_GOVERNED_ROUTING_CANDIDATES
    {
        anyhow::bail!("provider registry artifact header is invalid");
    }

    let authoritative_pricing =
        artifact.schema_version == RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION;
    let mut descriptors = Vec::with_capacity(artifact.descriptors.len());
    for (index, descriptor) in artifact.descriptors.into_iter().enumerate() {
        if descriptor.revision == 0
            || descriptor.pricing_revision == 0
            || descriptor.pricing_revision != artifact.pricing_revision
            || !descriptor.credential_ref.is_well_formed()
            || descriptor.endpoints.is_empty()
            || descriptor.endpoints.len() > prodex_provider_core::ALL_PROVIDER_ENDPOINTS.len()
            || descriptor.regions.is_empty()
            || descriptor.regions.len() > MAX_GOVERNED_PROVIDER_REGIONS
            || artifact_descriptors_duplicate_provider(&descriptors, descriptor.provider)
            || values_have_duplicate(&descriptor.endpoints)
            || [
                descriptor.cost,
                descriptor.latency,
                descriptor.risk,
                descriptor.priority,
            ]
            .into_iter()
            .any(|value| value > prodex_provider_spi::ROUTING_SCORE_SCALE)
            || descriptor.model_costs.len() > MAX_RUNTIME_GATEWAY_PROVIDER_PRICED_MODELS
            || (authoritative_pricing
                && !runtime_gateway_model_costs_are_authoritative(&descriptor.model_costs))
            || (!descriptor.model_costs.is_empty()
                && !runtime_gateway_model_costs_are_authoritative(&descriptor.model_costs))
        {
            anyhow::bail!("provider registry descriptor is invalid");
        }
        if descriptor.provider == context.provider {
            if !descriptor.executable
                || descriptor.credential_ref != context.credential_ref
                || descriptor
                    .endpoints
                    .iter()
                    .any(|endpoint| !context.endpoints.contains(endpoint))
                || !descriptor
                    .capabilities
                    .missing_from(&context.capabilities)
                    .is_empty()
            {
                anyhow::bail!("attached provider registry descriptor is invalid");
            }
        } else if descriptor.executable {
            let Some(projected_credential) = context.projected_credential.as_ref() else {
                anyhow::bail!("heterogeneous provider requires projected credentials");
            };
            if projected_credential.reference().provider() != descriptor.credential_ref.provider()
                || descriptor.upstream_base_url.as_deref().is_none_or(|value| {
                    crate::validate_credential_free_http_url(
                        value,
                        "provider registry upstream base URL",
                    )
                    .is_err()
                })
                || runtime_gateway_projected_provider_options(
                    descriptor.provider,
                    descriptor.upstream_base_url.as_deref().unwrap_or_default(),
                )
                .is_none()
            {
                anyhow::bail!("unsupported provider adapter cannot be executable");
            }
            let adapter = provider_adapter(descriptor.provider);
            if descriptor.endpoints.iter().any(|endpoint| {
                !adapter.supported_endpoints().contains(endpoint)
                    || !runtime_gateway_provider_capability_is_executable(
                        adapter.capability_status(*endpoint),
                    )
            }) || !descriptor
                .capabilities
                .missing_from(&runtime_gateway_provider_executable_capabilities(
                    descriptor.provider,
                ))
                .is_empty()
            {
                anyhow::bail!("unsupported provider capability cannot be executable");
            }
        }
        let mut regions = Vec::with_capacity(descriptor.regions.len());
        for region in descriptor.regions {
            let region = PolicySelector::new(region)
                .context("provider registry region selector is invalid")?;
            if regions.contains(&region) {
                anyhow::bail!("provider registry region selector is duplicated");
            }
            regions.push(region);
        }
        let model_costs = descriptor
            .model_costs
            .into_iter()
            .map(|(model, cost)| (model, cost.runtime_cost()))
            .collect();
        descriptors.push(RuntimeGatewayCompiledProviderDescriptor {
            revision: descriptor.revision,
            pricing_revision: descriptor.pricing_revision,
            provider: descriptor.provider,
            credential_ref: descriptor.credential_ref,
            enabled: descriptor.enabled,
            revoked: descriptor.revoked,
            executable: descriptor.executable,
            upstream_base_url: descriptor.upstream_base_url,
            endpoints: descriptor.endpoints,
            capabilities: descriptor.capabilities,
            regions,
            local_execution: descriptor.local_execution,
            trust_tier: descriptor.trust_tier.into(),
            maximum_classification: descriptor.maximum_classification,
            retention_seconds: descriptor.retention_seconds,
            training_use: descriptor.training_use,
            model_costs: Arc::new(model_costs),
            cost: descriptor.cost,
            latency: descriptor.latency,
            risk: descriptor.risk,
            priority: descriptor.priority,
        });
        debug_assert_eq!(descriptors.len(), index + 1);
    }
    if !descriptors
        .iter()
        .any(|descriptor| descriptor.provider == context.provider)
    {
        anyhow::bail!("provider registry omits attached provider");
    }
    Ok(RuntimeGatewayGovernedProviderRegistrySnapshot {
        revision: artifact.revision,
        authoritative_pricing,
        attached_provider: context.provider,
        projected_credential: context.projected_credential.clone(),
        descriptors,
    })
}

fn runtime_gateway_provider_catalog_has_pricing(provider: ProviderId) -> bool {
    provider_model_catalog(provider).iter().any(|model| {
        model.input_cost_per_million_microusd.is_some()
            || model.output_cost_per_million_microusd.is_some()
    })
}

fn runtime_gateway_model_costs_are_authoritative(
    model_costs: &BTreeMap<String, RuntimeGatewayProviderModelCostArtifact>,
) -> bool {
    !model_costs.is_empty()
        && model_costs.contains_key("*")
        && model_costs.iter().all(|(model, cost)| {
            !model.trim().is_empty()
                && model.len() <= 128
                && (cost.input_cost_per_million_microusd.is_some()
                    || cost.output_cost_per_million_microusd.is_some())
        })
        && !model_costs.keys().enumerate().any(|(index, model)| {
            model_costs
                .keys()
                .take(index)
                .any(|previous| previous.eq_ignore_ascii_case(model))
        })
}

fn runtime_gateway_builtin_model_costs(
    provider: ProviderId,
) -> BTreeMap<String, RuntimeGatewayProviderModelCostArtifact> {
    let mut model_costs = BTreeMap::new();
    let fallback = provider_model_catalog(provider)
        .iter()
        .map(|model| model.cost())
        .fold(ProviderModelCost::default(), max_provider_model_cost);
    let fallback_input = fallback.input_cost_per_million_microusd.unwrap_or_default();
    let fallback_output = fallback
        .output_cost_per_million_microusd
        .unwrap_or_default();
    for model in provider_model_catalog(provider) {
        let cost = model.cost();
        let artifact = RuntimeGatewayProviderModelCostArtifact {
            input_cost_per_million_microusd: Some(
                cost.input_cost_per_million_microusd
                    .unwrap_or(fallback_input),
            ),
            output_cost_per_million_microusd: Some(
                cost.output_cost_per_million_microusd
                    .unwrap_or(fallback_output),
            ),
        };
        insert_case_insensitive_model_cost(&mut model_costs, model.id, artifact);
        for alias in model.aliases {
            insert_case_insensitive_model_cost(&mut model_costs, alias, artifact);
        }
    }
    model_costs.insert(
        "*".to_string(),
        RuntimeGatewayProviderModelCostArtifact {
            input_cost_per_million_microusd: Some(fallback_input),
            output_cost_per_million_microusd: Some(fallback_output),
        },
    );
    model_costs
}

fn insert_case_insensitive_model_cost(
    model_costs: &mut BTreeMap<String, RuntimeGatewayProviderModelCostArtifact>,
    model: &str,
    cost: RuntimeGatewayProviderModelCostArtifact,
) {
    if !model_costs
        .keys()
        .any(|configured| configured.eq_ignore_ascii_case(model))
    {
        model_costs.insert(model.to_string(), cost);
    }
}

fn artifact_descriptors_duplicate_provider(
    descriptors: &[RuntimeGatewayCompiledProviderDescriptor],
    provider: ProviderId,
) -> bool {
    descriptors
        .iter()
        .any(|descriptor| descriptor.provider == provider)
}

fn values_have_duplicate<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> RuntimeLocalRewriteProviderOptions {
        RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["test-key".to_string()],
        }
    }

    fn artifact() -> RuntimeGatewayProviderRegistryArtifact {
        let provider = provider();
        let context = runtime_gateway_attached_provider_registry_context(&provider, None);
        RuntimeGatewayProviderRegistryArtifact {
            schema_version: RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION,
            revision: 7,
            pricing_revision: 4,
            descriptors: vec![RuntimeGatewayProviderRegistryDescriptorArtifact {
                revision: 9,
                pricing_revision: 4,
                provider: context.provider,
                credential_ref: context.credential_ref,
                enabled: true,
                revoked: false,
                executable: true,
                upstream_base_url: None,
                endpoints: context.endpoints,
                capabilities: context.capabilities,
                regions: vec!["*".to_string()],
                local_execution: false,
                trust_tier: RuntimeGatewayProviderRegistryTrustTier::Enterprise,
                maximum_classification: DataClassification::Confidential,
                retention_seconds: 0,
                training_use: false,
                model_costs: runtime_gateway_builtin_model_costs(context.provider),
                cost: 2_000,
                latency: 3_000,
                risk: 1_000,
                priority: 8_000,
            }],
        }
    }

    fn compile(
        artifact: &RuntimeGatewayProviderRegistryArtifact,
    ) -> Result<RuntimeGatewayGovernedProviderRegistrySnapshot> {
        compile_runtime_gateway_provider_registry_artifact(
            &serde_json::to_vec(artifact)?,
            &provider(),
            None,
        )
    }

    fn projected_credential() -> RuntimeProjectedProviderCredential {
        let root = std::env::temp_dir().join(format!(
            "prodex-governed-routing-projected-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        RuntimeProjectedProviderCredential::new(
            SecretRef::new("external", "openai", None::<String>),
            secret_store::ProjectedSecretProvider::new(root, "external").unwrap(),
        )
    }

    #[test]
    fn provider_registry_compiler_rejects_invalid_and_unsupported_executable_adapters() {
        let mut invalid = artifact();
        invalid.revision = 0;
        assert!(compile(&invalid).is_err());

        let mut mismatched_pricing = artifact();
        mismatched_pricing.descriptors[0].pricing_revision += 1;
        assert!(compile(&mismatched_pricing).is_err());

        let mut unsupported = artifact();
        unsupported
            .descriptors
            .push(RuntimeGatewayProviderRegistryDescriptorArtifact {
                revision: 1,
                pricing_revision: 4,
                provider: ProviderId::Anthropic,
                credential_ref: SecretRef::new("projected", "anthropic", None::<String>),
                enabled: true,
                revoked: false,
                executable: true,
                upstream_base_url: Some("https://api.example.com".to_string()),
                endpoints: vec![ProviderEndpoint::Messages],
                capabilities: CapabilitySet::new(vec![]),
                regions: vec!["*".to_string()],
                local_execution: false,
                trust_tier: RuntimeGatewayProviderRegistryTrustTier::Enterprise,
                maximum_classification: DataClassification::Confidential,
                retention_seconds: 0,
                training_use: false,
                model_costs: runtime_gateway_builtin_model_costs(ProviderId::Anthropic),
                cost: 5_000,
                latency: 5_000,
                risk: 5_000,
                priority: 5_000,
            });
        assert!(compile(&unsupported).is_err());
    }

    #[test]
    fn governed_pricing_is_revision_pinned_and_required_when_enforcing() {
        let mut governed = artifact();
        governed.descriptors[0].model_costs = BTreeMap::from([
            (
                "*".to_string(),
                RuntimeGatewayProviderModelCostArtifact {
                    input_cost_per_million_microusd: Some(90),
                    output_cost_per_million_microusd: Some(100),
                },
            ),
            (
                "governed-model".to_string(),
                RuntimeGatewayProviderModelCostArtifact {
                    input_cost_per_million_microusd: Some(30),
                    output_cost_per_million_microusd: Some(40),
                },
            ),
        ]);
        let snapshot = compile(&governed).unwrap();
        assert!(snapshot.has_authoritative_pricing());
        assert_eq!(
            snapshot.reservation_cost_for_model("governed-model"),
            Some(ProviderModelCost {
                input_cost_per_million_microusd: Some(30),
                output_cost_per_million_microusd: Some(40),
            })
        );
        assert_eq!(
            snapshot.reservation_cost_for_model("unknown-model"),
            Some(ProviderModelCost {
                input_cost_per_million_microusd: Some(90),
                output_cost_per_million_microusd: Some(100),
            })
        );

        let mut legacy = governed;
        legacy.schema_version = RUNTIME_GATEWAY_PROVIDER_REGISTRY_LEGACY_SCHEMA_VERSION;
        legacy.descriptors[0].model_costs.clear();
        let encoded = serde_json::to_vec(&legacy).unwrap();
        assert!(compile(&legacy).is_ok());
        assert!(
            compile_runtime_gateway_provider_registry_artifact_for_deployment(
                &encoded,
                &provider(),
                None,
                prodex_config::GovernanceMode::BankEnforce,
            )
            .is_err()
        );
    }

    #[test]
    fn bootstrap_without_declared_catalog_prices_is_not_authoritative() {
        let snapshot = runtime_gateway_bootstrap_provider_registry_snapshot(
            &prodex_runtime_policy::RuntimePolicyGovernanceSettings::default(),
            &provider(),
            None,
        )
        .unwrap();

        assert!(!snapshot.has_authoritative_pricing());
        assert_eq!(snapshot.reservation_cost_for_model("unknown-model"), None);
    }

    #[test]
    fn provider_registry_resolves_selected_heterogeneous_projected_adapter() {
        let credential = projected_credential();
        let mut artifact = artifact();
        artifact.descriptors[0].credential_ref = credential.reference().clone();
        artifact.descriptors[0].priority = 0;
        artifact
            .descriptors
            .push(RuntimeGatewayProviderRegistryDescriptorArtifact {
                revision: 10,
                pricing_revision: 4,
                provider: ProviderId::Anthropic,
                credential_ref: SecretRef::new("external", "anthropic", None::<String>),
                enabled: true,
                revoked: false,
                executable: true,
                upstream_base_url: Some("https://api.example.com".to_string()),
                endpoints: vec![ProviderEndpoint::Responses],
                capabilities: runtime_gateway_provider_executable_capabilities(
                    ProviderId::Anthropic,
                ),
                regions: vec!["*".to_string()],
                local_execution: false,
                trust_tier: RuntimeGatewayProviderRegistryTrustTier::Enterprise,
                maximum_classification: DataClassification::Confidential,
                retention_seconds: 0,
                training_use: false,
                model_costs: runtime_gateway_builtin_model_costs(ProviderId::Anthropic),
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 10_000,
            });
        let snapshot = compile_runtime_gateway_provider_registry_artifact(
            &serde_json::to_vec(&artifact).unwrap(),
            &provider(),
            Some(&credential),
        )
        .unwrap();
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let registry =
            snapshot.for_tenant(tenant, ProviderEndpoint::Responses, &Default::default());
        let policy = prodex_domain::PolicyDecision {
            effect: prodex_domain::PolicyEffect::Allow,
            obligations: Vec::new(),
            reason_codes: Vec::new(),
            policy_revision: prodex_domain::PolicyRevisionId::new(),
            valid_until_unix_ms: u64::MAX,
        };
        let required = CapabilitySet::new(vec![prodex_domain::ModelCapability::ResponsesApi]);
        let routing = prodex_provider_spi::plan_governed_provider_route(
            &prodex_provider_spi::GovernedRoutingRequest {
                tenant,
                classification: DataClassification::Internal,
                required_capabilities: &required,
                policy: &policy,
                registry: &registry,
                score_revision: 1,
                weights: GovernedRoutingWeights::default(),
                affinity_provider: None,
                max_fallbacks: 1,
            },
        )
        .unwrap();

        assert_eq!(routing.primary.provider, ProviderId::Anthropic);
        assert_eq!(routing.fallbacks.len(), 1);
        let execution = snapshot
            .execution_for_route(&routing.primary, ProviderEndpoint::Responses)
            .unwrap();
        assert_eq!(
            execution.provider.bridge_kind().provider_id(),
            ProviderId::Anthropic
        );
        assert_eq!(
            execution.credential.reference(),
            &routing.primary.credential_ref
        );
        assert_eq!(execution.upstream_base_url, "https://api.example.com");
    }

    #[test]
    fn provider_registry_snapshot_set_is_tenant_bound_and_retains_lkg_on_invalid_refresh() {
        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();
        let snapshot = compile(&artifact()).unwrap();
        let set = RuntimeGatewayProviderRegistrySnapshotSet::bootstrap(snapshot.clone(), false)
            .with_tenant_snapshot(tenant_a, snapshot)
            .unwrap();

        assert!(set.snapshot_for(tenant_a).is_some());
        assert!(set.snapshot_for(tenant_b).is_none());

        let mut invalid = artifact();
        invalid.descriptors[0].credential_ref =
            SecretRef::new("projected", "wrong", None::<String>);
        assert!(compile(&invalid).is_err());
        assert_eq!(set.snapshot_for(tenant_a).unwrap().revision, 7);
    }

    #[test]
    fn provider_registry_revocation_revalidates_before_dispatch() {
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let active = compile(&artifact()).unwrap();
        let registry = active.for_tenant(tenant, ProviderEndpoint::Responses, &Default::default());
        let route = &registry.providers[0];
        let routing = GovernedRoutingPlan {
            tenant,
            registry_revision: registry.revision,
            score_revision: 1,
            policy_revision: prodex_domain::PolicyRevisionId::new(),
            primary: prodex_provider_spi::GovernedRoute {
                provider: route.provider,
                descriptor_revision: route.revision,
                pricing_revision: route.pricing_revision,
                credential_ref: route.credential_ref.clone(),
                score: 0,
                score_breakdown: prodex_provider_spi::GovernedScoreBreakdown {
                    score_revision: 1,
                    components: std::array::from_fn(|_| {
                        prodex_provider_spi::GovernedScoreComponent {
                            kind: prodex_provider_spi::GovernedScoreComponentKind::Health,
                            normalized_value: 0,
                            weight: 0,
                            weighted_value: 0,
                        }
                    }),
                    weighted_total: 0,
                    weight_total: 1,
                    score: 0,
                },
            },
            fallbacks: Vec::new(),
            candidate_evaluations: Vec::new(),
        };
        assert!(active.matches_route(&routing, ProviderEndpoint::Responses));

        let mut revoked = artifact();
        revoked.descriptors[0].revoked = true;
        assert!(
            !compile(&revoked)
                .unwrap()
                .matches_route(&routing, ProviderEndpoint::Responses)
        );

        let mut repriced = artifact();
        repriced.pricing_revision += 1;
        repriced.descriptors[0].pricing_revision += 1;
        assert!(
            !compile(&repriced)
                .unwrap()
                .matches_route(&routing, ProviderEndpoint::Responses)
        );
    }

    #[test]
    fn provider_registry_projects_bounded_runtime_signals_without_probing() {
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let active = compile(&artifact()).unwrap();
        let mut runtime = RuntimeGatewayProviderRuntimeSnapshot::default();
        runtime.insert(
            ProviderId::OpenAi,
            RuntimeGatewayProviderRuntimeSignals {
                health: Some(2_500),
                load: 7_500,
                quota_headroom: Some(1_500),
                circuit_open: true,
                quota_available: false,
                inflight_cap_reached: true,
            },
        );

        let registry = active.for_tenant(tenant, ProviderEndpoint::Responses, &runtime);
        let descriptor = &registry.providers[0];
        assert_eq!(descriptor.signals.health, Some(2_500));
        assert_eq!(descriptor.signals.load, 7_500);
        assert_eq!(descriptor.signals.quota_headroom, Some(1_500));
        assert!(descriptor.circuit_open);
        assert!(!descriptor.quota_available);
        assert!(descriptor.inflight_cap_reached);
    }

    fn routing_scores_artifact(revision: u64, cost: u16) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "revision": revision,
            "weights": {
                "health": 2_000,
                "load": 1_000,
                "cost": cost,
                "latency": 1_000,
                "risk": 1_000,
                "priority": 1_000,
                "affinity": 1_000
            }
        }))
        .unwrap()
    }

    #[test]
    fn routing_scores_compiler_is_deterministic_and_bounded() {
        let artifact = routing_scores_artifact(11, 3_000);
        let first = compile_runtime_gateway_routing_scores_artifact(&artifact).unwrap();
        let second = compile_runtime_gateway_routing_scores_artifact(&artifact).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.revision, 11);
        assert_eq!(first.weights.cost, 3_000);

        assert!(
            compile_runtime_gateway_routing_scores_artifact(&routing_scores_artifact(11, 10_001))
                .is_err()
        );
        assert!(
            compile_runtime_gateway_routing_scores_artifact(&routing_scores_artifact(0, 3_000))
                .is_err()
        );
    }

    #[test]
    fn routing_scores_are_tenant_bound_and_invalid_refresh_retains_lkg() {
        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();
        let snapshot =
            compile_runtime_gateway_routing_scores_artifact(&routing_scores_artifact(13, 3_000))
                .unwrap();
        let set = RuntimeGatewayRoutingScoresSnapshotSet::bootstrap(snapshot, false)
            .with_tenant_snapshot(tenant_a, snapshot)
            .unwrap();

        assert_eq!(set.snapshot_for(tenant_a).unwrap().revision, 13);
        assert!(set.snapshot_for(tenant_b).is_none());
        assert!(
            compile_runtime_gateway_routing_scores_artifact(&routing_scores_artifact(14, 10_001))
                .is_err()
        );
        assert_eq!(set.snapshot_for(tenant_a).unwrap().revision, 13);
    }
}
