#[derive(Debug, PartialEq, Eq)]
pub(super) struct RuntimeGatewayBootstrapDescriptorPlan {
    pub(super) schema_version: u32,
    pub(super) registry_revision: u64,
    pub(super) descriptor_revision: u64,
    pub(super) pricing_revision: u64,
    pub(super) enabled: bool,
    pub(super) revoked: bool,
    pub(super) local_execution: bool,
    pub(super) trust_tier: RuntimeGatewayProviderRegistryTrustTier,
    pub(super) maximum_classification: DataClassification,
    pub(super) retention_seconds: u32,
    pub(super) training_use: bool,
    pub(super) use_default_region: bool,
    pub(super) cost: u16,
    pub(super) latency: u16,
    pub(super) risk: u16,
    pub(super) priority: u16,
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_gateway_bootstrap_descriptor_plan(
    settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    pricing_known: bool,
) -> RuntimeGatewayBootstrapDescriptorPlan {
    use prodex_mojo_core::policy::{
        ProviderRegistryBootstrapSettings, ProviderRegistryClassification,
        ProviderRegistryTrustTier,
    };
    let provider = settings
        .provider
        .as_ref()
        .map(|settings| ProviderRegistryBootstrapSettings {
            descriptor_revision: settings.descriptor_revision,
            enabled: settings.enabled,
            revoked: settings.revoked,
            local_execution: settings.local_execution,
            trust_tier: match settings.trust_tier {
                prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Standard => {
                    ProviderRegistryTrustTier::Standard
                }
                prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Enterprise => {
                    ProviderRegistryTrustTier::Enterprise
                }
                prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::RestrictedApproved => {
                    ProviderRegistryTrustTier::RestrictedApproved
                }
            },
            maximum_classification: match settings.maximum_classification {
                prodex_runtime_policy::RuntimeGovernanceDataClassification::Public => {
                    ProviderRegistryClassification::Public
                }
                prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal => {
                    ProviderRegistryClassification::Internal
                }
                prodex_runtime_policy::RuntimeGovernanceDataClassification::Confidential => {
                    ProviderRegistryClassification::Confidential
                }
                prodex_runtime_policy::RuntimeGovernanceDataClassification::Restricted => {
                    ProviderRegistryClassification::Restricted
                }
            },
            retention_seconds: settings.retention_seconds,
            training_use: settings.training_use,
            regions_present: !settings.regions.is_empty(),
        });
    let plan = prodex_mojo_core::policy::plan_provider_registry_bootstrap(
        pricing_known,
        settings.provider_registry_revision,
        provider,
    )
    .expect("validated runtime policy must produce a provider-registry bootstrap plan");
    RuntimeGatewayBootstrapDescriptorPlan {
        schema_version: plan.schema_version,
        registry_revision: plan.registry_revision,
        descriptor_revision: plan.descriptor_revision,
        pricing_revision: plan.pricing_revision,
        enabled: plan.enabled,
        revoked: plan.revoked,
        local_execution: plan.local_execution,
        trust_tier: match plan.trust_tier {
            ProviderRegistryTrustTier::Standard => {
                RuntimeGatewayProviderRegistryTrustTier::Standard
            }
            ProviderRegistryTrustTier::Enterprise => {
                RuntimeGatewayProviderRegistryTrustTier::Enterprise
            }
            ProviderRegistryTrustTier::RestrictedApproved => {
                RuntimeGatewayProviderRegistryTrustTier::RestrictedApproved
            }
        },
        maximum_classification: match plan.maximum_classification {
            ProviderRegistryClassification::Public => DataClassification::Public,
            ProviderRegistryClassification::Internal => DataClassification::Internal,
            ProviderRegistryClassification::Confidential => DataClassification::Confidential,
            ProviderRegistryClassification::Restricted => DataClassification::Restricted,
        },
        retention_seconds: plan.retention_seconds,
        training_use: plan.training_use,
        use_default_region: plan.use_default_region,
        cost: plan.cost,
        latency: plan.latency,
        risk: plan.risk,
        priority: plan.priority,
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_gateway_bootstrap_descriptor_plan(
    settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    pricing_known: bool,
) -> RuntimeGatewayBootstrapDescriptorPlan {
    rust_bootstrap_descriptor_plan(settings, pricing_known)
}

#[cfg(any(test, not(feature = "mojo-core")))]
fn rust_bootstrap_descriptor_plan(
    settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    pricing_known: bool,
) -> RuntimeGatewayBootstrapDescriptorPlan {
    let provider = settings.provider.as_ref();
    let trust_tier = match provider.map(|settings| settings.trust_tier) {
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Enterprise) => {
            RuntimeGatewayProviderRegistryTrustTier::Enterprise
        }
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::RestrictedApproved) => {
            RuntimeGatewayProviderRegistryTrustTier::RestrictedApproved
        }
        _ => RuntimeGatewayProviderRegistryTrustTier::Standard,
    };
    RuntimeGatewayBootstrapDescriptorPlan {
        schema_version: if pricing_known { 2 } else { 1 },
        registry_revision: settings.provider_registry_revision.unwrap_or(1),
        descriptor_revision: provider.map_or(1, |settings| settings.descriptor_revision),
        pricing_revision: 1,
        enabled: provider.is_none_or(|settings| settings.enabled),
        revoked: provider.is_some_and(|settings| settings.revoked),
        local_execution: provider.is_some_and(|settings| settings.local_execution),
        trust_tier,
        maximum_classification: match provider.map(|settings| settings.maximum_classification) {
            Some(prodex_runtime_policy::RuntimeGovernanceDataClassification::Public) => {
                DataClassification::Public
            }
            Some(prodex_runtime_policy::RuntimeGovernanceDataClassification::Confidential) => {
                DataClassification::Confidential
            }
            Some(prodex_runtime_policy::RuntimeGovernanceDataClassification::Restricted) => {
                DataClassification::Restricted
            }
            _ => DataClassification::Internal,
        },
        retention_seconds: provider.map_or(u32::MAX, |settings| settings.retention_seconds),
        training_use: provider.is_none_or(|settings| settings.training_use),
        use_default_region: provider.is_none_or(|settings| settings.regions.is_empty()),
        cost: 5_000,
        latency: 5_000,
        risk: match trust_tier {
            RuntimeGatewayProviderRegistryTrustTier::Standard => 8_000,
            RuntimeGatewayProviderRegistryTrustTier::Enterprise => 4_000,
            RuntimeGatewayProviderRegistryTrustTier::RestrictedApproved => 1_000,
        },
        priority: 5_000,
    }
}

#[cfg(all(test, feature = "mojo-core"))]
mod tests {
    use super::*;

    #[test]
    fn mojo_bootstrap_plan_matches_non_mojo_defaults_and_configured_policy() {
        let mut settings = prodex_runtime_policy::RuntimePolicyGovernanceSettings::default();
        for pricing_known in [false, true] {
            assert_eq!(
                runtime_gateway_bootstrap_descriptor_plan(&settings, pricing_known),
                rust_bootstrap_descriptor_plan(&settings, pricing_known)
            );
        }
        settings.provider_registry_revision = Some(9);
        settings.provider = Some(
            prodex_runtime_policy::RuntimePolicyGovernanceProviderSettings {
                descriptor_revision: 7,
                enabled: false,
                revoked: true,
                trust_tier:
                    prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::RestrictedApproved,
                local_execution: true,
                maximum_classification:
                    prodex_runtime_policy::RuntimeGovernanceDataClassification::Restricted,
                regions: vec!["ap-southeast-1".to_string()],
                retention_seconds: 60,
                training_use: false,
            },
        );
        assert_eq!(
            runtime_gateway_bootstrap_descriptor_plan(&settings, true),
            rust_bootstrap_descriptor_plan(&settings, true)
        );
    }
}
use super::{DataClassification, RuntimeGatewayProviderRegistryTrustTier};
