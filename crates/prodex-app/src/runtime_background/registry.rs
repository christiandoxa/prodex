use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::{
    RUNTIME_BROKER_METADATA_BY_LOG_PATH, RUNTIME_PERSISTENCE_MODE_BY_LOG_PATH,
    RuntimeBrokerMetadata, RuntimeRotationProxyShared,
    clear_runtime_proxy_continuity_failure_reason_metrics,
    unregister_runtime_presidio_redaction_proxy_state,
    unregister_runtime_smart_context_proxy_state,
};

pub(crate) struct RuntimeProxyMarkerGuard {
    log_path: PathBuf,
}

impl RuntimeProxyMarkerGuard {
    pub(crate) fn new(log_path: &Path) -> Self {
        Self {
            log_path: log_path.to_path_buf(),
        }
    }
}

impl Drop for RuntimeProxyMarkerGuard {
    fn drop(&mut self) {
        clear_runtime_proxy_continuity_failure_reason_metrics(&self.log_path);
        unregister_runtime_proxy_persistence_mode(&self.log_path);
        unregister_runtime_broker_metadata(&self.log_path);
        unregister_runtime_smart_context_proxy_state(&self.log_path);
        unregister_runtime_presidio_redaction_proxy_state(&self.log_path);
    }
}

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

pub(crate) fn runtime_broker_metadata_for_log_path(
    log_path: &Path,
) -> Option<RuntimeBrokerMetadata> {
    runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        register_runtime_smart_context_proxy_state, runtime_smart_context_proxy_state_registered,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_proxy_marker_guard_removes_registered_state() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let log_path = PathBuf::from(format!("/tmp/prodex-marker-{suffix}.log"));
        let guard = RuntimeProxyMarkerGuard::new(&log_path);
        register_runtime_proxy_persistence_mode(&log_path, false);
        register_runtime_smart_context_proxy_state(&log_path, false, None, None);

        assert!(!runtime_proxy_persistence_enabled_for_log_path(&log_path));
        assert!(runtime_smart_context_proxy_state_registered(&log_path));

        drop(guard);

        assert!(runtime_proxy_persistence_enabled_for_log_path(&log_path));
        assert!(!runtime_smart_context_proxy_state_registered(&log_path));
    }
}
