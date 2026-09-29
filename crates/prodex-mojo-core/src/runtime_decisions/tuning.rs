const RUNTIME_SCALAR_CONFIG_ABI_VERSION: i64 = 1;
const RUNTIME_SCALAR_CONFIG_LOG_FORMAT: i64 = 0;
const RUNTIME_SCALAR_CONFIG_PROXY_PRESET: i64 = 1;
const RUNTIME_SCALAR_CONFIG_WEB_SEARCH: i64 = 2;
const RUNTIME_SCALAR_CONFIG_CLOCK_SOURCE: i64 = 3;
const RUNTIME_SCALAR_CONFIG_OPENAI_PROVIDER: i64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLogFormatClass {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProxyPresetClass {
    Low,
    Default,
    ManyTerminals,
    Aggressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebSearchModeClass {
    Disabled,
    Cached,
    Indexed,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeClockSourceClass {
    System,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningDefaults {
    pub worker_count: usize,
    pub long_lived_worker_count: usize,
    pub probe_refresh_worker_count: usize,
    pub async_worker_count: usize,
    pub log_queue_capacity: usize,
    pub websocket_connect_worker_count: usize,
    pub websocket_dns_worker_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningCapacityDefaults {
    pub long_lived_queue_capacity: usize,
    pub active_request_limit: usize,
    pub log_queue_capacity: usize,
    pub websocket_connect_queue_capacity: usize,
    pub websocket_connect_overflow_capacity: usize,
    pub websocket_dns_queue_capacity: usize,
    pub websocket_dns_overflow_capacity: usize,
    pub responses_lane_limit: usize,
    pub compact_lane_limit: usize,
    pub websocket_lane_limit: usize,
    pub standard_lane_limit: usize,
}

/// Values supplied by a selected runtime-proxy policy preset.
///
/// `None` means the preset leaves that field unset so downstream runtime
/// tuning can apply its normal default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeTuningProxyPresetDefaults {
    pub worker_count: Option<usize>,
    pub long_lived_worker_count: Option<usize>,
    pub probe_refresh_worker_count: Option<usize>,
    pub async_worker_count: Option<usize>,
    pub long_lived_queue_capacity: Option<usize>,
    pub active_request_limit: Option<usize>,
    pub profile_inflight_soft_limit: Option<usize>,
    pub profile_inflight_hard_limit: Option<usize>,
    pub responses_active_limit: Option<usize>,
    pub compact_active_limit: Option<usize>,
    pub websocket_active_limit: Option<usize>,
    pub standard_active_limit: Option<usize>,
    pub websocket_connect_worker_count: Option<usize>,
    pub websocket_connect_queue_capacity: Option<usize>,
    pub websocket_connect_overflow_capacity: Option<usize>,
    pub websocket_dns_worker_count: Option<usize>,
    pub websocket_dns_queue_capacity: Option<usize>,
    pub websocket_dns_overflow_capacity: Option<usize>,
    pub startup_sync_probe_warm_limit: Option<usize>,
}

const RUNTIME_PROXY_PRESET_FIELD_COUNT: usize = 19;

unsafe extern "C" {
    fn prodex_runtime_scalar_config_policy_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
    ) -> i64;
    fn prodex_runtime_tuning_defaults(
        parallelism: i64,
        worker_count: *mut i64,
        long_lived_worker_count: *mut i64,
        probe_refresh_worker_count: *mut i64,
        async_worker_count: *mut i64,
        log_queue_capacity: *mut i64,
        websocket_connect_worker_count: *mut i64,
        websocket_dns_worker_count: *mut i64,
    ) -> i64;
    fn prodex_runtime_tuning_capacity_defaults(
        parallelism: i64,
        global_limit: i64,
        worker_count: i64,
        long_lived_worker_count: i64,
        responses_override: i64,
        compact_override: i64,
        websocket_override: i64,
        standard_override: i64,
        websocket_connect_queue_override: i64,
        websocket_dns_queue_override: i64,
        long_lived_queue_capacity: *mut i64,
        active_request_limit: *mut i64,
        log_queue_capacity: *mut i64,
        websocket_connect_queue_capacity: *mut i64,
        websocket_connect_overflow_capacity: *mut i64,
        websocket_dns_queue_capacity: *mut i64,
        websocket_dns_overflow_capacity: *mut i64,
        responses_lane_limit: *mut i64,
        compact_lane_limit: *mut i64,
        websocket_lane_limit: *mut i64,
        standard_lane_limit: *mut i64,
    ) -> i64;
    fn prodex_runtime_proxy_preset_defaults_v1(preset: i64, output: *mut i64) -> i64;
}

fn runtime_scalar_config_policy(operation: i64, value: &str) -> Result<i64, crate::MojoError> {
    let length = i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_runtime_scalar_config_policy_v1(
            RUNTIME_SCALAR_CONFIG_ABI_VERSION,
            operation,
            value.as_ptr() as usize as u64,
            length,
        )
    };
    match result {
        -3 => Err(crate::MojoError::AbiMismatch),
        -2 => Err(crate::MojoError::InvalidInput),
        value => Ok(value),
    }
}

pub fn runtime_log_format_class(
    value: &str,
) -> Result<Option<RuntimeLogFormatClass>, crate::MojoError> {
    Ok(
        match runtime_scalar_config_policy(RUNTIME_SCALAR_CONFIG_LOG_FORMAT, value)? {
            -1 => None,
            0 => Some(RuntimeLogFormatClass::Text),
            1 => Some(RuntimeLogFormatClass::Json),
            _ => return Err(crate::MojoError::InvalidOutput),
        },
    )
}

pub fn runtime_proxy_preset_class(
    value: &str,
) -> Result<Option<RuntimeProxyPresetClass>, crate::MojoError> {
    Ok(
        match runtime_scalar_config_policy(RUNTIME_SCALAR_CONFIG_PROXY_PRESET, value)? {
            -1 => None,
            0 => Some(RuntimeProxyPresetClass::Low),
            1 => Some(RuntimeProxyPresetClass::Default),
            2 => Some(RuntimeProxyPresetClass::ManyTerminals),
            3 => Some(RuntimeProxyPresetClass::Aggressive),
            _ => return Err(crate::MojoError::InvalidOutput),
        },
    )
}

pub fn runtime_web_search_mode_class(
    value: &str,
) -> Result<Option<RuntimeWebSearchModeClass>, crate::MojoError> {
    Ok(
        match runtime_scalar_config_policy(RUNTIME_SCALAR_CONFIG_WEB_SEARCH, value)? {
            -1 => None,
            0 => Some(RuntimeWebSearchModeClass::Disabled),
            1 => Some(RuntimeWebSearchModeClass::Cached),
            2 => Some(RuntimeWebSearchModeClass::Indexed),
            3 => Some(RuntimeWebSearchModeClass::Live),
            _ => return Err(crate::MojoError::InvalidOutput),
        },
    )
}

pub fn runtime_clock_source_class(
    value: &str,
) -> Result<Option<RuntimeClockSourceClass>, crate::MojoError> {
    Ok(
        match runtime_scalar_config_policy(RUNTIME_SCALAR_CONFIG_CLOCK_SOURCE, value)? {
            -1 => None,
            0 => Some(RuntimeClockSourceClass::System),
            1 => Some(RuntimeClockSourceClass::External),
            _ => return Err(crate::MojoError::InvalidOutput),
        },
    )
}

pub fn runtime_model_provider_is_openai(value: &str) -> Result<bool, crate::MojoError> {
    match runtime_scalar_config_policy(RUNTIME_SCALAR_CONFIG_OPENAI_PROVIDER, value)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn runtime_tuning_defaults(
    parallelism: usize,
) -> Result<RuntimeTuningDefaults, crate::MojoError> {
    let parallelism = i64::try_from(parallelism).unwrap_or(i64::MAX);
    let mut values = [0_i64; 7];
    let status = unsafe {
        prodex_runtime_tuning_defaults(
            parallelism,
            &mut values[0],
            &mut values[1],
            &mut values[2],
            &mut values[3],
            &mut values[4],
            &mut values[5],
            &mut values[6],
        )
    };
    if status != 0 || values.iter().any(|value| *value < 0) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(RuntimeTuningDefaults {
        worker_count: usize::try_from(values[0]).map_err(|_| crate::MojoError::InvalidOutput)?,
        long_lived_worker_count: usize::try_from(values[1])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        probe_refresh_worker_count: usize::try_from(values[2])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        async_worker_count: usize::try_from(values[3])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        log_queue_capacity: usize::try_from(values[4])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        websocket_connect_worker_count: usize::try_from(values[5])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        websocket_dns_worker_count: usize::try_from(values[6])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
    })
}

pub fn runtime_tuning_capacity_defaults(
    parallelism: usize,
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: [Option<usize>; 4],
    queue_overrides: [Option<usize>; 2],
) -> Result<RuntimeTuningCapacityDefaults, crate::MojoError> {
    let to_i64 = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
    let values = [
        to_i64(parallelism),
        to_i64(global_limit),
        to_i64(worker_count),
        to_i64(long_lived_worker_count),
        to_i64(overrides[0].unwrap_or_default()),
        to_i64(overrides[1].unwrap_or_default()),
        to_i64(overrides[2].unwrap_or_default()),
        to_i64(overrides[3].unwrap_or_default()),
        to_i64(queue_overrides[0].unwrap_or_default()),
        to_i64(queue_overrides[1].unwrap_or_default()),
    ];
    let mut output = [0_i64; 11];
    let status = unsafe {
        prodex_runtime_tuning_capacity_defaults(
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            values[5],
            values[6],
            values[7],
            values[8],
            values[9],
            &mut output[0],
            &mut output[1],
            &mut output[2],
            &mut output[3],
            &mut output[4],
            &mut output[5],
            &mut output[6],
            &mut output[7],
            &mut output[8],
            &mut output[9],
            &mut output[10],
        )
    };
    if status != 0 || output.iter().any(|value| *value < 1) {
        return Err(crate::MojoError::InvalidOutput);
    }
    let values = output
        .map(|value| usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeTuningCapacityDefaults {
        long_lived_queue_capacity: values[0],
        active_request_limit: values[1],
        log_queue_capacity: values[2],
        websocket_connect_queue_capacity: values[3],
        websocket_connect_overflow_capacity: values[4],
        websocket_dns_queue_capacity: values[5],
        websocket_dns_overflow_capacity: values[6],
        responses_lane_limit: values[7],
        compact_lane_limit: values[8],
        websocket_lane_limit: values[9],
        standard_lane_limit: values[10],
    })
}

/// Resolve one runtime-proxy preset through the Mojo-owned normalization
/// kernel. Preset ids are owned by the policy adapter: 0 is low, 1 is
/// default, 2 is many-terminals, and 3 is aggressive.
pub fn runtime_tuning_proxy_preset_defaults(
    preset: i64,
) -> Result<RuntimeTuningProxyPresetDefaults, crate::MojoError> {
    let mut output = [0_i64; RUNTIME_PROXY_PRESET_FIELD_COUNT];
    let status = unsafe { prodex_runtime_proxy_preset_defaults_v1(preset, output.as_mut_ptr()) };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    let values = output
        .into_iter()
        .map(|value| {
            usize::try_from(value)
                .map(|value| (value > 0).then_some(value))
                .map_err(|_| crate::MojoError::InvalidOutput)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeTuningProxyPresetDefaults {
        worker_count: values[0],
        long_lived_worker_count: values[1],
        probe_refresh_worker_count: values[2],
        async_worker_count: values[3],
        long_lived_queue_capacity: values[4],
        active_request_limit: values[5],
        profile_inflight_soft_limit: values[6],
        profile_inflight_hard_limit: values[7],
        responses_active_limit: values[8],
        compact_active_limit: values[9],
        websocket_active_limit: values[10],
        standard_active_limit: values[11],
        websocket_connect_worker_count: values[12],
        websocket_connect_queue_capacity: values[13],
        websocket_connect_overflow_capacity: values[14],
        websocket_dns_worker_count: values[15],
        websocket_dns_queue_capacity: values[16],
        websocket_dns_overflow_capacity: values[17],
        startup_sync_probe_warm_limit: values[18],
    })
}

#[cfg(test)]
mod scalar_config_tests {
    use super::*;

    #[test]
    fn runtime_scalar_config_policy_preserves_caller_whitespace_contracts() {
        assert_eq!(
            runtime_log_format_class("\u{2003}JSON\u{2003}").unwrap(),
            Some(RuntimeLogFormatClass::Json)
        );
        assert_eq!(
            runtime_proxy_preset_class("Many_Terminals").unwrap(),
            Some(RuntimeProxyPresetClass::ManyTerminals)
        );
        assert_eq!(
            runtime_proxy_preset_class(" many-terminals ").unwrap(),
            None
        );
        assert_eq!(
            runtime_web_search_mode_class("LiVe").unwrap(),
            Some(RuntimeWebSearchModeClass::Live)
        );
        assert_eq!(runtime_web_search_mode_class(" live ").unwrap(), None);
        assert_eq!(
            runtime_clock_source_class("ExTeRnAl").unwrap(),
            Some(RuntimeClockSourceClass::External)
        );
        assert_eq!(runtime_clock_source_class(" external ").unwrap(), None);
        assert!(runtime_model_provider_is_openai("OPENAI").unwrap());
        assert!(!runtime_model_provider_is_openai(" openai ").unwrap());
    }
}
