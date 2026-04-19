use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::types::RuntimePolicyConfig;

static RUNTIME_POLICY_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>>> =
    OnceLock::new();

pub(crate) fn runtime_policy_cache()
-> &'static Mutex<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>> {
    RUNTIME_POLICY_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[cfg(test)]
pub(crate) fn clear_runtime_policy_cache() {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

pub(crate) fn cached_policy_for(root: &Path) -> Option<Option<RuntimePolicyConfig>> {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(root)
        .cloned()
}

pub(crate) fn store_cached_policy(root: &Path, policy: Option<RuntimePolicyConfig>) {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(root.to_path_buf(), policy);
}
