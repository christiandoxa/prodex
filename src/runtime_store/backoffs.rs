use anyhow::Result;
use chrono::Local;
use std::collections::BTreeMap;

use super::io::{
    read_versioned_json_file_with_backup, remember_runtime_sidecar_generation,
    runtime_backoffs_file_path, runtime_backoffs_last_good_file_path,
    save_versioned_json_file_with_fence,
};
use crate::{
    AppPaths, AppState, ProfileEntry, RecoveredLoad, RuntimeProfileBackoffs,
    runtime_profile_route_circuit_profile_name,
    runtime_profile_transport_backoff_key_matches_profiles,
};

pub(crate) fn compact_runtime_profile_backoffs(
    mut backoffs: RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    backoffs
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    backoffs.transport_backoff_until.retain(|key, until| {
        runtime_profile_transport_backoff_key_matches_profiles(key, profiles) && *until > now
    });
    backoffs
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    backoffs
}

pub(crate) fn merge_runtime_profile_backoffs(
    existing: &RuntimeProfileBackoffs,
    incoming: &RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    let mut merged = existing.clone();
    for (profile_name, until) in &incoming.retry_backoff_until {
        merged
            .retry_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (profile_name, until) in &incoming.transport_backoff_until {
        merged
            .transport_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (route_profile_key, until) in &incoming.route_circuit_open_until {
        merged
            .route_circuit_open_until
            .insert(route_profile_key.clone(), *until);
    }
    compact_runtime_profile_backoffs(merged, profiles, now)
}

pub(crate) fn load_runtime_profile_backoffs(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RuntimeProfileBackoffs> {
    let path = runtime_backoffs_file_path(paths);
    if !path.exists() {
        return Ok(RuntimeProfileBackoffs::default());
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeProfileBackoffs>(
        &path,
        &runtime_backoffs_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(compact_runtime_profile_backoffs(
        loaded.value,
        profiles,
        Local::now().timestamp(),
    ))
}

pub(crate) fn load_runtime_profile_backoffs_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<RuntimeProfileBackoffs>> {
    let path = runtime_backoffs_file_path(paths);
    if !path.exists() && !runtime_backoffs_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: RuntimeProfileBackoffs::default(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeProfileBackoffs>(
        &path,
        &runtime_backoffs_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_profile_backoffs(loaded.value, profiles, Local::now().timestamp()),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn save_runtime_profile_backoffs(
    paths: &AppPaths,
    backoffs: &RuntimeProfileBackoffs,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_profile_backoffs_for_profiles(paths, backoffs, &profiles)
}

pub(crate) fn save_runtime_profile_backoffs_for_profiles(
    paths: &AppPaths,
    backoffs: &RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<()> {
    let path = runtime_backoffs_file_path(paths);
    let compacted =
        compact_runtime_profile_backoffs(backoffs.clone(), profiles, Local::now().timestamp());
    save_versioned_json_file_with_fence(
        &path,
        &runtime_backoffs_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}
