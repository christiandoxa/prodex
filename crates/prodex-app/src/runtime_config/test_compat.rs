use super::RuntimeConfig;

pub(crate) fn runtime_proxy_stream_idle_timeout_ms() -> u64 {
    RuntimeConfig::compatibility_current()
        .tuning
        .stream_idle_timeout_ms
}

pub(crate) fn runtime_proxy_websocket_precommit_progress_timeout_ms() -> u64 {
    RuntimeConfig::compatibility_current()
        .tuning
        .websocket_precommit_progress_timeout_ms
}

pub(crate) fn runtime_proxy_profile_inflight_soft_limit() -> usize {
    RuntimeConfig::compatibility_current()
        .tuning
        .profile_inflight_soft_limit
}

pub(crate) fn runtime_proxy_profile_inflight_hard_limit() -> usize {
    RuntimeConfig::compatibility_current()
        .tuning
        .profile_inflight_hard_limit
}
