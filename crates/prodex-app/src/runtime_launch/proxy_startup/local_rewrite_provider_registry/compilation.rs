use super::bootstrap_planning::runtime_gateway_bootstrap_descriptor_plan;
use super::*;

pub(super) struct RuntimeGatewayAttachedProviderRegistryContext {
    pub(super) provider: ProviderId,
    pub(super) credential_ref: SecretRef,
    projected_credential: Option<RuntimeProjectedProviderCredential>,
    pub(super) endpoints: Vec<ProviderEndpoint>,
    pub(super) capabilities: CapabilitySet,
}

pub(super) fn runtime_gateway_attached_provider_registry_context(
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_bootstrap_provider_registry_snapshot(
    settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    provider_options: &RuntimeLocalRewriteProviderOptions,
    credential: Option<&RuntimeProjectedProviderCredential>,
) -> Result<RuntimeGatewayGovernedProviderRegistrySnapshot> {
    let context = runtime_gateway_attached_provider_registry_context(provider_options, credential);
    let builtin_cost_plan = runtime_gateway_builtin_model_cost_plan(context.provider);
    let bootstrap =
        runtime_gateway_bootstrap_descriptor_plan(settings, builtin_cost_plan.pricing_known);
    let regions = if bootstrap.use_default_region {
        vec!["*".to_string()]
    } else {
        settings
            .provider
            .as_ref()
            .map_or_else(Vec::new, |settings| settings.regions.clone())
    };
    compile_runtime_gateway_provider_registry_artifact(
        &serde_json::to_vec(&RuntimeGatewayProviderRegistryArtifact {
            schema_version: bootstrap.schema_version,
            revision: bootstrap.registry_revision,
            pricing_revision: bootstrap.pricing_revision,
            descriptors: vec![RuntimeGatewayProviderRegistryDescriptorArtifact {
                revision: bootstrap.descriptor_revision,
                pricing_revision: bootstrap.pricing_revision,
                provider: context.provider,
                credential_ref: context.credential_ref.clone(),
                enabled: bootstrap.enabled,
                revoked: bootstrap.revoked,
                executable: true,
                upstream_base_url: None,
                endpoints: context.endpoints.clone(),
                capabilities: context.capabilities.clone(),
                regions,
                local_execution: bootstrap.local_execution,
                trust_tier: bootstrap.trust_tier,
                maximum_classification: bootstrap.maximum_classification,
                retention_seconds: bootstrap.retention_seconds,
                training_use: bootstrap.training_use,
                model_costs: builtin_cost_plan.model_costs,
                cost: bootstrap.cost,
                latency: bootstrap.latency,
                risk: bootstrap.risk,
                priority: bootstrap.priority,
            }],
        })
        .context("failed to encode bootstrap provider registry")?,
        provider_options,
        credential,
    )
}

pub(in crate::runtime_launch::proxy_startup) fn compile_runtime_gateway_provider_registry_artifact(
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

pub(in crate::runtime_launch::proxy_startup) fn compile_runtime_gateway_provider_registry_artifact_for_deployment(
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
    runtime_gateway_validate_provider_registry_structure(&artifact)?;
    let authoritative_pricing =
        artifact.schema_version == RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION;
    let mut descriptors = Vec::with_capacity(artifact.descriptors.len());
    for (index, descriptor) in artifact.descriptors.into_iter().enumerate() {
        runtime_gateway_validate_provider_descriptor_attachment(&descriptor, context)?;
        descriptors.push(runtime_gateway_compile_provider_descriptor(descriptor)?);
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

fn runtime_gateway_validate_provider_descriptor_attachment(
    descriptor: &RuntimeGatewayProviderRegistryDescriptorArtifact,
    context: &RuntimeGatewayAttachedProviderRegistryContext,
) -> Result<()> {
    if descriptor.provider == context.provider {
        if !descriptor.executable {
            anyhow::bail!("attached provider registry descriptor is invalid");
        }
        if descriptor.credential_ref != context.credential_ref {
            anyhow::bail!("attached provider registry descriptor is invalid");
        }
        if descriptor
            .endpoints
            .iter()
            .any(|endpoint| !context.endpoints.contains(endpoint))
        {
            anyhow::bail!("attached provider registry descriptor is invalid");
        }
        if !descriptor
            .capabilities
            .missing_from(&context.capabilities)
            .is_empty()
        {
            anyhow::bail!("attached provider registry descriptor is invalid");
        }
        return Ok(());
    }
    if !descriptor.executable {
        return Ok(());
    }
    runtime_gateway_validate_projected_provider_descriptor(descriptor, context)
}

fn runtime_gateway_validate_projected_provider_descriptor(
    descriptor: &RuntimeGatewayProviderRegistryDescriptorArtifact,
    context: &RuntimeGatewayAttachedProviderRegistryContext,
) -> Result<()> {
    let Some(projected_credential) = context.projected_credential.as_ref() else {
        anyhow::bail!("heterogeneous provider requires projected credentials");
    };
    if projected_credential.reference().provider() != descriptor.credential_ref.provider() {
        anyhow::bail!("unsupported provider adapter cannot be executable");
    }
    if descriptor.upstream_base_url.as_deref().is_none_or(|value| {
        crate::validate_credential_free_http_url(value, "provider registry upstream base URL")
            .is_err()
    }) {
        anyhow::bail!("unsupported provider adapter cannot be executable");
    }
    if runtime_gateway_projected_provider_options(
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
    }) {
        anyhow::bail!("unsupported provider capability cannot be executable");
    }
    if !descriptor
        .capabilities
        .missing_from(&runtime_gateway_provider_executable_capabilities(
            descriptor.provider,
        ))
        .is_empty()
    {
        anyhow::bail!("unsupported provider capability cannot be executable");
    }
    Ok(())
}

fn runtime_gateway_compile_provider_descriptor(
    descriptor: RuntimeGatewayProviderRegistryDescriptorArtifact,
) -> Result<RuntimeGatewayCompiledProviderDescriptor> {
    let regions = runtime_gateway_compile_provider_regions(descriptor.regions)?;
    let model_costs = descriptor
        .model_costs
        .into_iter()
        .map(|(model, cost)| (model, cost.runtime_cost()))
        .collect();
    Ok(RuntimeGatewayCompiledProviderDescriptor {
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
    })
}

fn runtime_gateway_compile_provider_regions(regions: Vec<String>) -> Result<Vec<PolicySelector>> {
    let mut compiled = Vec::with_capacity(regions.len());
    for region in regions {
        let region =
            PolicySelector::new(region).context("provider registry region selector is invalid")?;
        if cfg!(not(feature = "mojo-core")) && compiled.contains(&region) {
            anyhow::bail!("provider registry region selector is duplicated");
        }
        compiled.push(region);
    }
    Ok(compiled)
}
