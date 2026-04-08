use anyhow::{Context, Result, bail};
use chrono::Local;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    AppPaths, AppState, JsonFileLock, LAST_GOOD_FILE_SUFFIX, ProfileEntry,
    RUNTIME_SCORE_RETENTION_SECONDS, RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS, RecoveredLoad,
    RecoveredVersionedLoad, RuntimeProfileBackoffs, RuntimeProfileHealth,
    RuntimeProfileUsageSnapshot, STATE_SAVE_SEQUENCE, StateFileLock, VersionedJson,
    runtime_profile_route_circuit_profile_name,
    runtime_profile_transport_backoff_key_matches_profiles, runtime_take_fault_injection,
};

static RUNTIME_SIDECAR_GENERATION_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> = OnceLock::new();

pub(super) fn acquire_state_file_lock(paths: &AppPaths) -> Result<StateFileLock> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let lock_path = state_lock_file_path(&paths.state_file);
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to lock {}", lock_path.display()))?;
    Ok(StateFileLock { file })
}

pub(super) fn try_acquire_runtime_owner_lock(paths: &AppPaths) -> Result<Option<StateFileLock>> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let lock_path = runtime_owner_lock_file_path(paths);
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(StateFileLock { file })),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to lock {}", lock_path.display())),
    }
}

pub(super) fn state_lock_file_path(state_file: &Path) -> PathBuf {
    json_lock_file_path(state_file)
}

pub(super) fn runtime_owner_lock_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-owner.lock")
}

pub(super) fn json_lock_file_path(path: &Path) -> PathBuf {
    path.with_extension("json.lock")
}

pub(super) fn acquire_json_file_lock(path: &Path) -> Result<JsonFileLock> {
    let lock_path = json_lock_file_path(path);
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to lock {}", lock_path.display()))?;
    Ok(JsonFileLock { file })
}

fn runtime_sidecar_generation_cache() -> &'static Mutex<BTreeMap<PathBuf, u64>> {
    RUNTIME_SIDECAR_GENERATION_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn runtime_sidecar_cached_generation(path: &Path) -> Option<u64> {
    runtime_sidecar_generation_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(path)
        .copied()
}

pub(super) fn remember_runtime_sidecar_generation(path: &Path, generation: u64) {
    runtime_sidecar_generation_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.to_path_buf(), generation);
}

fn forget_runtime_sidecar_generation(path: &Path) {
    runtime_sidecar_generation_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(path);
}

pub(super) fn last_good_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot.json");
    path.with_file_name(format!("{file_name}{LAST_GOOD_FILE_SUFFIX}"))
}

pub(super) fn runtime_sidecar_generation_from_content(content: &str) -> Result<u64> {
    let value: serde_json::Value =
        serde_json::from_str(content).context("failed to parse runtime sidecar json")?;
    Ok(value
        .get("generation")
        .and_then(|value| value.as_u64())
        .unwrap_or(0))
}

pub(super) fn runtime_sidecar_generation_from_disk(path: &Path, backup_path: &Path) -> Result<u64> {
    match fs::read_to_string(path) {
        Ok(content) => runtime_sidecar_generation_from_content(&content).or_else(|primary_err| {
            match fs::read_to_string(backup_path) {
                Ok(backup_content) => runtime_sidecar_generation_from_content(&backup_content)
                    .with_context(|| {
                        format!(
                            "failed to parse {} after primary load error: {primary_err:#}",
                            backup_path.display()
                        )
                    }),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
                Err(err) => {
                    Err(err).with_context(|| format!("failed to read {}", backup_path.display()))
                }
            }
        }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            match fs::read_to_string(backup_path) {
                Ok(backup_content) => runtime_sidecar_generation_from_content(&backup_content)
                    .with_context(|| format!("failed to parse {}", backup_path.display())),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
                Err(err) => {
                    Err(err).with_context(|| format!("failed to read {}", backup_path.display()))
                }
            }
        }
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub(super) fn parse_versioned_json_or_raw<T>(content: &str) -> Result<(T, u64)>
where
    T: for<'de> Deserialize<'de>,
{
    match serde_json::from_str::<VersionedJson<T>>(content) {
        Ok(versioned) => Ok((versioned.value, versioned.generation)),
        Err(_) => Ok((serde_json::from_str::<T>(content)?, 0)),
    }
}

pub(super) fn read_versioned_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredVersionedLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let primary =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()));
    match primary.and_then(|content| {
        parse_versioned_json_or_raw::<T>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }) {
        Ok((value, generation)) => Ok(RecoveredVersionedLoad {
            value,
            generation,
            recovered_from_backup: false,
        }),
        Err(primary_err) => {
            let backup_content = fs::read_to_string(backup_path)
                .with_context(|| format!("failed to read {}", backup_path.display()))?;
            let (value, generation) = parse_versioned_json_or_raw::<T>(&backup_content)
                .with_context(|| {
                    format!(
                        "failed to parse {} after primary load error: {primary_err:#}",
                        backup_path.display()
                    )
                })?;
            Ok(RecoveredVersionedLoad {
                value,
                generation,
                recovered_from_backup: true,
            })
        }
    }
}

pub(super) fn write_versioned_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
    generation: u64,
    value: &T,
) -> Result<()>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let json = serde_json::to_string_pretty(&VersionedJson { generation, value })
        .context("failed to serialize runtime sidecar")?;
    write_json_file_with_backup(path, backup_path, &json, |content| {
        let _: VersionedJson<T> =
            serde_json::from_str(content).context("failed to validate runtime sidecar")?;
        Ok(())
    })
}

pub(super) fn save_versioned_json_file_with_fence<T>(
    path: &Path,
    backup_path: &Path,
    value: &T,
) -> Result<()>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let _lock = acquire_json_file_lock(path)?;
    let cached_generation = runtime_sidecar_cached_generation(path);
    let expected_generation = cached_generation
        .unwrap_or_else(|| runtime_sidecar_generation_from_disk(path, backup_path).unwrap_or(0));
    let current_generation = runtime_sidecar_generation_from_disk(path, backup_path)?;
    if current_generation != expected_generation {
        if current_generation == 0
            && expected_generation > 0
            && cached_generation.is_some()
            && !path.exists()
            && !backup_path.exists()
        {
            forget_runtime_sidecar_generation(path);
            return save_versioned_json_file_with_fence(path, backup_path, value);
        }
        bail!(
            "stale runtime sidecar generation for {} expected={} current={}",
            path.display(),
            expected_generation,
            current_generation
        );
    }
    let next_generation = current_generation.saturating_add(1);
    write_versioned_json_file_with_backup(path, backup_path, next_generation, value)?;
    remember_runtime_sidecar_generation(path, next_generation);
    Ok(())
}

pub(super) fn runtime_sidecar_generation_error_is_stale(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .to_string()
            .contains("stale runtime sidecar generation")
    })
}

pub(super) fn write_json_file_with_backup(
    path: &Path,
    backup_path: &Path,
    json: &str,
    validate: impl Fn(&str) -> Result<()>,
) -> Result<()> {
    let temp_file = unique_state_temp_file_path(path);
    fs::write(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    validate(json).with_context(|| format!("failed to validate staged {}", temp_file.display()))?;
    fs::rename(&temp_file, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    let written = fs::read_to_string(path)
        .with_context(|| format!("failed to re-read {}", path.display()))?;
    validate(&written).with_context(|| format!("failed to validate {}", path.display()))?;
    fs::write(backup_path, &written)
        .with_context(|| format!("failed to refresh {}", backup_path.display()))?;
    Ok(())
}

pub(super) fn load_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let primary =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()));
    match primary.and_then(|content| {
        serde_json::from_str::<T>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }) {
        Ok(value) => Ok(RecoveredLoad {
            value,
            recovered_from_backup: false,
        }),
        Err(primary_err) => {
            let backup_content = fs::read_to_string(backup_path)
                .with_context(|| format!("failed to read {}", backup_path.display()))?;
            let value = serde_json::from_str::<T>(&backup_content).with_context(|| {
                format!(
                    "failed to parse {} after primary load error: {primary_err:#}",
                    backup_path.display()
                )
            })?;
            Ok(RecoveredLoad {
                value,
                recovered_from_backup: true,
            })
        }
    }
}

pub(super) fn unique_state_temp_file_path(state_file: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = STATE_SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = format!(
        "{}.{}.{}.{}.tmp",
        state_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("state.json"),
        std::process::id(),
        nanos,
        sequence
    );

    state_file.with_file_name(file_name)
}

pub(super) fn state_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&paths.state_file)
}

pub(super) fn runtime_scores_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-scores.json")
}

pub(super) fn runtime_usage_snapshots_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-usage-snapshots.json")
}

pub(super) fn runtime_scores_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_scores_file_path(paths))
}

pub(super) fn runtime_usage_snapshots_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_usage_snapshots_file_path(paths))
}

pub(super) fn runtime_backoffs_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-backoffs.json")
}

pub(super) fn runtime_backoffs_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_backoffs_file_path(paths))
}

pub(super) fn write_state_json_atomic(paths: &AppPaths, json: &str) -> Result<()> {
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE") {
        bail!("injected runtime state save failure");
    }
    write_json_file_with_backup(
        &paths.state_file,
        &state_last_good_file_path(paths),
        json,
        |content| {
            let _: AppState =
                serde_json::from_str(content).context("failed to validate prodex state")?;
            Ok(())
        },
    )
}

pub(super) fn compact_runtime_profile_scores(
    mut scores: BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let oldest_allowed = now.saturating_sub(RUNTIME_SCORE_RETENTION_SECONDS);
    scores.retain(|key, value| {
        profiles.contains_key(runtime_profile_score_profile_name(key))
            && value.updated_at >= oldest_allowed
    });
    scores
}

pub(super) fn compact_runtime_usage_snapshots(
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

pub(super) fn compact_runtime_profile_backoffs(
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

pub(super) fn merge_runtime_profile_scores(
    existing: &BTreeMap<String, RuntimeProfileHealth>,
    incoming: &BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let mut merged = existing.clone();
    for (key, value) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| current.updated_at <= value.updated_at);
        if should_replace {
            merged.insert(key.clone(), value.clone());
        }
    }
    compact_runtime_profile_scores(merged, profiles, Local::now().timestamp())
}

pub(super) fn load_runtime_profile_scores(
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

pub(super) fn load_runtime_profile_scores_with_recovery(
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
pub(super) fn save_runtime_profile_scores(
    paths: &AppPaths,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_profile_scores_for_profiles(paths, scores, &profiles)
}

pub(super) fn save_runtime_profile_scores_for_profiles(
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

pub(super) fn merge_runtime_usage_snapshots(
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

pub(super) fn load_runtime_usage_snapshots(
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

pub(super) fn load_runtime_usage_snapshots_with_recovery(
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
pub(super) fn save_runtime_usage_snapshots(
    paths: &AppPaths,
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_usage_snapshots_for_profiles(paths, snapshots, &profiles)
}

pub(super) fn save_runtime_usage_snapshots_for_profiles(
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

pub(super) fn merge_runtime_profile_backoffs(
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
    merged
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    merged.transport_backoff_until.retain(|key, until| {
        runtime_profile_transport_backoff_key_matches_profiles(key, profiles) && *until > now
    });
    merged
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    compact_runtime_profile_backoffs(merged, profiles, now)
}

pub(super) fn load_runtime_profile_backoffs(
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

pub(super) fn load_runtime_profile_backoffs_with_recovery(
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
pub(super) fn save_runtime_profile_backoffs(
    paths: &AppPaths,
    backoffs: &RuntimeProfileBackoffs,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_profile_backoffs_for_profiles(paths, backoffs, &profiles)
}

pub(super) fn save_runtime_profile_backoffs_for_profiles(
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

pub(super) fn runtime_profile_score_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}
