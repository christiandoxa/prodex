use crate::MojoError;

const ABI_VERSION: i64 = 1;
const CONFIG_RECORDS: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderAliasPlan {
    Anthropic,
    Copilot,
    DeepSeek,
    Gemini,
    Kiro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeExternalProviderClass {
    Anthropic,
    Copilot,
    DeepSeek,
    Gemini,
    GeminiOauth,
    Kiro,
    GeminiNative,
    Antigravity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileImportSourceClass {
    Claude,
    Copilot,
    Kiro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeModelProviderClass {
    Local,
    DeepSeek,
    Gemini,
    Anthropic,
    Copilot,
    Kiro,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalCatalogProviderClass {
    Anthropic = 0,
    Copilot = 1,
    Kiro = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCatalogStaticModel {
    pub slug: String,
    pub display_name: String,
    pub description: String,
}

/// Normalized numeric limits for an external provider model catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalProviderNumericConfig {
    pub context_window: u64,
    pub auto_compact_token_limit: u64,
}

/// Numeric setting rejected by external provider configuration normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderNumericField {
    ContextWindow,
    AutoCompactTokenLimit,
}

/// Reason an external provider numeric setting failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderNumericIssue {
    Empty,
    Whitespace,
    InvalidUnsignedInteger,
    MustBeGreaterThanOne,
}

/// Boundary or field error returned by external provider normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderConfigError {
    Mojo(MojoError),
    InvalidValue {
        field: ExternalProviderNumericField,
        issue: ExternalProviderNumericIssue,
    },
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOpenAiScalarPolicy {
    ProviderName = 0,
    LargeContextModel = 1,
    PreferMaxContextModel = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDeepSeekWebSearchToken {
    Auto,
    Off,
    OpenAiChat,
    Anthropic,
}

unsafe extern "C" {
    fn prodex_super_external_provider_alias_v1(abi_version: i64, address: u64, length: i64) -> i64;

    fn prodex_runtime_external_provider_class_v1(
        abi_version: i64,
        address: u64,
        length: i64,
    ) -> i64;

    fn prodex_profile_import_source_class_v1(abi_version: i64, address: u64, length: i64) -> i64;

    fn prodex_runtime_openai_scalar_policy_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
    ) -> i64;

    fn prodex_runtime_model_provider_class_v1(abi_version: i64, address: u64, length: i64) -> i64;

    fn prodex_external_catalog_model_count_v1(abi_version: i64, provider: i64) -> i64;

    fn prodex_external_catalog_model_at_v1(
        abi_version: i64,
        provider: i64,
        index: i64,
        output_address: u64,
        output_capacity: i64,
        records_address: u64,
        record_count: i64,
        written_address: u64,
    ) -> i64;

    fn prodex_external_catalog_model_find_exact_v1(
        abi_version: i64,
        provider: i64,
        address: u64,
        length: i64,
    ) -> i64;

    fn prodex_deepseek_catalog_model_count_v1(abi_version: i64) -> i64;

    fn prodex_deepseek_catalog_model_at_v1(
        abi_version: i64,
        index: i64,
        output_address: u64,
        output_capacity: i64,
        records_address: u64,
        record_count: i64,
        written_address: u64,
    ) -> i64;

    fn prodex_deepseek_catalog_model_find_v1(abi_version: i64, address: u64, length: i64) -> i64;

    fn prodex_runtime_bool_token_v1(abi_version: i64, address: u64, length: i64) -> i64;

    fn prodex_runtime_ci_truth_token_v1(abi_version: i64, address: u64, length: i64) -> i64;

    fn prodex_runtime_deepseek_web_search_token_v1(
        abi_version: i64,
        address: u64,
        length: i64,
    ) -> i64;

    fn prodex_super_toml_string_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;

    fn prodex_super_provider_config_v1(
        abi_version: i64,
        provider_id_address: u64,
        provider_id_length: i64,
        provider_name_address: u64,
        provider_name_length: i64,
        base_url_address: u64,
        base_url_length: i64,
        model_address: u64,
        model_length: i64,
        web_search_address: u64,
        web_search_length: i64,
        context_window: u64,
        auto_compact_token_limit: u64,
        image_generation: i64,
        output_address: u64,
        output_capacity: i64,
        records_address: u64,
        record_count: i64,
        written_address: u64,
    ) -> i64;

    fn prodex_external_provider_numeric_config_v1(
        abi_version: i64,
        context_address: u64,
        context_length: i64,
        context_present: i64,
        default_context_window: u64,
        compact_address: u64,
        compact_length: i64,
        compact_present: i64,
        default_auto_compact_token_limit: u64,
        output_address: u64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn external_provider_alias(
    value: &str,
) -> Result<Option<ExternalProviderAliasPlan>, MojoError> {
    let result = unsafe {
        prodex_super_external_provider_alias_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(ExternalProviderAliasPlan::Anthropic)),
        1 => Ok(Some(ExternalProviderAliasPlan::Copilot)),
        2 => Ok(Some(ExternalProviderAliasPlan::DeepSeek)),
        3 => Ok(Some(ExternalProviderAliasPlan::Gemini)),
        4 => Ok(Some(ExternalProviderAliasPlan::Kiro)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_external_provider_class(
    value: &str,
) -> Result<Option<RuntimeExternalProviderClass>, MojoError> {
    let result = unsafe {
        prodex_runtime_external_provider_class_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(RuntimeExternalProviderClass::Anthropic)),
        1 => Ok(Some(RuntimeExternalProviderClass::Copilot)),
        2 => Ok(Some(RuntimeExternalProviderClass::DeepSeek)),
        3 => Ok(Some(RuntimeExternalProviderClass::Gemini)),
        4 => Ok(Some(RuntimeExternalProviderClass::GeminiOauth)),
        5 => Ok(Some(RuntimeExternalProviderClass::Kiro)),
        6 => Ok(Some(RuntimeExternalProviderClass::GeminiNative)),
        7 => Ok(Some(RuntimeExternalProviderClass::Antigravity)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn profile_import_source_class(
    value: &str,
) -> Result<Option<ProfileImportSourceClass>, MojoError> {
    let result = unsafe {
        prodex_profile_import_source_class_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(ProfileImportSourceClass::Claude)),
        1 => Ok(Some(ProfileImportSourceClass::Copilot)),
        2 => Ok(Some(ProfileImportSourceClass::Kiro)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_openai_scalar_policy(
    operation: RuntimeOpenAiScalarPolicy,
    value: &str,
) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_runtime_openai_scalar_policy_v1(
            ABI_VERSION,
            operation as i64,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn external_catalog_record(output: &[u8], record: &[i64]) -> Result<String, MojoError> {
    let [start, length] = <[i64; 2]>::try_from(record).map_err(|_| MojoError::InvalidOutput)?;
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    let end = start.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    let bytes = output.get(start..end).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn external_catalog_static_model_count(
    provider: ExternalCatalogProviderClass,
) -> Result<usize, MojoError> {
    let count = unsafe { prodex_external_catalog_model_count_v1(ABI_VERSION, provider as i64) };
    usize::try_from(count).map_err(|_| MojoError::InvalidOutput)
}

pub fn external_catalog_static_model(
    provider: ExternalCatalogProviderClass,
    index: usize,
) -> Result<ExternalCatalogStaticModel, MojoError> {
    const RECORDS: usize = 3;
    const OUTPUT_CAPACITY: usize = 1024;
    let mut output = vec![0_u8; OUTPUT_CAPACITY];
    let mut records = [-1_i64; RECORDS * 2];
    let mut written = -1_i64;
    status(unsafe {
        prodex_external_catalog_model_at_v1(
            ABI_VERSION,
            provider as i64,
            i64::try_from(index).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            records.as_mut_ptr() as usize as u64,
            i64::try_from(RECORDS).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(ExternalCatalogStaticModel {
        slug: external_catalog_record(&output, &records[0..2])?,
        display_name: external_catalog_record(&output, &records[2..4])?,
        description: external_catalog_record(&output, &records[4..6])?,
    })
}

pub fn external_catalog_static_models(
    provider: ExternalCatalogProviderClass,
) -> Result<Vec<ExternalCatalogStaticModel>, MojoError> {
    let count = external_catalog_static_model_count(provider)?;
    (0..count)
        .map(|index| external_catalog_static_model(provider, index))
        .collect()
}

pub fn external_catalog_model_find_exact(
    provider: ExternalCatalogProviderClass,
    model: &str,
) -> Result<Option<usize>, MojoError> {
    let index = unsafe {
        prodex_external_catalog_model_find_exact_v1(
            ABI_VERSION,
            provider as i64,
            model.as_ptr() as usize as u64,
            signed_len(model)?,
        )
    };
    match index {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        value if value >= 0 => Ok(Some(
            usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?,
        )),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn external_catalog_model_metadata(
    provider: ExternalCatalogProviderClass,
    model: &str,
) -> Result<Option<ExternalCatalogStaticModel>, MojoError> {
    external_catalog_model_find_exact(provider, model)?
        .map(|index| external_catalog_static_model(provider, index))
        .transpose()
}

pub fn deepseek_catalog_static_model_count() -> Result<usize, MojoError> {
    let count = unsafe { prodex_deepseek_catalog_model_count_v1(ABI_VERSION) };
    usize::try_from(count).map_err(|_| MojoError::InvalidOutput)
}

pub fn deepseek_catalog_static_model(
    index: usize,
) -> Result<ExternalCatalogStaticModel, MojoError> {
    const RECORDS: usize = 3;
    const OUTPUT_CAPACITY: usize = 1024;
    let mut output = vec![0_u8; OUTPUT_CAPACITY];
    let mut records = [-1_i64; RECORDS * 2];
    let mut written = -1_i64;
    status(unsafe {
        prodex_deepseek_catalog_model_at_v1(
            ABI_VERSION,
            i64::try_from(index).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            records.as_mut_ptr() as usize as u64,
            i64::try_from(RECORDS).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(ExternalCatalogStaticModel {
        slug: external_catalog_record(&output, &records[0..2])?,
        display_name: external_catalog_record(&output, &records[2..4])?,
        description: external_catalog_record(&output, &records[4..6])?,
    })
}

pub fn deepseek_catalog_static_models() -> Result<Vec<ExternalCatalogStaticModel>, MojoError> {
    let count = deepseek_catalog_static_model_count()?;
    (0..count).map(deepseek_catalog_static_model).collect()
}

pub fn deepseek_catalog_model_metadata(
    model: &str,
) -> Result<Option<ExternalCatalogStaticModel>, MojoError> {
    let index = unsafe {
        prodex_deepseek_catalog_model_find_v1(
            ABI_VERSION,
            model.as_ptr() as usize as u64,
            signed_len(model)?,
        )
    };
    match index {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        value if value >= 0 => deepseek_catalog_static_model(
            usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?,
        )
        .map(Some),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_model_provider_class(
    value: &str,
) -> Result<Option<RuntimeModelProviderClass>, MojoError> {
    let result = unsafe {
        prodex_runtime_model_provider_class_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(RuntimeModelProviderClass::Local)),
        1 => Ok(Some(RuntimeModelProviderClass::DeepSeek)),
        2 => Ok(Some(RuntimeModelProviderClass::Gemini)),
        3 => Ok(Some(RuntimeModelProviderClass::Anthropic)),
        4 => Ok(Some(RuntimeModelProviderClass::Copilot)),
        5 => Ok(Some(RuntimeModelProviderClass::Kiro)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_bool_token(value: &str) -> Result<Option<bool>, MojoError> {
    let result = unsafe {
        prodex_runtime_bool_token_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(false)),
        1 => Ok(Some(true)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_ci_truth_token(value: &str) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_runtime_ci_truth_token_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_deepseek_web_search_token(
    value: &str,
) -> Result<Option<RuntimeDeepSeekWebSearchToken>, MojoError> {
    let result = unsafe {
        prodex_runtime_deepseek_web_search_token_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(RuntimeDeepSeekWebSearchToken::Auto)),
        1 => Ok(Some(RuntimeDeepSeekWebSearchToken::Off)),
        2 => Ok(Some(RuntimeDeepSeekWebSearchToken::OpenAiChat)),
        3 => Ok(Some(RuntimeDeepSeekWebSearchToken::Anthropic)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn toml_string_literal(value: &str) -> Result<String, MojoError> {
    let capacity = value
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_add(2))
        .ok_or(MojoError::InvalidInput)?
        .max(2);
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    status(unsafe {
        prodex_super_toml_string_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

pub struct ProviderConfigInput<'a> {
    pub provider_id: &'a str,
    pub provider_name: &'a str,
    pub base_url: &'a str,
    pub model: &'a str,
    pub web_search: &'a str,
    pub context_window: usize,
    pub auto_compact_token_limit: usize,
    pub image_generation: bool,
}

fn external_provider_optional_text_parts(
    value: Option<&str>,
) -> Result<(u64, i64, i64), MojoError> {
    let Some(value) = value else {
        return Ok((0, 0, 0));
    };
    Ok((value.as_ptr() as usize as u64, signed_len(value)?, 1))
}

/// Validates configured limits and clamps compacting below the context window.
pub fn external_provider_numeric_config(
    context_window: Option<&str>,
    default_context_window: u64,
    auto_compact_token_limit: Option<&str>,
    default_auto_compact_token_limit: u64,
) -> Result<ExternalProviderNumericConfig, ExternalProviderConfigError> {
    let (context_address, context_length, context_present) =
        external_provider_optional_text_parts(context_window)
            .map_err(ExternalProviderConfigError::Mojo)?;
    let (compact_address, compact_length, compact_present) =
        external_provider_optional_text_parts(auto_compact_token_limit)
            .map_err(ExternalProviderConfigError::Mojo)?;
    let mut output = [0_u64; 2];
    let status = unsafe {
        prodex_external_provider_numeric_config_v1(
            ABI_VERSION,
            context_address,
            context_length,
            context_present,
            default_context_window,
            compact_address,
            compact_length,
            compact_present,
            default_auto_compact_token_limit,
            output.as_mut_ptr() as usize as u64,
        )
    };
    let issue = |field, issue| ExternalProviderConfigError::InvalidValue { field, issue };
    match status {
        0 => Ok(ExternalProviderNumericConfig {
            context_window: output[0],
            auto_compact_token_limit: output[1],
        }),
        11 => Err(issue(
            ExternalProviderNumericField::ContextWindow,
            ExternalProviderNumericIssue::Empty,
        )),
        12 => Err(issue(
            ExternalProviderNumericField::ContextWindow,
            ExternalProviderNumericIssue::Whitespace,
        )),
        13 => Err(issue(
            ExternalProviderNumericField::ContextWindow,
            ExternalProviderNumericIssue::InvalidUnsignedInteger,
        )),
        14 => Err(issue(
            ExternalProviderNumericField::ContextWindow,
            ExternalProviderNumericIssue::MustBeGreaterThanOne,
        )),
        21 => Err(issue(
            ExternalProviderNumericField::AutoCompactTokenLimit,
            ExternalProviderNumericIssue::Empty,
        )),
        22 => Err(issue(
            ExternalProviderNumericField::AutoCompactTokenLimit,
            ExternalProviderNumericIssue::Whitespace,
        )),
        23 => Err(issue(
            ExternalProviderNumericField::AutoCompactTokenLimit,
            ExternalProviderNumericIssue::InvalidUnsignedInteger,
        )),
        24 => Err(issue(
            ExternalProviderNumericField::AutoCompactTokenLimit,
            ExternalProviderNumericIssue::MustBeGreaterThanOne,
        )),
        1 => Err(ExternalProviderConfigError::Mojo(MojoError::InvalidInput)),
        4 => Err(ExternalProviderConfigError::Mojo(MojoError::AbiMismatch)),
        _ => Err(ExternalProviderConfigError::Mojo(MojoError::InvalidOutput)),
    }
}

pub fn provider_config_entries(input: ProviderConfigInput<'_>) -> Result<Vec<String>, MojoError> {
    let capacity = input
        .provider_id
        .len()
        .checked_mul(7)
        .and_then(|value| {
            input
                .provider_name
                .len()
                .checked_add(input.base_url.len())
                .and_then(|sum| sum.checked_add(input.model.len()))
                .and_then(|sum| sum.checked_add(input.web_search.len()))
                .and_then(|sum| sum.checked_mul(2))
                .and_then(|dynamic| value.checked_add(dynamic))
        })
        .and_then(|value| value.checked_add(2048))
        .ok_or(MojoError::InvalidInput)?
        .max(2048);

    let mut output = vec![0_u8; capacity];
    let mut records = [-1_i64; CONFIG_RECORDS * 2];
    let mut written = 0_i64;
    status(unsafe {
        prodex_super_provider_config_v1(
            ABI_VERSION,
            input.provider_id.as_ptr() as usize as u64,
            signed_len(input.provider_id)?,
            input.provider_name.as_ptr() as usize as u64,
            signed_len(input.provider_name)?,
            input.base_url.as_ptr() as usize as u64,
            signed_len(input.base_url)?,
            input.model.as_ptr() as usize as u64,
            signed_len(input.model)?,
            input.web_search.as_ptr() as usize as u64,
            signed_len(input.web_search)?,
            u64::try_from(input.context_window).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(input.auto_compact_token_limit).map_err(|_| MojoError::InvalidInput)?,
            i64::from(input.image_generation),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            records.as_mut_ptr() as usize as u64,
            CONFIG_RECORDS as i64,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;

    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    records
        .as_chunks::<2>()
        .0
        .iter()
        .map(|record| {
            let offset = usize::try_from(record[0]).map_err(|_| MojoError::InvalidOutput)?;
            let length = usize::try_from(record[1]).map_err(|_| MojoError::InvalidOutput)?;
            let end = offset
                .checked_add(length)
                .filter(|end| *end <= written)
                .ok_or(MojoError::InvalidOutput)?;
            String::from_utf8(output[offset..end].to_vec()).map_err(|_| MojoError::InvalidOutput)
        })
        .collect()
}

#[cfg(test)]
#[path = "super_provider_config/tests.rs"]
mod tests;
