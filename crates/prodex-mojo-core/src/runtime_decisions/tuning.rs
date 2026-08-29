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

unsafe extern "C" {
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
