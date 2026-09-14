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
use prodex_provider_core::{ProviderEndpoint, ProviderId, ProviderModelCost, provider_adapter};
use prodex_provider_spi::{
    GovernedProviderDescriptor, GovernedProviderRegistry, GovernedRoute, GovernedRoutingPlan,
    GovernedRoutingSignals, GovernedRoutingWeights,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

mod compilation;
mod provider_planning;
mod routing;
mod snapshot;
mod tenant_snapshot_set;
mod validation;
pub(super) use self::routing::{
    RuntimeGatewayProviderRegistrySnapshotSet, RuntimeGatewayRoutingScoresSnapshot,
    RuntimeGatewayRoutingScoresSnapshotSet, RuntimeGatewayTenantSnapshotSet,
    compile_runtime_gateway_routing_scores_artifact, max_provider_model_cost,
    runtime_gateway_bootstrap_routing_scores_snapshot, runtime_gateway_model_cost,
    runtime_gateway_projected_provider_options,
};
#[cfg(test)]
use compilation::{
    compile_runtime_gateway_provider_registry_artifact,
    runtime_gateway_attached_provider_registry_context,
};
pub(in crate::runtime_launch::proxy_startup) use compilation::{
    compile_runtime_gateway_provider_registry_artifact_for_deployment,
    runtime_gateway_bootstrap_provider_registry_snapshot,
};
use provider_planning::runtime_gateway_builtin_model_cost_plan;
use validation::runtime_gateway_validate_provider_registry_structure;

const RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION: u32 = 2;
const RUNTIME_GATEWAY_PROVIDER_REGISTRY_LEGACY_SCHEMA_VERSION: u32 = 1;
const RUNTIME_GATEWAY_ROUTING_SCORES_SCHEMA_VERSION: u32 = 1;
#[cfg(not(feature = "mojo-core"))]
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
                model_costs: runtime_gateway_builtin_model_cost_plan(context.provider).model_costs,
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
                model_costs: runtime_gateway_builtin_model_cost_plan(ProviderId::Anthropic)
                    .model_costs,
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
                model_costs: runtime_gateway_builtin_model_cost_plan(ProviderId::Anthropic)
                    .model_costs,
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
