use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::types::RuntimePolicyConfig;

static RUNTIME_POLICY_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>>> =
    OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePolicyCacheInvalidationPlan {
    pub root: PathBuf,
    pub had_cached_entry: bool,
    pub cached_policy_version: Option<u32>,
}

fn runtime_policy_cache() -> &'static Mutex<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>> {
    RUNTIME_POLICY_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn clear_runtime_policy_cache() {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

pub fn invalidate_runtime_policy_cache_for(root: &Path) -> Option<Option<RuntimePolicyConfig>> {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&runtime_policy_cache_key(root))
}

pub fn plan_runtime_policy_cache_invalidation(root: &Path) -> RuntimePolicyCacheInvalidationPlan {
    let root = runtime_policy_cache_key(root);
    let invalidated = invalidate_runtime_policy_cache_for(&root);
    RuntimePolicyCacheInvalidationPlan {
        root,
        had_cached_entry: invalidated.is_some(),
        cached_policy_version: invalidated.flatten().map(|policy| policy.version),
    }
}

pub(crate) fn cached_policy_for(root: &Path) -> Option<Option<RuntimePolicyConfig>> {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&runtime_policy_cache_key(root))
        .cloned()
}

pub(crate) fn store_cached_policy(root: &Path, policy: Option<RuntimePolicyConfig>) {
    runtime_policy_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(runtime_policy_cache_key(root), policy);
}

fn runtime_policy_cache_key(root: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in root.components() {
        if !matches!(component, Component::CurDir) {
            normalized.push(component.as_os_str());
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}
