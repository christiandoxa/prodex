use std::fs;
use std::path::PathBuf;

use crate::{AppPaths, AppState, last_good_file_path};

pub(crate) const RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX: &str = "live-";

pub(crate) fn merge_app_state_for_save(existing: AppState, desired: &AppState) -> AppState {
    prodex_state::merge_app_state_for_save(existing, desired)
}

pub(crate) fn runtime_continuations_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuations.json")
}

pub(crate) fn runtime_continuations_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuations_file_path(paths))
}

pub(crate) fn runtime_continuation_journal_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuation-journal.json")
}

pub(crate) fn runtime_continuation_journal_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuation_journal_file_path(paths))
}

pub(crate) fn runtime_broker_registry_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths.root.join(format!("runtime-broker-{broker_key}.json"))
}

pub(crate) fn runtime_broker_registry_last_good_file_path(
    paths: &AppPaths,
    broker_key: &str,
) -> PathBuf {
    last_good_file_path(&runtime_broker_registry_file_path(paths, broker_key))
}

pub(crate) fn runtime_broker_capability_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}.capability"))
}

pub(crate) fn runtime_broker_lease_dir(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-leases"))
}

pub(crate) fn runtime_broker_ensure_lock_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-ensure"))
}

pub(crate) fn runtime_broker_registry_keys(paths: &AppPaths) -> Vec<String> {
    runtime_registry_keys(paths, |key| {
        !key.starts_with(RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX)
    })
}

pub(crate) fn runtime_live_log_source_registry_keys(paths: &AppPaths) -> Vec<String> {
    runtime_registry_keys(paths, |key| {
        key.starts_with(RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX)
    })
}

fn runtime_registry_keys(paths: &AppPaths, include: impl Fn(&str) -> bool) -> Vec<String> {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return Vec::new();
    };

    let mut keys = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            name.strip_prefix("runtime-broker-")
                .and_then(|suffix| suffix.strip_suffix(".json"))
                .filter(|key| include(key))
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys
}

pub(crate) fn update_check_cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("update-check.json")
}
