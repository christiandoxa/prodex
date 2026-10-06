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

pub fn runtime_proxy_long_lived_queue_capacity_default(worker_count: usize) -> usize {
    mojo::capacity_default(0, worker_count, 0)
}

pub fn runtime_proxy_active_request_limit_default(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    mojo::capacity_default(1, worker_count, long_lived_worker_count)
}

pub fn runtime_proxy_log_queue_capacity_default(parallelism: usize) -> usize {
    mojo::capacity_default(2, parallelism, 0)
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
    let defaults = mojo::runtime_tuning_capacity_defaults(
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
    mojo::capacity_default(3, worker_count, 0)
}

pub fn runtime_websocket_tcp_connect_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    mojo::capacity_default(4, worker_count, queue_capacity)
}

pub fn runtime_websocket_dns_resolve_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).websocket_dns_worker_count
}

pub fn runtime_websocket_dns_resolve_queue_capacity_default(worker_count: usize) -> usize {
    mojo::capacity_default(5, worker_count, 0)
}

pub fn runtime_websocket_dns_resolve_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    mojo::capacity_default(6, worker_count, queue_capacity)
}
