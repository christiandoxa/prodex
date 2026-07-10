use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
#[cfg(not(test))]
use std::sync::{Mutex, OnceLock};

use crate::types::RuntimePolicyConfig;

#[cfg(not(test))]
static RUNTIME_POLICY_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>>> =
    OnceLock::new();

#[cfg(test)]
thread_local! {
    static RUNTIME_POLICY_CACHE: std::cell::RefCell<BTreeMap<PathBuf, Option<RuntimePolicyConfig>>> =
        std::cell::RefCell::new(BTreeMap::new());
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePolicyCacheInvalidationPlan {
    pub root: PathBuf,
    pub had_cached_entry: bool,
    pub cached_policy_version: Option<u32>,
}

#[cfg(not(test))]
fn runtime_policy_cache_with<R>(
    f: impl FnOnce(&mut BTreeMap<PathBuf, Option<RuntimePolicyConfig>>) -> R,
) -> R {
    let mut guard = RUNTIME_POLICY_CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

#[cfg(test)]
fn runtime_policy_cache_with<R>(
    f: impl FnOnce(&mut BTreeMap<PathBuf, Option<RuntimePolicyConfig>>) -> R,
) -> R {
    RUNTIME_POLICY_CACHE.with(|cache| f(&mut cache.borrow_mut()))
}

pub fn clear_runtime_policy_cache() {
    runtime_policy_cache_with(|cache| cache.clear());
}

pub fn invalidate_runtime_policy_cache_for(root: &Path) -> Option<Option<RuntimePolicyConfig>> {
    runtime_policy_cache_with(|cache| cache.remove(&runtime_policy_cache_key(root)))
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
    runtime_policy_cache_with(|cache| cache.get(&runtime_policy_cache_key(root)).cloned())
}

pub(crate) fn store_cached_policy(root: &Path, policy: Option<RuntimePolicyConfig>) {
    runtime_policy_cache_with(|cache| {
        cache.insert(runtime_policy_cache_key(root), policy);
    });
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
