use super::*;

mod duplicates;
mod history;

use self::duplicates::cleanup_duplicate_profiles;
use self::history::cleanup_prodex_chat_history_at;
use prodex_core::{
    login_temp_dir_name_is_owned, owned_root_temp_file_name, root_temp_file_pid,
    runtime_broker_artifact_key, runtime_broker_lease_pid, runtime_proxy_log_file_name_is_owned,
    select_newest_modified_path, select_runtime_log_paths_to_remove,
    should_remove_stale_root_temp_file, system_time_to_unix_seconds,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProdexCleanupSummary {
    pub(crate) duplicate_profiles_removed: usize,
    pub(crate) duplicate_managed_profile_homes_removed: usize,
    pub(crate) runtime_logs_removed: usize,
    pub(crate) stale_runtime_log_pointer_removed: usize,
    pub(crate) stale_login_dirs_removed: usize,
    pub(crate) orphan_managed_profile_dirs_removed: usize,
    pub(crate) transient_root_files_removed: usize,
    pub(crate) stale_root_temp_files_removed: usize,
    pub(crate) chat_history_entries_removed: usize,
    pub(crate) dead_runtime_broker_leases_removed: usize,
    pub(crate) dead_runtime_broker_registries_removed: usize,
}

impl ProdexCleanupSummary {
    pub(crate) fn total_removed(self) -> usize {
        self.duplicate_profiles_removed
            + self.duplicate_managed_profile_homes_removed
            + self.runtime_logs_removed
            + self.stale_runtime_log_pointer_removed
            + self.stale_login_dirs_removed
            + self.orphan_managed_profile_dirs_removed
            + self.transient_root_files_removed
            + self.stale_root_temp_files_removed
            + self.chat_history_entries_removed
            + self.dead_runtime_broker_leases_removed
            + self.dead_runtime_broker_registries_removed
    }

    fn merge(mut self, other: Self) -> Self {
        self.duplicate_profiles_removed += other.duplicate_profiles_removed;
        self.duplicate_managed_profile_homes_removed +=
            other.duplicate_managed_profile_homes_removed;
        self.runtime_logs_removed += other.runtime_logs_removed;
        self.stale_runtime_log_pointer_removed += other.stale_runtime_log_pointer_removed;
        self.stale_login_dirs_removed += other.stale_login_dirs_removed;
        self.orphan_managed_profile_dirs_removed += other.orphan_managed_profile_dirs_removed;
        self.transient_root_files_removed += other.transient_root_files_removed;
        self.stale_root_temp_files_removed += other.stale_root_temp_files_removed;
        self.chat_history_entries_removed += other.chat_history_entries_removed;
        self.dead_runtime_broker_leases_removed += other.dead_runtime_broker_leases_removed;
        self.dead_runtime_broker_registries_removed += other.dead_runtime_broker_registries_removed;
        self
    }
}

fn remove_file_if_exists(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

fn prodex_cleanup_transient_root_file_paths(paths: &AppPaths) -> Vec<PathBuf> {
    vec![
        runtime_scores_file_path(paths),
        runtime_scores_last_good_file_path(paths),
        runtime_usage_snapshots_file_path(paths),
        runtime_usage_snapshots_last_good_file_path(paths),
        runtime_backoffs_file_path(paths),
        runtime_backoffs_last_good_file_path(paths),
        update_check_cache_file_path(paths),
    ]
}

pub(crate) fn cleanup_prodex_transient_root_files(paths: &AppPaths) -> usize {
    prodex_cleanup_transient_root_file_paths(paths)
        .into_iter()
        .filter(|path| remove_file_if_exists(path))
        .count()
}

pub(crate) fn cleanup_prodex_stale_root_temp_files_at(paths: &AppPaths, now: SystemTime) -> usize {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return 0;
    };
    let oldest_allowed =
        system_time_to_unix_seconds(now).unwrap_or_default() - PROD_EX_TMP_LOGIN_RETENTION_SECONDS;
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
        let pid_alive = root_temp_file_pid(name).is_some_and(runtime_process_pid_alive);
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

pub(crate) fn collect_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
) -> Vec<String> {
    let oldest_allowed = system_time_to_unix_seconds(now).unwrap_or_default()
        - ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS;
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

pub(crate) fn collect_orphan_managed_profile_dirs(
    paths: &AppPaths,
    state: &AppState,
) -> Vec<String> {
    collect_orphan_managed_profile_dirs_at(paths, state, SystemTime::now())
}

pub(crate) fn cleanup_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
) -> usize {
    let mut removed = 0usize;
    for name in collect_orphan_managed_profile_dirs_at(paths, state, now) {
        if remove_dir_if_exists(&paths.managed_profiles_root.join(name)).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub(crate) fn prodex_runtime_log_paths_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|entry| entry.ok().map(|item| item.path())))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    runtime_proxy_log_file_name_is_owned(name, RUNTIME_PROXY_LOG_FILE_PREFIX)
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub(crate) fn cleanup_runtime_proxy_logs_in_dir(dir: &Path, now: SystemTime) -> usize {
    let now_epoch = system_time_to_unix_seconds(now).unwrap_or_default();
    let oldest_allowed = now_epoch.saturating_sub(RUNTIME_PROXY_LOG_RETENTION_SECONDS);
    let paths = prodex_runtime_log_paths_in_dir(dir)
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
    for path in
        select_runtime_log_paths_to_remove(paths, oldest_allowed, RUNTIME_PROXY_LOG_RETENTION_COUNT)
    {
        if fs::remove_file(path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub(crate) fn newest_runtime_proxy_log_in_dir(dir: &Path) -> Option<PathBuf> {
    let paths = prodex_runtime_log_paths_in_dir(dir)
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

pub(crate) fn cleanup_runtime_proxy_latest_pointer(pointer_path: &Path) -> bool {
    let should_remove_pointer = fs::read_to_string(pointer_path)
        .ok()
        .map(|content| PathBuf::from(content.trim()))
        .is_some_and(|path| !path.exists());
    if should_remove_pointer {
        return fs::remove_file(pointer_path).is_ok();
    }
    false
}

pub(crate) fn cleanup_runtime_proxy_log_housekeeping() {
    let temp_dir = runtime_proxy_log_dir();
    cleanup_runtime_proxy_logs_in_dir(&temp_dir, SystemTime::now());
    cleanup_runtime_proxy_latest_pointer(&runtime_proxy_latest_log_pointer_path());
}

pub(crate) fn cleanup_stale_login_dirs_at(paths: &AppPaths, now: SystemTime) -> usize {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return 0;
    };
    let oldest_allowed =
        system_time_to_unix_seconds(now).unwrap_or_default() - PROD_EX_TMP_LOGIN_RETENTION_SECONDS;
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
        if modified < oldest_allowed && remove_dir_if_exists(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub(crate) fn cleanup_stale_login_dirs(paths: &AppPaths) -> usize {
    cleanup_stale_login_dirs_at(paths, SystemTime::now())
}

fn runtime_broker_artifact_keys(paths: &AppPaths) -> Vec<String> {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(key) = runtime_broker_artifact_key(name, path.is_dir()) {
            keys.push(key.to_string());
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

pub(crate) fn cleanup_runtime_broker_stale_registries(paths: &AppPaths) -> Result<usize> {
    let mut removed = 0usize;
    for broker_key in runtime_broker_artifact_keys(paths) {
        let Some(registry) = load_runtime_broker_registry(paths, &broker_key)? else {
            continue;
        };
        if runtime_process_pid_alive(registry.pid) {
            continue;
        }
        remove_runtime_broker_registry_if_token_matches(
            paths,
            &broker_key,
            &registry.instance_token,
        );
        removed += 1;
    }
    Ok(removed)
}

pub(crate) fn cleanup_runtime_broker_stale_leases_for_all(paths: &AppPaths) -> usize {
    let mut removed = 0usize;
    for broker_key in runtime_broker_artifact_keys(paths) {
        let lease_dir = runtime_broker_lease_dir(paths, &broker_key);
        let Ok(entries) = fs::read_dir(&lease_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let pid = runtime_broker_lease_pid(file_name);
            if pid.is_some_and(runtime_process_pid_alive) {
                continue;
            }
            if fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        let should_remove_dir = fs::read_dir(&lease_dir)
            .ok()
            .is_some_and(|mut remaining| remaining.next().is_none());
        if should_remove_dir {
            let _ = fs::remove_dir(&lease_dir);
        }
    }
    removed
}

pub(crate) fn perform_prodex_cleanup_at(
    paths: &AppPaths,
    state: &AppState,
    runtime_log_dir: &Path,
    runtime_log_pointer_path: &Path,
    now: SystemTime,
) -> Result<ProdexCleanupSummary> {
    Ok(ProdexCleanupSummary {
        duplicate_profiles_removed: 0,
        duplicate_managed_profile_homes_removed: 0,
        runtime_logs_removed: cleanup_runtime_proxy_logs_in_dir(runtime_log_dir, now),
        stale_runtime_log_pointer_removed: usize::from(cleanup_runtime_proxy_latest_pointer(
            runtime_log_pointer_path,
        )),
        stale_login_dirs_removed: cleanup_stale_login_dirs_at(paths, now),
        orphan_managed_profile_dirs_removed: cleanup_orphan_managed_profile_dirs_at(
            paths, state, now,
        ),
        transient_root_files_removed: cleanup_prodex_transient_root_files(paths),
        stale_root_temp_files_removed: cleanup_prodex_stale_root_temp_files_at(paths, now),
        chat_history_entries_removed: cleanup_prodex_chat_history_at(paths, state, now),
        dead_runtime_broker_leases_removed: cleanup_runtime_broker_stale_leases_for_all(paths),
        dead_runtime_broker_registries_removed: cleanup_runtime_broker_stale_registries(paths)?,
    })
}

pub(crate) fn perform_prodex_cleanup(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<ProdexCleanupSummary> {
    let duplicate_summary = cleanup_duplicate_profiles(paths, state)?;
    let artifact_summary = perform_prodex_cleanup_at(
        paths,
        state,
        &runtime_proxy_log_dir(),
        &runtime_proxy_latest_log_pointer_path(),
        SystemTime::now(),
    )?;
    Ok(duplicate_summary.merge(artifact_summary))
}
