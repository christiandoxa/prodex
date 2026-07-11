use prodex_shared_types::RuntimeQuotaSource;

pub fn runtime_quota_source_to_proxy(
    source: RuntimeQuotaSource,
) -> runtime_proxy::RuntimeSelectionQuotaSource {
    match source {
        RuntimeQuotaSource::LiveProbe => runtime_proxy::RuntimeSelectionQuotaSource::LiveProbe,
        RuntimeQuotaSource::PersistedSnapshot => {
            runtime_proxy::RuntimeSelectionQuotaSource::PersistedSnapshot
        }
    }
}

pub fn runtime_quota_source_option_to_proxy(
    source: Option<RuntimeQuotaSource>,
) -> Option<runtime_proxy::RuntimeSelectionQuotaSource> {
    source.map(runtime_quota_source_to_proxy)
}

pub fn runtime_quota_source_from_proxy(
    source: runtime_proxy::RuntimeSelectionQuotaSource,
) -> RuntimeQuotaSource {
    match source {
        runtime_proxy::RuntimeSelectionQuotaSource::LiveProbe => RuntimeQuotaSource::LiveProbe,
        runtime_proxy::RuntimeSelectionQuotaSource::PersistedSnapshot => {
            RuntimeQuotaSource::PersistedSnapshot
        }
    }
}

pub fn runtime_quota_source_label(source: RuntimeQuotaSource) -> &'static str {
    runtime_proxy::runtime_selection_quota_source_label(runtime_quota_source_to_proxy(source))
}
