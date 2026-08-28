use super::RuntimeTuningDefaults;
use super::capacity::{RuntimeProxyLaneLimitOverrides, RuntimeTuningCapacityDefaults};

pub(super) fn runtime_tuning_defaults(parallelism: usize) -> RuntimeTuningDefaults {
    let values = prodex_mojo_core::runtime::runtime_tuning_defaults(parallelism)
        .expect("Mojo runtime tuning defaults returned an invalid result");
    RuntimeTuningDefaults {
        worker_count: values.worker_count,
        long_lived_worker_count: values.long_lived_worker_count,
        probe_refresh_worker_count: values.probe_refresh_worker_count,
        async_worker_count: values.async_worker_count,
        log_queue_capacity: values.log_queue_capacity,
        websocket_connect_worker_count: values.websocket_connect_worker_count,
        websocket_dns_worker_count: values.websocket_dns_worker_count,
    }
}

pub(super) fn runtime_tuning_capacity_defaults(
    parallelism: usize,
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: RuntimeProxyLaneLimitOverrides,
    queue_overrides: [Option<usize>; 2],
) -> RuntimeTuningCapacityDefaults {
    let values = prodex_mojo_core::runtime::runtime_tuning_capacity_defaults(
        parallelism,
        global_limit,
        worker_count,
        long_lived_worker_count,
        [
            overrides.responses,
            overrides.compact,
            overrides.websocket,
            overrides.standard,
        ],
        queue_overrides,
    )
    .expect("Mojo runtime tuning capacity defaults returned an invalid result");
    RuntimeTuningCapacityDefaults {
        long_lived_queue_capacity: values.long_lived_queue_capacity,
        active_request_limit: values.active_request_limit,
        log_queue_capacity: values.log_queue_capacity,
        websocket_connect_queue_capacity: values.websocket_connect_queue_capacity,
        websocket_connect_overflow_capacity: values.websocket_connect_overflow_capacity,
        websocket_dns_queue_capacity: values.websocket_dns_queue_capacity,
        websocket_dns_overflow_capacity: values.websocket_dns_overflow_capacity,
        responses_lane_limit: values.responses_lane_limit,
        compact_lane_limit: values.compact_lane_limit,
        websocket_lane_limit: values.websocket_lane_limit,
        standard_lane_limit: values.standard_lane_limit,
    }
}
