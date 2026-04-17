use super::*;

pub(crate) fn runtime_persistence_mode_by_log_path() -> &'static Mutex<BTreeMap<PathBuf, bool>> {
    RUNTIME_PERSISTENCE_MODE_BY_LOG_PATH.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(crate) fn register_runtime_proxy_persistence_mode(log_path: &Path, enabled: bool) {
    let mut modes = runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    modes.insert(log_path.to_path_buf(), enabled);
}

pub(crate) fn unregister_runtime_proxy_persistence_mode(log_path: &Path) {
    let mut modes = runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    modes.remove(log_path);
}

pub(crate) fn runtime_proxy_persistence_enabled_for_log_path(log_path: &Path) -> bool {
    runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .copied()
        .unwrap_or(true)
}

pub(crate) fn runtime_proxy_persistence_enabled(shared: &RuntimeRotationProxyShared) -> bool {
    runtime_proxy_persistence_enabled_for_log_path(&shared.log_path)
}

pub(crate) fn runtime_broker_metadata_by_log_path()
-> &'static Mutex<BTreeMap<PathBuf, RuntimeBrokerMetadata>> {
    RUNTIME_BROKER_METADATA_BY_LOG_PATH.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(crate) fn register_runtime_broker_metadata(log_path: &Path, metadata: RuntimeBrokerMetadata) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    metadata_by_path.insert(log_path.to_path_buf(), metadata);
}

pub(crate) fn unregister_runtime_broker_metadata(log_path: &Path) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    metadata_by_path.remove(log_path);
}

#[allow(dead_code)]
pub(crate) fn runtime_broker_metadata_for_log_path(
    log_path: &Path,
) -> Option<RuntimeBrokerMetadata> {
    runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .cloned()
}
