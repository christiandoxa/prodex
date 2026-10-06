use super::RuntimeTuningDefaults;
use super::capacity::RuntimeProxyLaneLimitOverrides;
use prodex_mojo_core::runtime::RuntimeTuningCapacityDefaults;

pub(super) fn runtime_tuning_defaults(parallelism: usize) -> RuntimeTuningDefaults {
    prodex_mojo_core::runtime::runtime_tuning_defaults(parallelism)
        .expect("Mojo runtime tuning defaults returned an invalid result")
}

pub(super) fn runtime_tuning_capacity_defaults(
    parallelism: usize,
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: RuntimeProxyLaneLimitOverrides,
    queue_overrides: [Option<usize>; 2],
) -> RuntimeTuningCapacityDefaults {
    prodex_mojo_core::runtime::runtime_tuning_capacity_defaults(
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
    .expect("Mojo runtime tuning capacity defaults returned an invalid result")
}

pub(super) fn capacity_default(operation: i64, value: usize, secondary: usize) -> usize {
    prodex_mojo_core::runtime::runtime_tuning_capacity_default(operation, value, secondary)
        .expect("Mojo runtime tuning capacity default returned an invalid result")
}
