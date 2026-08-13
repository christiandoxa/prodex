use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use crate::runtime_take_fault_injection_budget;
use crate::{
    JsonFileLock, LAST_GOOD_FILE_SUFFIX, RecoveredVersionedLoad, STATE_SAVE_SEQUENCE,
    StateFileLock, VersionedJson, runtime_take_fault_injection,
};

use crate::{AppPaths, AppState, RecoveredLoad};

static RUNTIME_SIDECAR_GENERATION_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> = OnceLock::new();
pub(crate) const RUNTIME_STORE_JSON_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[cfg(test)]
const TEST_RUNTIME_STORE_WRITE_FAULT_ENV: &str =
    "PRODEX_RUNTIME_FAULT_RUNTIME_STORE_WRITE_ERROR_ONCE";
#[cfg(test)]
const TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV: &str =
    "PRODEX_RUNTIME_FAULT_RUNTIME_STORE_PRIMARY_RENAME_ERROR_ONCE";
#[cfg(test)]
const TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV: &str =
    "PRODEX_RUNTIME_FAULT_RUNTIME_STORE_SIDECAR_RENAME_ERROR_ONCE";

pub(crate) fn acquire_state_file_lock(paths: &AppPaths) -> Result<StateFileLock> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let lock_path = state_lock_file_path(&paths.state_file);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to lock {}", lock_path.display()))?;
    Ok(StateFileLock { file })
}

pub(crate) fn try_acquire_runtime_owner_lock(paths: &AppPaths) -> Result<Option<StateFileLock>> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let lock_path = runtime_owner_lock_file_path(paths);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
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

pub(crate) fn state_lock_file_path(state_file: &Path) -> PathBuf {
    json_lock_file_path(state_file)
}

pub(crate) fn runtime_owner_lock_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-owner.lock")
}

pub(crate) fn json_lock_file_path(path: &Path) -> PathBuf {
    path.with_extension("json.lock")
}

pub(crate) fn acquire_json_file_lock(path: &Path) -> Result<JsonFileLock> {
    let lock_path = json_lock_file_path(path);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
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

pub(crate) fn remember_runtime_sidecar_generation(path: &Path, generation: u64) {
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

pub(crate) fn last_good_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot.json");
    path.with_file_name(format!("{file_name}{LAST_GOOD_FILE_SUFFIX}"))
}

pub(crate) fn runtime_sidecar_generation_from_content(content: &str) -> Result<u64> {
    let value: serde_json::Value =
        serde_json::from_str(content).context("failed to parse runtime sidecar json")?;
    Ok(value
        .get("generation")
        .and_then(|value| value.as_u64())
        .unwrap_or(0))
}

pub(crate) fn runtime_sidecar_generation_from_disk(path: &Path, backup_path: &Path) -> Result<u64> {
    let primary_exists = path
        .try_exists()
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    let backup_exists = backup_path
        .try_exists()
        .with_context(|| format!("failed to inspect {}", backup_path.display()))?;
    if !primary_exists && !backup_exists {
        return Ok(0);
    }
    let loaded = read_json_file_with_backup_unlocked(path, backup_path, |content| {
        runtime_sidecar_generation_from_content(content)
    })?;
    Ok(loaded.value)
}

pub(crate) fn parse_versioned_json_or_raw<T>(content: &str) -> Result<(T, u64)>
where
    T: for<'de> Deserialize<'de>,
{
    match serde_json::from_str::<VersionedJson<T>>(content) {
        Ok(versioned) => Ok((versioned.value, versioned.generation)),
        Err(_) => Ok((serde_json::from_str::<T>(content)?, 0)),
    }
}

fn read_json_file_with_backup_unlocked<T>(
    path: &Path,
    backup_path: &Path,
    parse: impl Fn(&str) -> Result<T>,
) -> Result<RecoveredLoad<T>> {
    let primary = read_json_file_primary(path, &parse);
    match primary {
        Ok(value) => Ok(RecoveredLoad {
            value,
            recovered_from_backup: false,
        }),
        Err(primary_err) => {
            let backup_content = read_json_file_to_string(backup_path)
                .with_context(|| format!("failed to read {}", backup_path.display()))?;
            let value = parse(&backup_content).with_context(|| {
                format!(
                    "failed to parse {} after primary load error: {primary_err:#}",
                    backup_path.display()
                )
            })?;

            // Keep valid backup usable if best-effort primary repair hits a filesystem fault.
            let _ = write_private_file_atomic(path, backup_content.as_bytes());
            Ok(RecoveredLoad {
                value,
                recovered_from_backup: true,
            })
        }
    }
}

fn read_json_file_primary<T>(path: &Path, parse: &impl Fn(&str) -> Result<T>) -> Result<T> {
    read_json_file_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))
        .and_then(|content| {
            parse(&content).with_context(|| format!("failed to parse {}", path.display()))
        })
}

fn read_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
    parse: impl Fn(&str) -> Result<T>,
) -> Result<RecoveredLoad<T>> {
    if let Ok(value) = read_json_file_primary(path, &parse) {
        return Ok(RecoveredLoad {
            value,
            recovered_from_backup: false,
        });
    }
    let _lock = acquire_json_file_lock(path)?;
    read_json_file_with_backup_unlocked(path, backup_path, parse)
}

pub(crate) fn read_versioned_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredVersionedLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let loaded = read_json_file_with_backup(path, backup_path, |content| {
        parse_versioned_json_or_raw::<T>(content)
    })?;
    let (value, generation) = loaded.value;
    Ok(RecoveredVersionedLoad {
        value,
        generation,
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

pub(crate) fn write_versioned_json_file_with_backup<T>(
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

pub(crate) fn save_versioned_json_file_with_fence<T>(
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

pub(crate) fn runtime_sidecar_generation_error_is_stale(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .to_string()
            .contains("stale runtime sidecar generation")
    })
}

pub(crate) fn write_json_file_with_backup(
    path: &Path,
    backup_path: &Path,
    json: &str,
    validate: impl Fn(&str) -> Result<()>,
) -> Result<()> {
    validate(json).with_context(|| format!("failed to validate staged {}", path.display()))?;
    if json.len() as u64 > RUNTIME_STORE_JSON_MAX_BYTES {
        bail!(
            "runtime store json {} exceeds safe size limit ({} bytes)",
            path.display(),
            RUNTIME_STORE_JSON_MAX_BYTES
        );
    }
    write_json_file_atomic_private(path, json)?;
    let written = read_json_file_to_string(path)
        .with_context(|| format!("failed to re-read {}", path.display()))?;
    validate(&written).with_context(|| format!("failed to validate {}", path.display()))?;
    write_json_file_atomic_private(backup_path, &written)
        .with_context(|| format!("failed to refresh {}", backup_path.display()))?;
    Ok(())
}

fn write_json_file_atomic_private(path: &Path, json: &str) -> Result<()> {
    write_private_file_atomic(path, json.as_bytes())
}

pub(crate) fn write_private_file_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(test)]
    if let Err(error) = maybe_inject_runtime_store_write_failure() {
        return Err(error)
            .with_context(|| format!("failed to atomically write {}", path.display()));
    }

    let temp_file = unique_state_temp_file_path(path);
    let result = (|| -> io::Result<()> {
        let mut file = open_private_file(&temp_file)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file_atomic(&temp_file, path)?;
        sync_parent_directory(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_file);
    }
    result.with_context(|| format!("failed to atomically write {}", path.display()))
}

#[cfg(windows)]
pub(crate) fn replace_file_atomic(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(test)]
    maybe_inject_runtime_store_rename_failure(to)?;

    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let from = from
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let to = to
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are NUL-terminated Windows paths and remain live
    // for the call.
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn replace_file_atomic(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(test)]
    maybe_inject_runtime_store_rename_failure(to)?;

    fs::rename(from, to)
}

#[cfg(test)]
fn maybe_inject_runtime_store_write_failure() -> Result<()> {
    if runtime_take_fault_injection(TEST_RUNTIME_STORE_WRITE_FAULT_ENV) {
        bail!("injected runtime-store atomic write failure");
    }
    Ok(())
}

#[cfg(test)]
fn maybe_inject_runtime_store_rename_failure(path: &Path) -> io::Result<()> {
    let is_sidecar = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(LAST_GOOD_FILE_SUFFIX));
    let env_key = if is_sidecar {
        TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV
    } else {
        TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV
    };
    if runtime_take_fault_injection(env_key) {
        return Err(io::Error::other(
            "injected runtime-store atomic rename failure",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    fs::File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn open_private_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

pub(crate) fn load_json_file_with_backup_unlocked<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    read_json_file_with_backup_unlocked(path, backup_path, |content| {
        Ok(serde_json::from_str::<T>(content)?)
    })
}

pub(crate) fn load_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    read_json_file_with_backup(path, backup_path, |content| {
        Ok(serde_json::from_str::<T>(content)?)
    })
}

pub(crate) fn read_json_file_to_string(path: &Path) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "refusing to read json through symlink {}",
            path.display()
        )));
    }
    if !metadata.file_type().is_file() {
        return Err(io::Error::other(format!(
            "json path {} is not a file",
            path.display()
        )));
    }
    if metadata.len() > RUNTIME_STORE_JSON_MAX_BYTES {
        return Err(io::Error::other(format!(
            "json path {} exceeds safe size limit ({} bytes)",
            path.display(),
            RUNTIME_STORE_JSON_MAX_BYTES
        )));
    }

    let file = prodex_core::open_regular_file_no_follow(path)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)? {
        return Err(io::Error::other(format!(
            "json path changed while opening {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.take(RUNTIME_STORE_JSON_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > RUNTIME_STORE_JSON_MAX_BYTES {
        return Err(io::Error::other(format!(
            "json path {} exceeds safe size limit ({} bytes)",
            path.display(),
            RUNTIME_STORE_JSON_MAX_BYTES
        )));
    }
    String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

pub(crate) fn unique_state_temp_file_path(state_file: &Path) -> PathBuf {
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

pub(crate) fn state_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&paths.state_file)
}

pub(crate) fn runtime_scores_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-scores.json")
}

pub(crate) fn runtime_usage_snapshots_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-usage-snapshots.json")
}

pub(crate) fn runtime_scores_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_scores_file_path(paths))
}

pub(crate) fn runtime_usage_snapshots_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_usage_snapshots_file_path(paths))
}

pub(crate) fn runtime_backoffs_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-backoffs.json")
}

pub(crate) fn runtime_backoffs_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_backoffs_file_path(paths))
}

pub(crate) fn write_state_json_atomic(paths: &AppPaths, json: &str) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestEnvVarGuard;

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-store-{name}-{}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test root should be created");
        root
    }

    #[test]
    fn sidecar_generation_recovery_repairs_corrupt_primary() {
        let root = temp_root("generation-recovery");
        let path = root.join("runtime.json");
        let backup_path = root.join("runtime.json.last-good");
        let backup = r#"{"generation":7,"value":{"source":"last-good"}}"#;
        fs::write(&path, "{broken").expect("primary should be writable");
        fs::write(&backup_path, backup).expect("backup should be writable");

        assert_eq!(
            runtime_sidecar_generation_from_disk(&path, &backup_path).unwrap(),
            7
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), backup);
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
        let _ = fs::remove_dir_all(root);
    }

    fn reset_runtime_store_fault_budget(env_key: &'static str) {
        let _env_lock = TestEnvVarGuard::lock();
        let _ = runtime_take_fault_injection_budget(env_key, 0);
    }

    fn assert_last_good_recovery_keeps_backup_on_fault(
        env_key: &'static str,
        primary: Option<&str>,
    ) {
        reset_runtime_store_fault_budget(env_key);
        let root = temp_root("recovery-fault");
        let path = root.join("state.json");
        let backup_path = root.join("state.last-good.json");
        let backup = r#"{"source":"last-good"}"#;
        if let Some(primary) = primary {
            fs::write(&path, primary).expect("primary should be writable");
        }
        fs::write(&backup_path, backup).expect("backup should be writable");

        let loaded = {
            let _fault = TestEnvVarGuard::set(env_key, "1");
            load_json_file_with_backup::<serde_json::Value>(&path, &backup_path)
                .expect("valid backup should remain usable")
        };

        assert!(loaded.recovered_from_backup);
        assert_eq!(loaded.value["source"], "last-good");
        match primary {
            Some(primary) => assert_eq!(fs::read_to_string(&path).unwrap(), primary),
            None => assert!(!path.exists()),
        }
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
        reset_runtime_store_fault_budget(env_key);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn last_good_recovery_keeps_backup_when_primary_write_fails() {
        assert_last_good_recovery_keeps_backup_on_fault(
            TEST_RUNTIME_STORE_WRITE_FAULT_ENV,
            Some("{broken"),
        );
    }

    #[test]
    fn last_good_recovery_keeps_backup_when_primary_rename_fails() {
        assert_last_good_recovery_keeps_backup_on_fault(
            TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV,
            Some("{broken"),
        );
    }

    #[test]
    fn last_good_recovery_keeps_backup_when_primary_is_missing_and_repair_fails() {
        for env_key in [
            TEST_RUNTIME_STORE_WRITE_FAULT_ENV,
            TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV,
        ] {
            assert_last_good_recovery_keeps_backup_on_fault(env_key, None);
        }
    }

    #[test]
    fn sidecar_migration_keeps_last_good_when_missing_primary_repair_fails() {
        for env_key in [
            TEST_RUNTIME_STORE_WRITE_FAULT_ENV,
            TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV,
        ] {
            reset_runtime_store_fault_budget(env_key);
            let root = temp_root("generation-recovery-fault");
            let path = root.join("runtime.json");
            let backup_path = root.join("runtime.json.last-good");
            let backup = r#"{"generation":7,"value":{"source":"last-good"}}"#;
            fs::write(&backup_path, backup).expect("backup should be writable");
            let generation = {
                let _fault = TestEnvVarGuard::set(env_key, "1");
                runtime_sidecar_generation_from_disk(&path, &backup_path)
                    .expect("valid last-good generation should remain usable")
            };

            assert_eq!(generation, 7);
            assert!(!path.exists());
            assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
            reset_runtime_store_fault_budget(env_key);
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn write_json_file_with_backup_keeps_backup_when_primary_write_fails() {
        reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_WRITE_FAULT_ENV);
        let root = temp_root("primary-write-fault");
        let path = root.join("state.json");
        let backup_path = root.join("state.last-good.json");
        let backup = r#"{"source":"last-good"}"#;
        fs::write(&backup_path, backup).expect("backup should be writable");
        let err = {
            let _fault = TestEnvVarGuard::set(TEST_RUNTIME_STORE_WRITE_FAULT_ENV, "1");
            write_json_file_with_backup(&path, &backup_path, r#"{"source":"new"}"#, |content| {
                let _: serde_json::Value = serde_json::from_str(content)?;
                Ok(())
            })
            .expect_err("injected primary write should fail")
        };

        assert!(err.to_string().contains("atomically write"));
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), backup);
        reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_WRITE_FAULT_ENV);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_json_file_with_backup_keeps_backup_when_primary_rename_fails() {
        reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV);
        let root = temp_root("primary-rename-fault");
        let path = root.join("state.json");
        let backup_path = root.join("state.last-good.json");
        let old = r#"{"source":"old"}"#;
        let new = r#"{"source":"new"}"#;
        fs::write(&path, old).expect("primary should be writable");
        fs::write(&backup_path, old).expect("backup should be writable");
        let err = {
            let _fault = TestEnvVarGuard::set(TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV, "1");
            write_json_file_with_backup(&path, &backup_path, new, |content| {
                let _: serde_json::Value = serde_json::from_str(content)?;
                Ok(())
            })
            .expect_err("injected primary rename should fail")
        };

        assert!(err.to_string().contains("atomically write"));
        assert_eq!(fs::read_to_string(&path).unwrap(), old);
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), old);
        reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_PRIMARY_RENAME_FAULT_ENV);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_json_file_with_backup_updates_backup_only_after_primary_is_durable() {
        reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV);
        let root = temp_root("sidecar-rename-fault");
        let path = root.join("state.json");
        let backup_path = root.join("state.last-good.json");
        let old = r#"{"source":"old"}"#;
        let new = r#"{"source":"new"}"#;
        fs::write(&path, old).expect("primary should be writable");
        fs::write(&backup_path, old).expect("backup should be writable");
        let err = {
            let _fault = TestEnvVarGuard::set(TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV, "1");
            write_json_file_with_backup(&path, &backup_path, new, |content| {
                let _: serde_json::Value = serde_json::from_str(content)?;
                Ok(())
            })
            .expect_err("injected sidecar rename should fail")
        };

        assert!(err.to_string().contains("refresh"));
        assert_eq!(fs::read_to_string(&path).unwrap(), new);
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), old);
        reset_runtime_store_fault_budget(TEST_RUNTIME_STORE_SIDECAR_RENAME_FAULT_ENV);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn write_json_file_with_backup_restricts_primary_and_backup_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("permissions");
        let path = root.join("state.json");
        let backup_path = root.join("state.last-good.json");

        write_json_file_with_backup(&path, &backup_path, r#"{"ok":true}"#, |content| {
            let _: serde_json::Value =
                serde_json::from_str(content).context("json should parse")?;
            Ok(())
        })
        .expect("json should be written");

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&backup_path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_json_file_to_string_rejects_oversized_file_before_reading() {
        let root = temp_root("oversized-read");
        let path = root.join("state.json");
        fs::File::create(&path)
            .expect("state file should be created")
            .set_len(RUNTIME_STORE_JSON_MAX_BYTES + 1)
            .expect("state file size should be set");

        let err = read_json_file_to_string(&path).expect_err("oversized state should be rejected");

        assert!(err.to_string().contains("exceeds safe size limit"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn read_json_file_to_string_rejects_symlink() {
        let root = temp_root("symlink-read");
        let target = root.join("target.json");
        let link = root.join("state.json");
        fs::write(&target, "{}").expect("target should write");
        std::os::unix::fs::symlink(&target, &link).expect("symlink should be created");

        let err = read_json_file_to_string(&link).expect_err("symlink should be rejected");

        assert!(
            err.to_string()
                .contains("refusing to read json through symlink")
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
#[path = "io_tests.rs"]
mod lock_tests;
