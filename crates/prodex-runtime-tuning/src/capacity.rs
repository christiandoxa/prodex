use super::RuntimeTuningLaneLimits;
#[cfg(feature = "mojo")]
use super::mojo;

#[cfg(feature = "mojo")]
pub fn runtime_proxy_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).worker_count
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_worker_count_default(parallelism: usize) -> usize {
    runtime_proxy_worker_count_default_rust(parallelism)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_proxy_worker_count_default_rust(parallelism: usize) -> usize {
    parallelism.clamp(4, 12)
}

#[cfg(feature = "mojo")]
pub fn runtime_proxy_long_lived_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).long_lived_worker_count
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_long_lived_worker_count_default(parallelism: usize) -> usize {
    runtime_proxy_long_lived_worker_count_default_rust(parallelism)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_proxy_long_lived_worker_count_default_rust(parallelism: usize) -> usize {
    parallelism.saturating_mul(2).clamp(8, 24)
}

#[cfg(feature = "mojo")]
pub fn runtime_probe_refresh_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).probe_refresh_worker_count
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_probe_refresh_worker_count_default(parallelism: usize) -> usize {
    runtime_probe_refresh_worker_count_default_rust(parallelism)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_probe_refresh_worker_count_default_rust(parallelism: usize) -> usize {
    parallelism.clamp(2, 4)
}

#[cfg(feature = "mojo")]
pub fn runtime_proxy_async_worker_count_default(parallelism: usize) -> usize {
    mojo::runtime_tuning_defaults(parallelism).async_worker_count
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_async_worker_count_default(parallelism: usize) -> usize {
    runtime_proxy_async_worker_count_default_rust(parallelism)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_proxy_async_worker_count_default_rust(parallelism: usize) -> usize {
    parallelism.clamp(2, 4)
}

#[cfg(feature = "mojo")]
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

#[cfg(feature = "mojo")]
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

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_long_lived_queue_capacity_default(worker_count: usize) -> usize {
    runtime_proxy_long_lived_queue_capacity_default_rust(worker_count)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_proxy_long_lived_queue_capacity_default_rust(worker_count: usize) -> usize {
    worker_count.saturating_mul(8).clamp(128, 1024)
}

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_active_request_limit_default(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    runtime_proxy_active_request_limit_default_rust(worker_count, long_lived_worker_count)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_proxy_active_request_limit_default_rust(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    worker_count
        .saturating_add(long_lived_worker_count.saturating_mul(3))
        .clamp(64, 512)
}

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_log_queue_capacity_default(parallelism: usize) -> usize {
    runtime_proxy_log_queue_capacity_default_rust(parallelism)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_proxy_log_queue_capacity_default_rust(parallelism: usize) -> usize {
    parallelism.saturating_mul(256).clamp(1024, 8192)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeProxyLaneLimitOverrides {
    pub responses: Option<usize>,
    pub compact: Option<usize>,
    pub websocket: Option<usize>,
    pub standard: Option<usize>,
}

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_lane_limits_from_overrides(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: RuntimeProxyLaneLimitOverrides,
) -> RuntimeTuningLaneLimits {
    runtime_proxy_lane_limits_from_overrides_rust(
        global_limit,
        worker_count,
        long_lived_worker_count,
        overrides,
    )
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_proxy_lane_limits_from_overrides_rust(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: RuntimeProxyLaneLimitOverrides,
) -> RuntimeTuningLaneLimits {
    let global_limit = global_limit.max(1);
    RuntimeTuningLaneLimits {
        responses: overrides
            .responses
            .filter(|value| *value > 0)
            .unwrap_or_else(|| (global_limit.saturating_mul(3) / 4).clamp(4, global_limit))
            .min(global_limit)
            .max(1),
        compact: overrides
            .compact
            .filter(|value| *value > 0)
            .unwrap_or_else(|| (global_limit / 4).clamp(2, 6).min(global_limit))
            .min(global_limit)
            .max(1),
        websocket: overrides
            .websocket
            .filter(|value| *value > 0)
            .unwrap_or_else(|| long_lived_worker_count.clamp(2, global_limit))
            .min(global_limit)
            .max(1),
        standard: overrides
            .standard
            .filter(|value| *value > 0)
            .unwrap_or_else(|| {
                worker_count
                    .saturating_mul(2)
                    .clamp(8, 24)
                    .min(global_limit)
            })
            .min(global_limit)
            .max(1),
    }
}

pub fn runtime_websocket_tcp_connect_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(4, 16)
}

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_websocket_tcp_connect_queue_capacity_default(worker_count: usize) -> usize {
    runtime_websocket_tcp_connect_queue_capacity_default_rust(worker_count)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_websocket_tcp_connect_queue_capacity_default_rust(
    worker_count: usize,
) -> usize {
    worker_count.saturating_mul(8).clamp(32, 128)
}

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_websocket_tcp_connect_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    runtime_websocket_tcp_connect_overflow_capacity_default_rust(worker_count, queue_capacity)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_websocket_tcp_connect_overflow_capacity_default_rust(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    queue_capacity
        .saturating_mul(4)
        .max(worker_count)
        .clamp(32, 512)
}

pub fn runtime_websocket_dns_resolve_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(2, 8)
}

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_websocket_dns_resolve_queue_capacity_default(worker_count: usize) -> usize {
    runtime_websocket_dns_resolve_queue_capacity_default_rust(worker_count)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_websocket_dns_resolve_queue_capacity_default_rust(
    worker_count: usize,
) -> usize {
    worker_count.saturating_mul(4).clamp(16, 64)
}

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub fn runtime_websocket_dns_resolve_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    runtime_websocket_dns_resolve_overflow_capacity_default_rust(worker_count, queue_capacity)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn runtime_websocket_dns_resolve_overflow_capacity_default_rust(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    queue_capacity
        .saturating_mul(2)
        .max(worker_count)
        .clamp(16, 128)
}
