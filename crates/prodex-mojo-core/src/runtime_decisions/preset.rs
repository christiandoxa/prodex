use super::tuning::RuntimeProxyPresetClass;

/// Values supplied by a selected runtime-proxy policy preset.
///
/// `None` means the preset leaves that field unset so downstream runtime
/// tuning can apply its normal default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeTuningProxyPresetValues {
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
    pub http_connect_timeout_ms: Option<u64>,
    pub stream_idle_timeout_ms: Option<u64>,
    pub compact_request_timeout_ms: Option<u64>,
    pub sse_lookahead_timeout_ms: Option<u64>,
    pub prefetch_backpressure_retry_ms: Option<u64>,
    pub prefetch_backpressure_timeout_ms: Option<u64>,
    pub prefetch_max_buffered_bytes: Option<usize>,
    pub websocket_connect_timeout_ms: Option<u64>,
    pub websocket_happy_eyeballs_delay_ms: Option<u64>,
    pub websocket_precommit_progress_timeout_ms: Option<u64>,
    pub broker_ready_timeout_ms: Option<u64>,
    pub broker_health_connect_timeout_ms: Option<u64>,
    pub broker_health_read_timeout_ms: Option<u64>,
    pub websocket_previous_response_reuse_stale_ms: Option<u64>,
    pub admission_wait_budget_ms: Option<u64>,
    pub pressure_admission_wait_budget_ms: Option<u64>,
    pub long_lived_queue_wait_budget_ms: Option<u64>,
    pub pressure_long_lived_queue_wait_budget_ms: Option<u64>,
    pub sync_probe_pressure_pause_ms: Option<u64>,
    pub responses_critical_floor_percent: Option<i64>,
    pub startup_sync_probe_warm_limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningProxyPresetPlan {
    pub effective_preset: Option<RuntimeProxyPresetClass>,
    pub values: RuntimeTuningProxyPresetValues,
}

const RUNTIME_PROXY_PRESET_PLAN_ABI_VERSION: i64 = 1;
const RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT: usize = 39;
const RUNTIME_PROXY_PRESET_PLAN_VALID_MASK: u64 =
    (1_u64 << RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT) - 1;

unsafe extern "C" {
    fn prodex_runtime_proxy_preset_plan_v1(
        abi_version: i64,
        configured_preset: i64,
        environment_preset: i64,
        override_presence_mask: u64,
        override_values_address: usize,
        override_critical_floor: i64,
        output_presence_mask_address: usize,
        output_values_address: usize,
        output_effective_preset_address: usize,
        output_critical_floor_address: usize,
    ) -> i64;
}

impl RuntimeTuningProxyPresetValues {
    fn into_abi(
        self,
    ) -> Result<([u64; RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT], u64, i64), crate::MojoError> {
        let mut values = [0_u64; RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT];
        let mut presence_mask = 0_u64;
        macro_rules! encode {
            ($index:literal, $field:ident) => {
                if let Some(value) = self.$field {
                    values[$index] =
                        u64::try_from(value).map_err(|_| crate::MojoError::InvalidInput)?;
                    presence_mask |= 1_u64 << $index;
                }
            };
        }
        encode!(0, worker_count);
        encode!(1, long_lived_worker_count);
        encode!(2, probe_refresh_worker_count);
        encode!(3, async_worker_count);
        encode!(4, long_lived_queue_capacity);
        encode!(5, active_request_limit);
        encode!(6, profile_inflight_soft_limit);
        encode!(7, profile_inflight_hard_limit);
        encode!(8, responses_active_limit);
        encode!(9, compact_active_limit);
        encode!(10, websocket_active_limit);
        encode!(11, standard_active_limit);
        encode!(12, http_connect_timeout_ms);
        encode!(13, stream_idle_timeout_ms);
        encode!(14, compact_request_timeout_ms);
        encode!(15, sse_lookahead_timeout_ms);
        encode!(16, prefetch_backpressure_retry_ms);
        encode!(17, prefetch_backpressure_timeout_ms);
        encode!(18, prefetch_max_buffered_bytes);
        encode!(19, websocket_connect_timeout_ms);
        encode!(20, websocket_happy_eyeballs_delay_ms);
        encode!(21, websocket_precommit_progress_timeout_ms);
        encode!(22, websocket_connect_worker_count);
        encode!(23, websocket_connect_queue_capacity);
        encode!(24, websocket_connect_overflow_capacity);
        encode!(25, websocket_dns_worker_count);
        encode!(26, websocket_dns_queue_capacity);
        encode!(27, websocket_dns_overflow_capacity);
        encode!(28, broker_ready_timeout_ms);
        encode!(29, broker_health_connect_timeout_ms);
        encode!(30, broker_health_read_timeout_ms);
        encode!(31, websocket_previous_response_reuse_stale_ms);
        encode!(32, admission_wait_budget_ms);
        encode!(33, pressure_admission_wait_budget_ms);
        encode!(34, long_lived_queue_wait_budget_ms);
        encode!(35, pressure_long_lived_queue_wait_budget_ms);
        encode!(36, sync_probe_pressure_pause_ms);
        encode!(38, startup_sync_probe_warm_limit);
        let critical_floor = self.responses_critical_floor_percent.unwrap_or_default();
        if self.responses_critical_floor_percent.is_some() {
            presence_mask |= 1_u64 << 37;
        }
        Ok((values, presence_mask, critical_floor))
    }

    fn from_abi(
        values: [u64; RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT],
        presence_mask: u64,
        critical_floor: i64,
    ) -> Result<Self, crate::MojoError> {
        macro_rules! usize_value {
            ($index:literal) => {
                if presence_mask & (1_u64 << $index) != 0 {
                    Some(
                        usize::try_from(values[$index])
                            .map_err(|_| crate::MojoError::InvalidOutput)?,
                    )
                } else {
                    None
                }
            };
        }
        macro_rules! u64_value {
            ($index:literal) => {
                (presence_mask & (1_u64 << $index) != 0).then_some(values[$index])
            };
        }
        Ok(Self {
            worker_count: usize_value!(0),
            long_lived_worker_count: usize_value!(1),
            probe_refresh_worker_count: usize_value!(2),
            async_worker_count: usize_value!(3),
            long_lived_queue_capacity: usize_value!(4),
            active_request_limit: usize_value!(5),
            profile_inflight_soft_limit: usize_value!(6),
            profile_inflight_hard_limit: usize_value!(7),
            responses_active_limit: usize_value!(8),
            compact_active_limit: usize_value!(9),
            websocket_active_limit: usize_value!(10),
            standard_active_limit: usize_value!(11),
            http_connect_timeout_ms: u64_value!(12),
            stream_idle_timeout_ms: u64_value!(13),
            compact_request_timeout_ms: u64_value!(14),
            sse_lookahead_timeout_ms: u64_value!(15),
            prefetch_backpressure_retry_ms: u64_value!(16),
            prefetch_backpressure_timeout_ms: u64_value!(17),
            prefetch_max_buffered_bytes: usize_value!(18),
            websocket_connect_timeout_ms: u64_value!(19),
            websocket_happy_eyeballs_delay_ms: u64_value!(20),
            websocket_precommit_progress_timeout_ms: u64_value!(21),
            websocket_connect_worker_count: usize_value!(22),
            websocket_connect_queue_capacity: usize_value!(23),
            websocket_connect_overflow_capacity: usize_value!(24),
            websocket_dns_worker_count: usize_value!(25),
            websocket_dns_queue_capacity: usize_value!(26),
            websocket_dns_overflow_capacity: usize_value!(27),
            broker_ready_timeout_ms: u64_value!(28),
            broker_health_connect_timeout_ms: u64_value!(29),
            broker_health_read_timeout_ms: u64_value!(30),
            websocket_previous_response_reuse_stale_ms: u64_value!(31),
            admission_wait_budget_ms: u64_value!(32),
            pressure_admission_wait_budget_ms: u64_value!(33),
            long_lived_queue_wait_budget_ms: u64_value!(34),
            pressure_long_lived_queue_wait_budget_ms: u64_value!(35),
            sync_probe_pressure_pause_ms: u64_value!(36),
            responses_critical_floor_percent: (presence_mask & (1_u64 << 37) != 0)
                .then_some(critical_floor),
            startup_sync_probe_warm_limit: usize_value!(38),
        })
    }
}

/// Resolve preset precedence, preset defaults, and explicit field overrides in Mojo.
pub fn runtime_tuning_proxy_preset_plan(
    configured_preset: Option<RuntimeProxyPresetClass>,
    environment_preset: Option<RuntimeProxyPresetClass>,
    overrides: RuntimeTuningProxyPresetValues,
) -> Result<RuntimeTuningProxyPresetPlan, crate::MojoError> {
    fn preset_id(preset: Option<RuntimeProxyPresetClass>) -> i64 {
        match preset {
            None => -1,
            Some(RuntimeProxyPresetClass::Low) => 0,
            Some(RuntimeProxyPresetClass::Default) => 1,
            Some(RuntimeProxyPresetClass::ManyTerminals) => 2,
            Some(RuntimeProxyPresetClass::Aggressive) => 3,
        }
    }

    fn preset_from_id(id: i64) -> Result<Option<RuntimeProxyPresetClass>, crate::MojoError> {
        match id {
            -1 => Ok(None),
            0 => Ok(Some(RuntimeProxyPresetClass::Low)),
            1 => Ok(Some(RuntimeProxyPresetClass::Default)),
            2 => Ok(Some(RuntimeProxyPresetClass::ManyTerminals)),
            3 => Ok(Some(RuntimeProxyPresetClass::Aggressive)),
            _ => Err(crate::MojoError::InvalidOutput),
        }
    }

    let (override_values, override_presence_mask, override_critical_floor) =
        overrides.into_abi()?;
    let mut output_values = [0_u64; RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT];
    let mut output_presence_mask = 0_u64;
    let mut output_effective_preset = -1_i64;
    let mut output_critical_floor = 0_i64;
    let status = unsafe {
        prodex_runtime_proxy_preset_plan_v1(
            RUNTIME_PROXY_PRESET_PLAN_ABI_VERSION,
            preset_id(configured_preset),
            preset_id(environment_preset),
            override_presence_mask,
            override_values.as_ptr() as usize,
            override_critical_floor,
            &mut output_presence_mask as *mut u64 as usize,
            output_values.as_mut_ptr() as usize,
            &mut output_effective_preset as *mut i64 as usize,
            &mut output_critical_floor as *mut i64 as usize,
        )
    };
    match status {
        0 => {}
        -3 => return Err(crate::MojoError::AbiMismatch),
        -2 => return Err(crate::MojoError::InvalidInput),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    if output_presence_mask & !RUNTIME_PROXY_PRESET_PLAN_VALID_MASK != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(RuntimeTuningProxyPresetPlan {
        effective_preset: preset_from_id(output_effective_preset)?,
        values: RuntimeTuningProxyPresetValues::from_abi(
            output_values,
            output_presence_mask,
            output_critical_floor,
        )?,
    })
}
