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

use crate::{
    JsonFileLock, LAST_GOOD_FILE_SUFFIX, RecoveredVersionedLoad, STATE_SAVE_SEQUENCE,
    StateFileLock, VersionedJson, runtime_take_fault_injection,
};

use crate::{AppPaths, AppState, RecoveredLoad};

static RUNTIME_SIDECAR_GENERATION_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> = OnceLock::new();
const RUNTIME_STORE_JSON_MAX_BYTES: u64 = 64 * 1024 * 1024;

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
    match read_json_file_to_string(path) {
        Ok(content) => runtime_sidecar_generation_from_content(&content).or_else(|primary_err| {
            match read_json_file_to_string(backup_path) {
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
            match read_json_file_to_string(backup_path) {
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

pub(crate) fn parse_versioned_json_or_raw<T>(content: &str) -> Result<(T, u64)>
where
    T: for<'de> Deserialize<'de>,
{
    match serde_json::from_str::<VersionedJson<T>>(content) {
        Ok(versioned) => Ok((versioned.value, versioned.generation)),
        Err(_) => Ok((serde_json::from_str::<T>(content)?, 0)),
    }
}

pub(crate) fn read_versioned_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredVersionedLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let primary = read_json_file_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()));
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
            let backup_content = read_json_file_to_string(backup_path)
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
    let temp_file = unique_state_temp_file_path(path);
    write_private_file(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    if let Err(err) = fs::rename(&temp_file, path) {
        let _ = fs::remove_file(&temp_file);
        return Err(err).with_context(|| format!("failed to replace {}", path.display()));
    }
    Ok(())
}

fn write_private_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = open_private_file(path)?;
    file.write_all(content.as_bytes())
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

pub(crate) fn load_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let primary = read_json_file_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()));
    match primary.and_then(|content| {
        serde_json::from_str::<T>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }) {
        Ok(value) => Ok(RecoveredLoad {
            value,
            recovered_from_backup: false,
        }),
        Err(primary_err) => {
            let backup_content = read_json_file_to_string(backup_path)
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

fn read_json_file_to_string(path: &Path) -> io::Result<String> {
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

    let file = fs::File::open(path)?;
    if !runtime_store_same_file_metadata(&metadata, &file.metadata()?) {
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

#[cfg(unix)]
fn runtime_store_same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn runtime_store_same_file_metadata(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
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
