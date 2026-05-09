//! Filesystem housekeeping helpers.
//!
//! The binary crate owns command orchestration and state persistence. This crate
//! keeps bounded cleanup rules reusable and testable without depending on the
//! runtime proxy hot path.

use prodex_core::{
    AppPaths, login_temp_dir_name_is_owned, owned_root_temp_file_name, root_temp_file_pid,
    runtime_proxy_log_file_name_is_owned, select_newest_modified_path,
    select_runtime_log_paths_to_remove, should_remove_stale_root_temp_file,
    system_time_to_unix_seconds,
};
use prodex_state::AppState;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProdexCleanupSummary {
    pub duplicate_profiles_removed: usize,
    pub duplicate_managed_profile_homes_removed: usize,
    pub runtime_logs_removed: usize,
    pub stale_runtime_log_pointer_removed: usize,
    pub stale_login_dirs_removed: usize,
    pub orphan_managed_profile_dirs_removed: usize,
    pub transient_root_files_removed: usize,
    pub stale_root_temp_files_removed: usize,
    pub dead_runtime_broker_leases_removed: usize,
    pub dead_runtime_broker_registries_removed: usize,
}

impl ProdexCleanupSummary {
    pub fn total_removed(self) -> usize {
        self.duplicate_profiles_removed
            + self.duplicate_managed_profile_homes_removed
            + self.runtime_logs_removed
            + self.stale_runtime_log_pointer_removed
            + self.stale_login_dirs_removed
            + self.orphan_managed_profile_dirs_removed
            + self.transient_root_files_removed
            + self.stale_root_temp_files_removed
            + self.dead_runtime_broker_leases_removed
            + self.dead_runtime_broker_registries_removed
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.duplicate_profiles_removed += other.duplicate_profiles_removed;
        self.duplicate_managed_profile_homes_removed +=
            other.duplicate_managed_profile_homes_removed;
        self.runtime_logs_removed += other.runtime_logs_removed;
        self.stale_runtime_log_pointer_removed += other.stale_runtime_log_pointer_removed;
        self.stale_login_dirs_removed += other.stale_login_dirs_removed;
        self.orphan_managed_profile_dirs_removed += other.orphan_managed_profile_dirs_removed;
        self.transient_root_files_removed += other.transient_root_files_removed;
        self.stale_root_temp_files_removed += other.stale_root_temp_files_removed;
        self.dead_runtime_broker_leases_removed += other.dead_runtime_broker_leases_removed;
        self.dead_runtime_broker_registries_removed += other.dead_runtime_broker_registries_removed;
        self
    }
}

pub fn remove_file_if_exists(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

pub fn cleanup_existing_files<I>(paths: I) -> usize
where
    I: IntoIterator<Item = PathBuf>,
{
    paths
        .into_iter()
        .filter(|path| remove_file_if_exists(path))
        .count()
}

pub fn cleanup_prodex_stale_root_temp_files_at(
    paths: &AppPaths,
    now: SystemTime,
    retention_seconds: i64,
    pid_alive: impl Fn(u32) -> bool,
) -> usize {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return 0;
    };
    let oldest_allowed = system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds;
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".tmp") || !owned_root_temp_file_name(name) {
            continue;
        }

        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(system_time_to_unix_seconds)
            .unwrap_or(i64::MIN);
        let pid_alive = root_temp_file_pid(name).is_some_and(&pid_alive);
        if should_remove_stale_root_temp_file(name, modified, oldest_allowed, pid_alive)
            && remove_file_if_exists(&path)
        {
            removed += 1;
        }
    }

    removed
}

fn runtime_managed_profile_dir_looks_safe_to_audit(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    path.join("auth.json").exists()
        || path.join("config.toml").exists()
        || path.join("state.json").exists()
        || path.join(".codex").exists()
}

pub fn collect_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
) -> Vec<String> {
    let oldest_allowed = if retention_seconds <= 0 {
        i64::MAX
    } else {
        system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds
    };
    let Ok(entries) = fs::read_dir(&paths.managed_profiles_root) else {
        return Vec::new();
    };
    let mut names = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_string();
            if state.profiles.contains_key(&name) {
                return None;
            }
            let metadata = fs::symlink_metadata(&path).ok()?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return None;
            }
            let modified = metadata
                .modified()
                .ok()
                .and_then(system_time_to_unix_seconds)
                .unwrap_or(i64::MIN);
            if modified >= oldest_allowed || !runtime_managed_profile_dir_looks_safe_to_audit(&path)
            {
                return None;
            }
            Some(name)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

pub fn cleanup_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
    remove_dir: impl Fn(&Path) -> bool,
) -> usize {
    let mut removed = 0usize;
    for name in collect_orphan_managed_profile_dirs_at(paths, state, now, retention_seconds) {
        if remove_dir(&paths.managed_profiles_root.join(name)) {
            removed += 1;
        }
    }
    removed
}

pub fn prodex_runtime_log_paths_in_dir(dir: &Path, log_prefix: &str) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|entry| entry.ok().map(|item| item.path())))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| runtime_proxy_log_file_name_is_owned(name, log_prefix))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn cleanup_runtime_proxy_logs_in_dir(
    dir: &Path,
    now: SystemTime,
    retention_seconds: i64,
    retention_count: usize,
    log_prefix: &str,
) -> usize {
    let now_epoch = system_time_to_unix_seconds(now).unwrap_or_default();
    let oldest_allowed = now_epoch.saturating_sub(retention_seconds);
    let paths = prodex_runtime_log_paths_in_dir(dir, log_prefix)
        .into_iter()
        .map(|path| {
            let modified = path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(system_time_to_unix_seconds)
                .unwrap_or(i64::MIN);
            (path, modified)
        })
        .collect::<Vec<_>>();
    let mut removed = 0usize;
    for path in select_runtime_log_paths_to_remove(paths, oldest_allowed, retention_count) {
        if fs::remove_file(path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub fn newest_runtime_proxy_log_in_dir(dir: &Path, log_prefix: &str) -> Option<PathBuf> {
    let paths = prodex_runtime_log_paths_in_dir(dir, log_prefix)
        .into_iter()
        .filter_map(|path| {
            let modified = path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis());
            modified.map(|modified| (modified, path))
        })
        .collect::<Vec<_>>();
    select_newest_modified_path(paths)
}

pub fn cleanup_runtime_proxy_latest_pointer(pointer_path: &Path) -> bool {
    let should_remove_pointer = fs::read_to_string(pointer_path)
        .ok()
        .map(|content| PathBuf::from(content.trim()))
        .is_some_and(|path| !path.exists());
    if should_remove_pointer {
        return fs::remove_file(pointer_path).is_ok();
    }
    false
}

pub fn cleanup_stale_login_dirs_at(
    paths: &AppPaths,
    now: SystemTime,
    retention_seconds: i64,
    remove_dir: impl Fn(&Path) -> bool,
) -> usize {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return 0;
    };
    let oldest_allowed = system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds;
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !login_temp_dir_name_is_owned(name) {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(system_time_to_unix_seconds)
            .unwrap_or(i64::MIN);
        if modified < oldest_allowed && remove_dir(&path) {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
