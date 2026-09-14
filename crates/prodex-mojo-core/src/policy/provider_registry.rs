use crate::MojoError;

use super::gateway_admin::GATEWAY_ADMIN_POLICY_ABI_VERSION;

const OUTPUT_WIDTH: usize = 16;

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRegistryTrustTier {
    Standard = 0,
    Enterprise = 1,
    RestrictedApproved = 2,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRegistryClassification {
    Public = 0,
    Internal = 1,
    Confidential = 2,
    Restricted = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRegistryBootstrapSettings {
    pub descriptor_revision: u64,
    pub enabled: bool,
    pub revoked: bool,
    pub local_execution: bool,
    pub trust_tier: ProviderRegistryTrustTier,
    pub maximum_classification: ProviderRegistryClassification,
    pub retention_seconds: u32,
    pub training_use: bool,
    pub regions_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRegistryBootstrapPlan {
    pub schema_version: u32,
    pub registry_revision: u64,
    pub descriptor_revision: u64,
    pub enabled: bool,
    pub revoked: bool,
    pub local_execution: bool,
    pub trust_tier: ProviderRegistryTrustTier,
    pub maximum_classification: ProviderRegistryClassification,
    pub retention_seconds: u32,
    pub training_use: bool,
    pub cost: u16,
    pub latency: u16,
    pub risk: u16,
    pub priority: u16,
    pub use_default_region: bool,
    pub pricing_revision: u64,
}

unsafe extern "C" {
    fn prodex_mojo_provider_registry_bootstrap_plan_v1(
        abi_version: i64,
        pricing_known: i64,
        registry_revision: u64,
        registry_revision_present: i64,
        provider_present: i64,
        regions_present: i64,
        descriptor_revision: u64,
        enabled: i64,
        revoked: i64,
        local_execution: i64,
        trust_tier: i64,
        maximum_classification: i64,
        retention_seconds: u64,
        training_use: i64,
        output: u64,
        output_capacity: i64,
    ) -> i64;
}

pub fn plan_provider_registry_bootstrap(
    pricing_known: bool,
    registry_revision: Option<u64>,
    settings: Option<ProviderRegistryBootstrapSettings>,
) -> Result<ProviderRegistryBootstrapPlan, MojoError> {
    let provider_present = settings.is_some();
    let settings = settings.unwrap_or(ProviderRegistryBootstrapSettings {
        descriptor_revision: 0,
        enabled: false,
        revoked: false,
        local_execution: false,
        trust_tier: ProviderRegistryTrustTier::Standard,
        maximum_classification: ProviderRegistryClassification::Public,
        retention_seconds: 0,
        training_use: false,
        regions_present: false,
    });
    let mut output = [0_u64; OUTPUT_WIDTH];
    let status = unsafe {
        prodex_mojo_provider_registry_bootstrap_plan_v1(
            GATEWAY_ADMIN_POLICY_ABI_VERSION,
            i64::from(pricing_known),
            registry_revision.unwrap_or_default(),
            i64::from(registry_revision.is_some()),
            i64::from(provider_present),
            i64::from(settings.regions_present),
            settings.descriptor_revision,
            i64::from(settings.enabled),
            i64::from(settings.revoked),
            i64::from(settings.local_execution),
            settings.trust_tier as i64,
            settings.maximum_classification as i64,
            u64::from(settings.retention_seconds),
            i64::from(settings.training_use),
            output.as_mut_ptr() as usize as u64,
            OUTPUT_WIDTH as i64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let boolean = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let trust_tier = match output[6] {
        0 => ProviderRegistryTrustTier::Standard,
        1 => ProviderRegistryTrustTier::Enterprise,
        2 => ProviderRegistryTrustTier::RestrictedApproved,
        _ => return Err(MojoError::InvalidOutput),
    };
    let maximum_classification = match output[7] {
        0 => ProviderRegistryClassification::Public,
        1 => ProviderRegistryClassification::Internal,
        2 => ProviderRegistryClassification::Confidential,
        3 => ProviderRegistryClassification::Restricted,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(ProviderRegistryBootstrapPlan {
        schema_version: u32::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        registry_revision: output[1],
        descriptor_revision: output[2],
        enabled: boolean(output[3])?,
        revoked: boolean(output[4])?,
        local_execution: boolean(output[5])?,
        trust_tier,
        maximum_classification,
        retention_seconds: u32::try_from(output[8]).map_err(|_| MojoError::InvalidOutput)?,
        training_use: boolean(output[9])?,
        cost: u16::try_from(output[10]).map_err(|_| MojoError::InvalidOutput)?,
        latency: u16::try_from(output[11]).map_err(|_| MojoError::InvalidOutput)?,
        risk: u16::try_from(output[12]).map_err(|_| MojoError::InvalidOutput)?,
        priority: u16::try_from(output[13]).map_err(|_| MojoError::InvalidOutput)?,
        use_default_region: boolean(output[14])?,
        pricing_revision: output[15],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_plan_owns_defaults_and_trust_risk() {
        assert_eq!(
            plan_provider_registry_bootstrap(false, None, None).unwrap(),
            ProviderRegistryBootstrapPlan {
                schema_version: 1,
                registry_revision: 1,
                descriptor_revision: 1,
                enabled: true,
                revoked: false,
                local_execution: false,
                trust_tier: ProviderRegistryTrustTier::Standard,
                maximum_classification: ProviderRegistryClassification::Internal,
                retention_seconds: u32::MAX,
                training_use: true,
                cost: 5_000,
                latency: 5_000,
                risk: 8_000,
                priority: 5_000,
                use_default_region: true,
                pricing_revision: 1,
            }
        );
        let plan = plan_provider_registry_bootstrap(
            true,
            Some(9),
            Some(ProviderRegistryBootstrapSettings {
                descriptor_revision: 7,
                enabled: false,
                revoked: true,
                local_execution: true,
                trust_tier: ProviderRegistryTrustTier::RestrictedApproved,
                maximum_classification: ProviderRegistryClassification::Restricted,
                retention_seconds: 60,
                training_use: false,
                regions_present: true,
            }),
        )
        .unwrap();
        assert_eq!(plan.schema_version, 2);
        assert_eq!(plan.registry_revision, 9);
        assert_eq!(plan.descriptor_revision, 7);
        assert_eq!(plan.risk, 1_000);
    }
}
