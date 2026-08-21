use super::RuntimeTuningDefaults;

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
