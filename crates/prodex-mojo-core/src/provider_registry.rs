use crate::MojoError;

pub const PROVIDER_REGISTRY_ABI_VERSION: i64 = 1;
pub const PROVIDER_REGISTRY_PROVIDER_COUNT: usize = 7;
pub const PROVIDER_REGISTRY_ENDPOINT_COUNT: usize = 11;
pub const PROVIDER_RUNTIME_METADATA_ABI_VERSION: i64 = 1;
const PROVIDER_RUNTIME_METADATA_FIELD_COUNT: usize = 6;
const PROVIDER_RUNTIME_METADATA_LABEL_MAX_BYTES: usize = 128;

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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ProviderRuntimeMetadataPlanRaw {
    abi_version: i64,
    provider: i64,
    default_context_window: i64,
    default_auto_compact_token_limit: i64,
    image_generation_enabled: i64,
}

const _: () = assert!(std::mem::size_of::<ProviderRuntimeMetadataPlanRaw>() == 40);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRuntimeMetadataPlan {
    pub provider: i64,
    pub model_provider_id: String,
    pub display_name: String,
    pub codex_provider_name: String,
    pub default_model: String,
    pub default_base_url: Option<String>,
    pub default_context_window: usize,
    pub default_auto_compact_token_limit: usize,
    pub web_search_mode: String,
    pub image_generation_enabled: bool,
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
    fn prodex_mojo_provider_runtime_metadata_plan_v1(
        abi_version: i64,
        provider: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_provider_runtime_metadata_label_v1(
        abi_version: i64,
        provider: i64,
        field: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
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

fn runtime_metadata_status_error(status: i64) -> MojoError {
    match status {
        1 => MojoError::InvalidInput,
        2 => MojoError::Capacity,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

fn runtime_metadata_plan(provider: i64) -> Result<ProviderRuntimeMetadataPlanRaw, MojoError> {
    if !(1..PROVIDER_REGISTRY_PROVIDER_COUNT as i64).contains(&provider) {
        return Err(MojoError::InvalidInput);
    }
    let mut raw = ProviderRuntimeMetadataPlanRaw::default();
    let status = unsafe {
        prodex_mojo_provider_runtime_metadata_plan_v1(
            PROVIDER_RUNTIME_METADATA_ABI_VERSION,
            provider,
            (&mut raw as *mut ProviderRuntimeMetadataPlanRaw) as u64,
        )
    };
    if status != 0 {
        return Err(runtime_metadata_status_error(status));
    }
    if raw.abi_version != PROVIDER_RUNTIME_METADATA_ABI_VERSION
        || raw.provider != provider
        || raw.default_context_window <= 0
        || raw.default_auto_compact_token_limit <= 0
        || raw.default_auto_compact_token_limit >= raw.default_context_window
        || !matches!(raw.image_generation_enabled, 0 | 1)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(raw)
}

fn load_runtime_metadata_label(provider: i64, field: i64) -> Result<String, MojoError> {
    let mut output = [0_u8; PROVIDER_RUNTIME_METADATA_LABEL_MAX_BYTES];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_provider_runtime_metadata_label_v1(
            PROVIDER_RUNTIME_METADATA_ABI_VERSION,
            provider,
            field,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(runtime_metadata_status_error(status));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn runtime_metadata(provider: i64) -> Result<ProviderRuntimeMetadataPlan, MojoError> {
    let raw = runtime_metadata_plan(provider)?;
    let labels = (0..PROVIDER_RUNTIME_METADATA_FIELD_COUNT as i64)
        .map(|field| load_runtime_metadata_label(provider, field))
        .collect::<Result<Vec<_>, _>>()?;
    let mut labels = labels.into_iter();
    let model_provider_id = labels.next().ok_or(MojoError::InvalidOutput)?;
    let display_name = labels.next().ok_or(MojoError::InvalidOutput)?;
    let codex_provider_name = labels.next().ok_or(MojoError::InvalidOutput)?;
    let default_model = labels.next().ok_or(MojoError::InvalidOutput)?;
    let base_url = labels.next().ok_or(MojoError::InvalidOutput)?;
    let web_search_mode = labels.next().ok_or(MojoError::InvalidOutput)?;
    if model_provider_id.is_empty()
        || display_name.is_empty()
        || codex_provider_name.is_empty()
        || default_model.is_empty()
        || web_search_mode.is_empty()
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ProviderRuntimeMetadataPlan {
        provider: raw.provider,
        model_provider_id,
        display_name,
        codex_provider_name,
        default_model,
        default_base_url: (!base_url.is_empty()).then_some(base_url),
        default_context_window: usize::try_from(raw.default_context_window)
            .map_err(|_| MojoError::InvalidOutput)?,
        default_auto_compact_token_limit: usize::try_from(raw.default_auto_compact_token_limit)
            .map_err(|_| MojoError::InvalidOutput)?,
        web_search_mode,
        image_generation_enabled: raw.image_generation_enabled == 1,
    })
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

    #[test]
    fn runtime_metadata_abi_preserves_every_provider_record() {
        let expected = [
            (
                1,
                "prodex-anthropic",
                "Anthropic Claude",
                "Anthropic Claude",
                "claude-sonnet-5-5",
                Some("https://api.anthropic.com/v1"),
                1_000_000,
                950_000,
                "live",
                false,
            ),
            (
                2,
                "prodex-copilot",
                "GitHub Copilot",
                "OpenAI",
                "gpt-6-astra",
                Some("https://api.githubcopilot.com"),
                1_050_000,
                997_500,
                "live",
                false,
            ),
            (
                3,
                "prodex-deepseek",
                "DeepSeek",
                "DeepSeek",
                "deepseek-v4-pro",
                Some("https://api.deepseek.com"),
                1_048_576,
                900_000,
                "live",
                false,
            ),
            (
                4,
                "prodex-gemini",
                "Google Gemini",
                "Azure",
                "auto",
                Some("https://generativelanguage.googleapis.com/v1beta"),
                1_048_576,
                900_000,
                "live",
                true,
            ),
            (
                5,
                "prodex-kiro",
                "Kiro",
                "Azure",
                "auto",
                Some("https://kiro.dev"),
                1_000_000,
                950_000,
                "live",
                false,
            ),
            (
                6,
                "prodex-local",
                "Prodex Local",
                "Prodex Local",
                "unsloth/qwen3.5-35b-a3b",
                None,
                16_384,
                14_000,
                "disabled",
                false,
            ),
        ];
        for (
            provider,
            model_provider_id,
            display_name,
            codex_provider_name,
            default_model,
            default_base_url,
            default_context_window,
            default_auto_compact_token_limit,
            web_search_mode,
            image_generation_enabled,
        ) in expected
        {
            assert_eq!(
                runtime_metadata(provider).unwrap(),
                ProviderRuntimeMetadataPlan {
                    provider,
                    model_provider_id: model_provider_id.to_owned(),
                    display_name: display_name.to_owned(),
                    codex_provider_name: codex_provider_name.to_owned(),
                    default_model: default_model.to_owned(),
                    default_base_url: default_base_url.map(str::to_owned),
                    default_context_window,
                    default_auto_compact_token_limit,
                    web_search_mode: web_search_mode.to_owned(),
                    image_generation_enabled,
                }
            );
        }
        assert_eq!(runtime_metadata(0), Err(MojoError::InvalidInput));
        assert_eq!(runtime_metadata(7), Err(MojoError::InvalidInput));
    }
}
