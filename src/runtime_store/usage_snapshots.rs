use anyhow::Result;
use chrono::Local;
use std::collections::BTreeMap;

use super::io::{
    read_versioned_json_file_with_backup, remember_runtime_sidecar_generation,
    runtime_usage_snapshots_file_path, runtime_usage_snapshots_last_good_file_path,
    save_versioned_json_file_with_fence,
};
use crate::{
    AppPaths, AppState, ProfileEntry, RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS, RecoveredLoad,
    RuntimeProfileUsageSnapshot,
};

pub(crate) fn compact_runtime_usage_snapshots(
    mut snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot> {
    let oldest_allowed = now.saturating_sub(RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS);
    snapshots.retain(|profile_name, snapshot| {
        profiles.contains_key(profile_name) && snapshot.checked_at >= oldest_allowed
    });
    snapshots
}

pub(crate) fn merge_runtime_usage_snapshots(
    existing: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    incoming: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot> {
    let mut merged = existing.clone();
    for (profile_name, snapshot) in incoming {
        let should_replace = merged
            .get(profile_name)
            .is_none_or(|current| current.checked_at <= snapshot.checked_at);
        if should_replace {
            merged.insert(profile_name.clone(), snapshot.clone());
        }
    }
    compact_runtime_usage_snapshots(merged, profiles, Local::now().timestamp())
}

pub(crate) fn load_runtime_usage_snapshots(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<BTreeMap<String, RuntimeProfileUsageSnapshot>> {
    let path = runtime_usage_snapshots_file_path(paths);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let loaded = read_versioned_json_file_with_backup::<
        BTreeMap<String, RuntimeProfileUsageSnapshot>,
    >(&path, &runtime_usage_snapshots_last_good_file_path(paths))?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(compact_runtime_usage_snapshots(
        loaded.value,
        profiles,
        Local::now().timestamp(),
    ))
}

pub(crate) fn load_runtime_usage_snapshots_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<BTreeMap<String, RuntimeProfileUsageSnapshot>>> {
    let path = runtime_usage_snapshots_file_path(paths);
    if !path.exists() && !runtime_usage_snapshots_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<
        BTreeMap<String, RuntimeProfileUsageSnapshot>,
    >(&path, &runtime_usage_snapshots_last_good_file_path(paths))?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_usage_snapshots(loaded.value, profiles, Local::now().timestamp()),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn save_runtime_usage_snapshots(
    paths: &AppPaths,
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_usage_snapshots_for_profiles(paths, snapshots, &profiles)
}

pub(crate) fn save_runtime_usage_snapshots_for_profiles(
    paths: &AppPaths,
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<()> {
    let path = runtime_usage_snapshots_file_path(paths);
    let compacted =
        compact_runtime_usage_snapshots(snapshots.clone(), profiles, Local::now().timestamp());
    save_versioned_json_file_with_fence(
        &path,
        &runtime_usage_snapshots_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}
