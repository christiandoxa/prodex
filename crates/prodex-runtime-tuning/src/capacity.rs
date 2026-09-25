use super::RuntimeTuningLaneLimits;
use super::mojo;

pub fn runtime_proxy_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).worker_count
}

pub fn runtime_proxy_long_lived_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).long_lived_worker_count
}

pub fn runtime_probe_refresh_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).probe_refresh_worker_count
}

pub fn runtime_proxy_async_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).async_worker_count
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeTuningCapacityDefaults {
    pub(super) long_lived_queue_capacity: usize,
    pub(super) active_request_limit: usize,
    pub(super) log_queue_capacity: usize,
    pub(super) websocket_connect_queue_capacity: usize,
    pub(super) websocket_connect_overflow_capacity: usize,
    pub(super) websocket_dns_queue_capacity: usize,
    pub(super) websocket_dns_overflow_capacity: usize,
    pub(super) responses_lane_limit: usize,
    pub(super) compact_lane_limit: usize,
    pub(super) websocket_lane_limit: usize,
    pub(super) standard_lane_limit: usize,
}

fn mojo_capacity_defaults(
    parallelism: usize,
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: RuntimeProxyLaneLimitOverrides,
    queue_overrides: [Option<usize>; 2],
) -> RuntimeTuningCapacityDefaults {
    mojo::runtime_tuning_capacity_defaults(
        parallelism,
        global_limit,
        worker_count,
        long_lived_worker_count,
        overrides,
        queue_overrides,
    )
}

pub fn runtime_proxy_long_lived_queue_capacity_default(worker_count: usize) -> usize {
    mojo_capacity_defaults(
        4,
        64,
        4,
        worker_count,
        RuntimeProxyLaneLimitOverrides::default(),
        [None, None],
    )
    .long_lived_queue_capacity
}

pub fn runtime_proxy_active_request_limit_default(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    mojo_capacity_defaults(
        4,
        64,
        worker_count,
        long_lived_worker_count,
        RuntimeProxyLaneLimitOverrides::default(),
        [None, None],
    )
    .active_request_limit
}

pub fn runtime_proxy_log_queue_capacity_default(parallelism: usize) -> usize {
    mojo_capacity_defaults(
        parallelism,
        64,
        runtime_proxy_worker_count_default(parallelism),
        runtime_proxy_long_lived_worker_count_default(parallelism),
        RuntimeProxyLaneLimitOverrides::default(),
        [None, None],
    )
    .log_queue_capacity
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeProxyLaneLimitOverrides {
    pub responses: Option<usize>,
    pub compact: Option<usize>,
    pub websocket: Option<usize>,
    pub standard: Option<usize>,
}

pub fn runtime_proxy_lane_limits_from_overrides(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: RuntimeProxyLaneLimitOverrides,
) -> RuntimeTuningLaneLimits {
    let defaults = mojo_capacity_defaults(
        4,
        global_limit,
        worker_count,
        long_lived_worker_count,
        overrides,
        [None, None],
    );
    RuntimeTuningLaneLimits {
        responses: defaults.responses_lane_limit,
        compact: defaults.compact_lane_limit,
        websocket: defaults.websocket_lane_limit,
        standard: defaults.standard_lane_limit,
    }
}

pub fn runtime_websocket_tcp_connect_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).websocket_connect_worker_count
}

pub fn runtime_websocket_tcp_connect_queue_capacity_default(worker_count: usize) -> usize {
    mojo_capacity_defaults(
        worker_count,
        64,
        4,
        8,
        RuntimeProxyLaneLimitOverrides::default(),
        [None, None],
    )
    .websocket_connect_queue_capacity
}

pub fn runtime_websocket_tcp_connect_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    mojo_capacity_defaults(
        worker_count,
        64,
        4,
        8,
        RuntimeProxyLaneLimitOverrides::default(),
        [Some(queue_capacity), None],
    )
    .websocket_connect_overflow_capacity
}

pub fn runtime_websocket_dns_resolve_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).websocket_dns_worker_count
}

pub fn runtime_websocket_dns_resolve_queue_capacity_default(worker_count: usize) -> usize {
    mojo_capacity_defaults(
        worker_count,
        64,
        4,
        8,
        RuntimeProxyLaneLimitOverrides::default(),
        [None, None],
    )
    .websocket_dns_queue_capacity
}

pub fn runtime_websocket_dns_resolve_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    mojo_capacity_defaults(
        worker_count,
        64,
        4,
        8,
        RuntimeProxyLaneLimitOverrides::default(),
        [None, Some(queue_capacity)],
    )
    .websocket_dns_overflow_capacity
}
