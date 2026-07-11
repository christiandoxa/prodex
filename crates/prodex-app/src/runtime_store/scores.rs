use anyhow::Result;
use chrono::Local;
use std::collections::BTreeMap;

use super::io::{
    read_versioned_json_file_with_backup, remember_runtime_sidecar_generation,
    runtime_scores_file_path, runtime_scores_last_good_file_path,
    save_versioned_json_file_with_fence,
};
use crate::{AppPaths, AppState, AppStateIoExt, ProfileEntry, RecoveredLoad, RuntimeProfileHealth};

pub(crate) use prodex_runtime_store::{
    compact_runtime_profile_scores, runtime_profile_score_profile_name,
};

pub(crate) fn merge_runtime_profile_scores(
    existing: &BTreeMap<String, RuntimeProfileHealth>,
    incoming: &BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, RuntimeProfileHealth> {
    prodex_runtime_store::merge_runtime_profile_scores(
        existing,
        incoming,
        profiles,
        Local::now().timestamp(),
    )
}

pub(crate) fn load_runtime_profile_scores(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<BTreeMap<String, RuntimeProfileHealth>> {
    let path = runtime_scores_file_path(paths);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let loaded = read_versioned_json_file_with_backup::<BTreeMap<String, RuntimeProfileHealth>>(
        &path,
        &runtime_scores_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(compact_runtime_profile_scores(
        loaded.value,
        profiles,
        Local::now().timestamp(),
    ))
}

pub(crate) fn load_runtime_profile_scores_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<BTreeMap<String, RuntimeProfileHealth>>> {
    let path = runtime_scores_file_path(paths);
    if !path.exists() && !runtime_scores_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<BTreeMap<String, RuntimeProfileHealth>>(
        &path,
        &runtime_scores_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_profile_scores(loaded.value, profiles, Local::now().timestamp()),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn save_runtime_profile_scores(
    paths: &AppPaths,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_profile_scores_for_profiles(paths, scores, &profiles)
}

pub(crate) fn save_runtime_profile_scores_for_profiles(
    paths: &AppPaths,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<()> {
    let path = runtime_scores_file_path(paths);
    let compacted =
        compact_runtime_profile_scores(scores.clone(), profiles, Local::now().timestamp());
    save_versioned_json_file_with_fence(
        &path,
        &runtime_scores_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}
