use crate::MojoError;

pub const PROVIDER_REGISTRY_ABI_VERSION: i64 = 1;
pub const PROVIDER_REGISTRY_PROVIDER_COUNT: usize = 7;
pub const PROVIDER_REGISTRY_ENDPOINT_COUNT: usize = 11;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ProviderRegistryPlanRaw {
    abi_version: i64,
    provider: i64,
    client_wire: i64,
    upstream_wire: i64,
    response_wire: i64,
    supports_streaming: i64,
    supports_model_fallback: i64,
    endpoint_mask: u64,
    passthrough_mask: u64,
    capability_bits: u64,
}

const _: () = assert!(std::mem::size_of::<ProviderRegistryPlanRaw>() == 80);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRegistryPlan {
    pub provider: i64,
    pub client_wire: i64,
    pub upstream_wire: i64,
    pub response_wire: i64,
    pub supports_streaming: bool,
    pub supports_model_fallback: bool,
    pub endpoint_mask: u64,
    pub passthrough_mask: u64,
    pub capability_statuses: [i64; PROVIDER_REGISTRY_ENDPOINT_COUNT],
}

unsafe extern "C" {
    fn prodex_mojo_provider_registry_plan_v1(
        abi_version: i64,
        provider: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_provider_registry_resolve_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        include_model_provider_ids: i64,
        output_address: u64,
    ) -> i64;
}

fn status_error(status: i64) -> MojoError {
    match status {
        1 => MojoError::InvalidInput,
        2 => MojoError::InvalidOutput,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

pub fn provider_plan(provider: i64) -> Result<ProviderRegistryPlan, MojoError> {
    if !(0..PROVIDER_REGISTRY_PROVIDER_COUNT as i64).contains(&provider) {
        return Err(MojoError::InvalidInput);
    }
    let mut raw = ProviderRegistryPlanRaw::default();
    let status = unsafe {
        prodex_mojo_provider_registry_plan_v1(
            PROVIDER_REGISTRY_ABI_VERSION,
            provider,
            (&mut raw as *mut ProviderRegistryPlanRaw) as u64,
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    if raw.abi_version != PROVIDER_REGISTRY_ABI_VERSION
        || raw.provider != provider
        || !(0..=4).contains(&raw.client_wire)
        || !(0..=4).contains(&raw.upstream_wire)
        || !(0..=4).contains(&raw.response_wire)
        || !matches!(raw.supports_streaming, 0 | 1)
        || !matches!(raw.supports_model_fallback, 0 | 1)
        || raw.endpoint_mask >> PROVIDER_REGISTRY_ENDPOINT_COUNT != 0
        || raw.passthrough_mask >> PROVIDER_REGISTRY_ENDPOINT_COUNT != 0
        || raw.passthrough_mask & !raw.endpoint_mask != 0
    {
        return Err(MojoError::InvalidOutput);
    }
    let mut capability_statuses = [5_i64; PROVIDER_REGISTRY_ENDPOINT_COUNT];
    for (endpoint, status) in capability_statuses.iter_mut().enumerate() {
        *status = ((raw.capability_bits >> (endpoint * 3)) & 7) as i64;
        if !(0..=6).contains(status) {
            return Err(MojoError::InvalidOutput);
        }
        let supported = raw.endpoint_mask & (1_u64 << endpoint) != 0;
        if supported == matches!(*status, 5 | 6) {
            return Err(MojoError::InvalidOutput);
        }
        if *status == 2 && raw.passthrough_mask & (1_u64 << endpoint) == 0 {
            return Err(MojoError::InvalidOutput);
        }
    }
    Ok(ProviderRegistryPlan {
        provider,
        client_wire: raw.client_wire,
        upstream_wire: raw.upstream_wire,
        response_wire: raw.response_wire,
        supports_streaming: raw.supports_streaming == 1,
        supports_model_fallback: raw.supports_model_fallback == 1,
        endpoint_mask: raw.endpoint_mask,
        passthrough_mask: raw.passthrough_mask,
        capability_statuses,
    })
}

fn resolve(value: &str, include_model_provider_ids: bool) -> Result<Option<i64>, MojoError> {
    if value.len() > 65_536 {
        return Err(MojoError::InvalidInput);
    }
    let mut provider = -1_i64;
    let status = unsafe {
        prodex_mojo_provider_registry_resolve_v1(
            PROVIDER_REGISTRY_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(include_model_provider_ids),
            (&mut provider as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    match provider {
        -1 => Ok(None),
        value if (0..PROVIDER_REGISTRY_PROVIDER_COUNT as i64).contains(&value) => Ok(Some(value)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn resolve_alias(value: &str) -> Result<Option<i64>, MojoError> {
    resolve(value, false)
}

pub fn resolve_model_provider_id(value: &str) -> Result<Option<i64>, MojoError> {
    resolve(value, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_cover_every_provider_and_alias_resolution_is_stable() {
        for provider in 0..PROVIDER_REGISTRY_PROVIDER_COUNT as i64 {
            let plan = provider_plan(provider).unwrap();
            assert_eq!(plan.provider, provider);
            assert!(plan.endpoint_mask != 0);
            assert!(plan.supports_streaming);
        }
        assert_eq!(resolve_alias("  OPENAI-compatible ").unwrap(), Some(0));
        assert_eq!(resolve_alias("claude").unwrap(), Some(1));
        assert_eq!(resolve_alias("google").unwrap(), Some(4));
        assert_eq!(resolve_alias("unknown").unwrap(), None);
        assert_eq!(resolve_model_provider_id("prodex-gemini").unwrap(), Some(4));
        assert_eq!(resolve_alias("prodex-gemini").unwrap(), None);
    }
}
