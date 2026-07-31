use super::{
    Arc, BTreeMap, Context as _, Deserialize, GovernedRoutingWeights,
    MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_ARTIFACT_BYTES, ProviderId, ProviderModelCost,
    RUNTIME_GATEWAY_ROUTING_SCORES_SCHEMA_VERSION, Result, RuntimeAnthropicProviderAuth,
    RuntimeCopilotProviderAuth, RuntimeDeepSeekWebSearchMode,
    RuntimeGatewayGovernedProviderRegistrySnapshot, RuntimeGeminiProviderAuth,
    RuntimeLocalRewriteProviderOptions, Serialize, TenantId,
};

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_model_cost(
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

pub(in crate::runtime_launch::proxy_startup) fn max_provider_model_cost(
    left: ProviderModelCost,
    right: ProviderModelCost,
) -> ProviderModelCost {
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

pub(in crate::runtime_launch::proxy_startup) fn max_optional_cost(
    left: Option<u64>,
    right: Option<u64>,
) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_projected_provider_options(
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
pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayTenantSnapshotSet<T> {
    pub(super) tenant_snapshots: BTreeMap<TenantId, Arc<T>>,
    pub(super) fallback: Option<Arc<T>>,
}

impl<T> Clone for RuntimeGatewayTenantSnapshotSet<T> {
    fn clone(&self) -> Self {
        Self {
            tenant_snapshots: self.tenant_snapshots.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

pub(in crate::runtime_launch::proxy_startup) type RuntimeGatewayProviderRegistrySnapshotSet =
    RuntimeGatewayTenantSnapshotSet<RuntimeGatewayGovernedProviderRegistrySnapshot>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayRoutingScoresSnapshot {
    pub(in crate::runtime_launch::proxy_startup) revision: u64,
    pub(in crate::runtime_launch::proxy_startup) weights: GovernedRoutingWeights,
}

pub(in crate::runtime_launch::proxy_startup) type RuntimeGatewayRoutingScoresSnapshotSet =
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_bootstrap_routing_scores_snapshot(
    settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
) -> RuntimeGatewayRoutingScoresSnapshot {
    RuntimeGatewayRoutingScoresSnapshot {
        revision: settings.routing_score_revision.unwrap_or(1),
        weights: GovernedRoutingWeights::default(),
    }
}

pub(in crate::runtime_launch::proxy_startup) fn compile_runtime_gateway_routing_scores_artifact(
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
