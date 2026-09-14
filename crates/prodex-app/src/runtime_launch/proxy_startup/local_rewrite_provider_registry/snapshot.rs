use super::*;

impl RuntimeGatewayGovernedProviderRegistrySnapshot {
    pub(in crate::runtime_launch::proxy_startup) fn revision(&self) -> u64 {
        self.revision
    }

    pub(in crate::runtime_launch::proxy_startup) fn provider_ids(
        &self,
    ) -> impl Iterator<Item = ProviderId> + '_ {
        self.descriptors
            .iter()
            .map(|descriptor| descriptor.provider)
    }

    pub(in crate::runtime_launch::proxy_startup) fn has_authoritative_pricing(&self) -> bool {
        self.authoritative_pricing
    }

    pub(in crate::runtime_launch::proxy_startup) fn reservation_cost_for_model(
        &self,
        model: &str,
    ) -> Option<ProviderModelCost> {
        self.authoritative_pricing.then_some(())?;
        self.descriptors
            .iter()
            .filter(|descriptor| descriptor.enabled && descriptor.executable && !descriptor.revoked)
            .filter_map(|descriptor| runtime_gateway_model_cost(&descriptor.model_costs, model))
            .reduce(max_provider_model_cost)
    }

    pub(in crate::runtime_launch::proxy_startup) fn runtime_profile_name(
        &self,
        provider: ProviderId,
    ) -> &str {
        if provider == self.attached_provider {
            super::super::local_rewrite::RUNTIME_LOCAL_REWRITE_PROFILE
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

    pub(in crate::runtime_launch::proxy_startup) fn for_tenant(
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

    pub(in crate::runtime_launch::proxy_startup) fn matches_route(
        &self,
        routing: &GovernedRoutingPlan,
        endpoint: ProviderEndpoint,
    ) -> bool {
        self.matches_governed_route(routing.registry_revision, &routing.primary, endpoint)
    }

    pub(in crate::runtime_launch::proxy_startup) fn matches_governed_route(
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

    pub(in crate::runtime_launch::proxy_startup) fn execution_for_route(
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

    pub(in crate::runtime_launch::proxy_startup) fn pricing_for_route(
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
