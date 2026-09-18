//! Filesystem housekeeping helpers.
//!
//! The binary crate owns command orchestration and state persistence. This crate
//! keeps bounded cleanup rules reusable and testable without depending on the
//! runtime proxy hot path.

use prodex_core::{
    AppPaths, login_temp_dir_name_is_owned, owned_root_temp_file_name, root_temp_file_pid,
    should_remove_stale_root_temp_file, system_time_to_unix_seconds,
};
use prodex_state::AppState;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

mod runtime_logs;

pub use runtime_logs::{
    cleanup_runtime_proxy_latest_pointer, cleanup_runtime_proxy_latest_pointer_with_counts,
    cleanup_runtime_proxy_logs_in_dir, cleanup_runtime_proxy_logs_in_dir_with_counts,
    newest_runtime_proxy_log_in_dir, prodex_runtime_log_paths_in_dir,
    prodex_runtime_log_paths_in_dir_with_counts,
};

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
    pub scan_failures: usize,
    pub delete_failures: usize,
}

impl ProdexCleanupSummary {
    #[cfg(test)]
    fn total_removed(self) -> usize {
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
        self.scan_failures += other.scan_failures;
        self.delete_failures += other.delete_failures;
        self
    }

    pub fn failure_count(self) -> usize {
        self.scan_failures + self.delete_failures
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProdexCleanupCounts {
    pub removed: usize,
    pub scan_failures: usize,
    pub delete_failures: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProdexCleanupFailureKind {
    OutsideRoot,
    Io(io::ErrorKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProdexCleanupFailure {
    pub path: PathBuf,
    pub kind: ProdexCleanupFailureKind,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProdexCleanupReport {
    pub removed: usize,
    pub missing: usize,
    pub failures: Vec<ProdexCleanupFailure>,
}

impl ProdexCleanupReport {
    pub fn counts(&self) -> ProdexCleanupCounts {
        ProdexCleanupCounts {
            removed: self.removed,
            scan_failures: 0,
            delete_failures: self.failures.len(),
        }
    }
}

pub fn remove_file_if_exists(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

pub fn cleanup_existing_files_under<I>(root: &Path, paths: I) -> ProdexCleanupReport
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut report = ProdexCleanupReport::default();
    for path in paths {
        if !path_is_contained_without_symlink_parents(root, &path) {
            report.failures.push(ProdexCleanupFailure {
                path,
                kind: ProdexCleanupFailureKind::OutsideRoot,
            });
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => report.removed += 1,
            Err(error) if error.kind() == io::ErrorKind::NotFound => report.missing += 1,
            Err(error) => report.failures.push(ProdexCleanupFailure {
                path,
                kind: ProdexCleanupFailureKind::Io(error.kind()),
            }),
        }
    }
    report
}

pub fn path_is_contained_without_symlink_parents(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(component) = component else {
            return false;
        };
        if components.peek().is_none() {
            return true;
        }
        current.push(component);
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            return false;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return false;
        }
    }
    false
}

pub fn cleanup_prodex_stale_root_temp_files_at_with_counts(
    paths: &AppPaths,
    now: SystemTime,
    retention_seconds: i64,
    pid_alive: impl Fn(u32) -> bool,
) -> ProdexCleanupCounts {
    let entries = match fs::read_dir(&paths.root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ProdexCleanupCounts::default();
        }
        Err(_) => {
            return ProdexCleanupCounts {
                scan_failures: 1,
                ..ProdexCleanupCounts::default()
            };
        }
    };
    let oldest_allowed = system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds;
    let mut counts = ProdexCleanupCounts::default();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                counts.scan_failures += 1;
                continue;
            }
        };
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".tmp") || !owned_root_temp_file_name(name) {
            continue;
        }

        let modified = match entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(system_time_to_unix_seconds)
        {
            Some(modified) => modified,
            None => {
                counts.scan_failures += 1;
                continue;
            }
        };
        let pid_alive = root_temp_file_pid(name).is_some_and(&pid_alive);
        if should_remove_stale_root_temp_file(name, modified, oldest_allowed, pid_alive) {
            match fs::remove_file(&path) {
                Ok(()) => counts.removed += 1,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => counts.delete_failures += 1,
            }
        }
    }

    counts
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

fn orphan_managed_profile_dir_name(
    entry: fs::DirEntry,
    state: &AppState,
    oldest_allowed: i64,
) -> Result<Option<String>, ()> {
    let Some(name) = entry.file_name().to_str().map(str::to_string) else {
        return Ok(None);
    };
    if state.profiles.contains_key(&name) {
        return Ok(None);
    }
    let path = entry.path();
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(None);
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(system_time_to_unix_seconds)
        .ok_or(())?;
    if modified >= oldest_allowed || !runtime_managed_profile_dir_looks_safe_to_audit(&path) {
        return Ok(None);
    }
    Ok(Some(name))
}

pub fn collect_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
) -> Vec<String> {
    collect_orphan_managed_profile_dirs_at_with_counts(paths, state, now, retention_seconds).0
}

pub fn collect_orphan_managed_profile_dirs_at_with_counts(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
) -> (Vec<String>, usize) {
    let oldest_allowed = if retention_seconds <= 0 {
        i64::MAX
    } else {
        system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds
    };
    let entries = match fs::read_dir(&paths.managed_profiles_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return (Vec::new(), 0),
        Err(_) => return (Vec::new(), 1),
    };
    let mut scan_failures = 0usize;
    let mut names = Vec::new();
    for entry in entries {
        match entry
            .map_err(|_| ())
            .and_then(|entry| orphan_managed_profile_dir_name(entry, state, oldest_allowed))
        {
            Ok(Some(name)) => names.push(name),
            Ok(None) => {}
            Err(()) => scan_failures += 1,
        }
    }
    names.sort();
    (names, scan_failures)
}

pub fn cleanup_orphan_managed_profile_dirs_at_with_counts(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
    remove_dir: impl Fn(&Path) -> bool,
) -> ProdexCleanupCounts {
    let (names, scan_failures) =
        collect_orphan_managed_profile_dirs_at_with_counts(paths, state, now, retention_seconds);
    let mut counts = ProdexCleanupCounts {
        scan_failures,
        ..ProdexCleanupCounts::default()
    };
    for name in names {
        if remove_dir(&paths.managed_profiles_root.join(name)) {
            counts.removed += 1;
        } else {
            counts.delete_failures += 1;
        }
    }
    counts
}

pub fn cleanup_stale_login_dirs_at(
    paths: &AppPaths,
    now: SystemTime,
    retention_seconds: i64,
    remove_dir: impl Fn(&Path) -> bool,
) -> usize {
    cleanup_stale_login_dirs_at_with_counts(paths, now, retention_seconds, remove_dir).removed
}

pub fn cleanup_stale_login_dirs_at_with_counts(
    paths: &AppPaths,
    now: SystemTime,
    retention_seconds: i64,
    remove_dir: impl Fn(&Path) -> bool,
) -> ProdexCleanupCounts {
    let entries = match fs::read_dir(&paths.managed_profiles_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ProdexCleanupCounts::default();
        }
        Err(_) => {
            return ProdexCleanupCounts {
                scan_failures: 1,
                ..ProdexCleanupCounts::default()
            };
        }
    };
    let oldest_allowed = system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds;
    let mut counts = ProdexCleanupCounts::default();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                counts.scan_failures += 1;
                continue;
            }
        };
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
            .and_then(system_time_to_unix_seconds);
        let Some(modified) = modified else {
            counts.scan_failures += 1;
            continue;
        };
        if modified < oldest_allowed {
            if remove_dir(&path) {
                counts.removed += 1;
            } else {
                counts.delete_failures += 1;
            }
        }
    }
    counts
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
