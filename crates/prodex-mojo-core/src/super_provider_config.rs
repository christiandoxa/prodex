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
pub enum RuntimeModelProviderClass {
    Local,
    DeepSeek,
    Gemini,
    Anthropic,
    Copilot,
    Kiro,
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

    fn prodex_runtime_model_provider_class_v1(abi_version: i64, address: u64, length: i64) -> i64;

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
mod tests {
    use super::*;

    #[test]
    fn super_provider_config_kernel_preserves_aliases_escaping_and_order() {
        assert_eq!(
            external_provider_alias(" GitHub_Copilot ").unwrap(),
            Some(ExternalProviderAliasPlan::Copilot)
        );
        assert_eq!(external_provider_alias("unknown").unwrap(), None);
        assert_eq!(
            runtime_external_provider_class("GITHUB_COPILOT").unwrap(),
            Some(RuntimeExternalProviderClass::Copilot)
        );
        assert_eq!(
            runtime_external_provider_class("gemini-oauth").unwrap(),
            Some(RuntimeExternalProviderClass::GeminiOauth)
        );
        assert_eq!(
            runtime_external_provider_class("gemini-native").unwrap(),
            Some(RuntimeExternalProviderClass::GeminiNative)
        );
        assert_eq!(runtime_external_provider_class(" gemini ").unwrap(), None);
        assert_eq!(runtime_external_provider_class("unknown").unwrap(), None);
        assert_eq!(
            runtime_model_provider_class("PRODEX-GEMINI").unwrap(),
            Some(RuntimeModelProviderClass::Gemini)
        );
        assert_eq!(
            runtime_model_provider_class("prodex-copilot").unwrap(),
            Some(RuntimeModelProviderClass::Copilot)
        );
        assert_eq!(
            runtime_model_provider_class(" prodex-local ").unwrap(),
            None
        );
        assert_eq!(runtime_model_provider_class("custom").unwrap(), None);
        assert_eq!(runtime_bool_token("YeS").unwrap(), Some(true));
        assert_eq!(runtime_bool_token("OFF").unwrap(), Some(false));
        assert_eq!(runtime_bool_token(" true ").unwrap(), None);
        assert_eq!(runtime_bool_token("maybe").unwrap(), None);
        assert!(runtime_ci_truth_token("YeS").unwrap());
        assert!(runtime_ci_truth_token("TRUE").unwrap());
        assert!(!runtime_ci_truth_token("on").unwrap());
        assert!(!runtime_ci_truth_token("off").unwrap());
        assert!(!runtime_ci_truth_token("").unwrap());
        assert_eq!(
            runtime_deepseek_web_search_token("OPENAI-CHAT").unwrap(),
            Some(RuntimeDeepSeekWebSearchToken::OpenAiChat)
        );
        assert_eq!(
            runtime_deepseek_web_search_token("DISABLE").unwrap(),
            Some(RuntimeDeepSeekWebSearchToken::Off)
        );
        assert_eq!(
            runtime_deepseek_web_search_token("ANTHROPIC").unwrap(),
            Some(RuntimeDeepSeekWebSearchToken::Anthropic)
        );
        assert_eq!(runtime_deepseek_web_search_token("live").unwrap(), None);
        assert_eq!(toml_string_literal("a\\b\"c").unwrap(), "\"a\\\\b\\\"c\"");

        let entries = provider_config_entries(ProviderConfigInput {
            provider_id: "gemini",
            provider_name: "Gemini \"Bridge\"",
            base_url: "https://example.com/v1",
            model: "模型/β",
            web_search: "live",
            context_window: 1_048_576,
            auto_compact_token_limit: 900_000,
            image_generation: true,
        })
        .unwrap();
        assert_eq!(entries.len(), 14);
        assert_eq!(entries[0], "model_provider=\"gemini\"");
        assert_eq!(entries[1], "model=\"模型/β\"");
        assert_eq!(
            entries[2],
            "model_providers.gemini.name=\"Gemini \\\"Bridge\\\"\""
        );
        assert_eq!(
            entries[3],
            "model_providers.gemini.base_url=\"https://example.com/v1\""
        );
        assert_eq!(entries[4], "model_providers.gemini.wire_api=\"responses\"");
        assert_eq!(
            entries[5],
            "model_providers.gemini.requires_openai_auth=true"
        );
        assert_eq!(
            entries[6],
            "model_providers.gemini.supports_websockets=false"
        );
        assert_eq!(entries[7], "model_context_window=1048576");
        assert_eq!(entries[8], "model_auto_compact_token_limit=900000");
        assert_eq!(entries[9], "model_reasoning_summary=\"none\"");
        assert_eq!(entries[10], "web_search=\"live\"");
        assert_eq!(entries[11], "features.apps=false");
        assert_eq!(entries[12], "features.js_repl=false");
        assert_eq!(entries[13], "features.image_generation=true");
    }
}
