use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct ArgumentView {
    address: u64,
    length: u64,
    valid_utf8: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigOverrideSelection {
    pub raw_value: String,
    pub normalized_value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelProviderSource {
    CliOverride,
    ProfileV2ConfigFile,
    ConfigFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProviderPlan {
    ReadProfileV2Config,
    ReadConfigFile,
    Selected {
        source: ModelProviderSource,
        provider_id: String,
    },
    NoProvider,
}

unsafe extern "C" {
    fn prodex_codex_config_normalize_value_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_codex_config_profile_name_valid_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
    ) -> i64;

    fn prodex_codex_config_profile_v2_v1(
        abi_version: i64,
        arguments_address: u64,
        count: i64,
        result_address: u64,
    ) -> i64;

    fn prodex_codex_config_override_v1(
        abi_version: i64,
        arguments_address: u64,
        count: i64,
        key_address: u64,
        key_length: i64,
        result_address: u64,
    ) -> i64;

    fn prodex_codex_config_model_provider_plan_v1(
        abi_version: i64,
        arguments_address: u64,
        count: i64,
        profile_config_path_present: i64,
        profile_config_loaded: i64,
        config_file_loaded: i64,
        result_address: u64,
    ) -> i64;
}

fn views(arguments: &[Option<&str>]) -> Result<Vec<ArgumentView>, MojoError> {
    arguments
        .iter()
        .map(|argument| match argument {
            Some(value) => Ok(ArgumentView {
                address: value.as_ptr() as usize as u64,
                length: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
                valid_utf8: 1,
            }),
            None => Ok(ArgumentView {
                address: 0,
                length: 0,
                valid_utf8: 0,
            }),
        })
        .collect()
}

fn validate_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn slice(
    arguments: &[Option<&str>],
    index: i64,
    offset: i64,
    length: i64,
) -> Result<String, MojoError> {
    let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
    let value = arguments
        .get(index)
        .and_then(|value| *value)
        .ok_or(MojoError::InvalidOutput)?;
    let offset = usize::try_from(offset).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    let end = offset.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    value
        .get(offset..end)
        .map(str::to_string)
        .ok_or(MojoError::InvalidOutput)
}

pub fn normalize_value(value: &str) -> Result<Option<String>, MojoError> {
    let mut result = [0_i64, -1_i64];
    let status = unsafe {
        prodex_codex_config_normalize_value_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    };
    validate_status(status)?;
    if result[1] < 0 {
        return Ok(None);
    }
    let offset = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    let end = offset.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    value
        .get(offset..end)
        .map(|value| Some(value.to_string()))
        .ok_or(MojoError::InvalidOutput)
}

pub fn profile_name_valid(name: &str) -> Result<bool, MojoError> {
    let status = unsafe {
        prodex_codex_config_profile_name_valid_v1(
            ABI_VERSION,
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn profile_v2_name(arguments: &[Option<&str>]) -> Result<Option<String>, MojoError> {
    let views = views(arguments)?;
    let mut result = [-1_i64, 0_i64, 0_i64];
    let status = unsafe {
        prodex_codex_config_profile_v2_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    };
    validate_status(status)?;
    if result[0] < 0 {
        return Ok(None);
    }
    slice(arguments, result[0], result[1], result[2]).map(Some)
}

pub fn config_override(
    arguments: &[Option<&str>],
    key: &str,
) -> Result<Option<ConfigOverrideSelection>, MojoError> {
    let views = views(arguments)?;
    let mut result = [-1_i64; 5];
    let status = unsafe {
        prodex_codex_config_override_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            key.as_ptr() as usize as u64,
            i64::try_from(key.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    };
    validate_status(status)?;
    if result[0] < 0 {
        return Ok(None);
    }
    let raw_value = slice(arguments, result[0], result[1], result[2])?;
    let normalized_value = if result[4] < 0 {
        None
    } else {
        Some(slice(arguments, result[0], result[3], result[4])?)
    };
    Ok(Some(ConfigOverrideSelection {
        raw_value,
        normalized_value,
    }))
}

pub fn model_provider_plan(
    model_provider_override: Option<&str>,
    profile_provider: Option<&str>,
    config_provider: Option<&str>,
    profile_config_path_present: bool,
    profile_config_loaded: bool,
    config_file_loaded: bool,
) -> Result<ModelProviderPlan, MojoError> {
    let arguments = [model_provider_override, profile_provider, config_provider];
    let views = views(&arguments)?;
    let mut result = [-1_i64, 0_i64, 0_i64];
    let status = unsafe {
        prodex_codex_config_model_provider_plan_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(u8::from(profile_config_path_present)),
            i64::from(u8::from(profile_config_loaded)),
            i64::from(u8::from(config_file_loaded)),
            result.as_mut_ptr() as usize as u64,
        )
    };
    validate_status(status)?;
    match result[0] {
        0 => Ok(ModelProviderPlan::ReadProfileV2Config),
        1 => Ok(ModelProviderPlan::ReadConfigFile),
        2..=4 => {
            let (source, argument_index) = match result[0] {
                2 => (ModelProviderSource::CliOverride, 0),
                3 => (ModelProviderSource::ProfileV2ConfigFile, 1),
                4 => (ModelProviderSource::ConfigFile, 2),
                _ => return Err(MojoError::InvalidOutput),
            };
            let provider_id = slice(&arguments, argument_index, result[1], result[2])?;
            if provider_id.is_empty() {
                return Err(MojoError::InvalidOutput);
            }
            Ok(ModelProviderPlan::Selected {
                source,
                provider_id,
            })
        }
        5 => Ok(ModelProviderPlan::NoProvider),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_config_kernel_smoke() {
        assert!(profile_name_valid("team_2").unwrap());
        assert!(!profile_name_valid("../evil").unwrap());
        assert_eq!(
            normalize_value("  ' cli-provider ' ").unwrap().as_deref(),
            Some("cli-provider")
        );
        assert_eq!(normalize_value("  '  '  ").unwrap(), None);
        assert_eq!(
            profile_v2_name(&[Some("exec"), Some("--profile"), Some("local_1-prod"),])
                .unwrap()
                .as_deref(),
            Some("local_1-prod")
        );
        assert_eq!(
            config_override(
                &[
                    Some("-c"),
                    Some("model='first'"),
                    Some("--config=model='last'"),
                    Some("--"),
                    Some("--config=model='ignored'"),
                ],
                "model",
            )
            .unwrap()
            .unwrap(),
            ConfigOverrideSelection {
                raw_value: "'last'".to_string(),
                normalized_value: Some("last".to_string()),
            }
        );
    }

    #[test]
    fn codex_model_provider_plan_kernel_smoke() {
        assert_eq!(
            model_provider_plan(Some("cli-provider"), None, None, true, false, false).unwrap(),
            ModelProviderPlan::ReadProfileV2Config
        );
        assert_eq!(
            model_provider_plan(
                Some("cli-provider"),
                Some("profile-provider"),
                None,
                true,
                true,
                false,
            )
            .unwrap(),
            ModelProviderPlan::Selected {
                source: ModelProviderSource::CliOverride,
                provider_id: "cli-provider".to_string(),
            }
        );
        assert_eq!(
            model_provider_plan(None, Some("profile-provider"), None, true, true, false).unwrap(),
            ModelProviderPlan::Selected {
                source: ModelProviderSource::ProfileV2ConfigFile,
                provider_id: "profile-provider".to_string(),
            }
        );
        assert_eq!(
            model_provider_plan(None, None, None, true, true, false).unwrap(),
            ModelProviderPlan::ReadConfigFile
        );
        assert_eq!(
            model_provider_plan(None, None, Some("config-provider"), true, true, true).unwrap(),
            ModelProviderPlan::Selected {
                source: ModelProviderSource::ConfigFile,
                provider_id: "config-provider".to_string(),
            }
        );
        assert_eq!(
            model_provider_plan(None, None, Some("config-provider"), false, false, true).unwrap(),
            ModelProviderPlan::Selected {
                source: ModelProviderSource::ConfigFile,
                provider_id: "config-provider".to_string(),
            }
        );
        assert_eq!(
            model_provider_plan(None, None, None, false, false, true).unwrap(),
            ModelProviderPlan::NoProvider
        );
    }
}
