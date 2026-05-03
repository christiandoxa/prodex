use super::*;

mod duplicates;

use self::duplicates::cleanup_duplicate_profiles;
use prodex_core::{runtime_broker_artifact_key, runtime_broker_lease_pid};

pub(crate) use prodex_housekeeping::ProdexCleanupSummary;

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
    prodex_housekeeping::cleanup_existing_files(prodex_cleanup_transient_root_file_paths(paths))
}

pub(crate) fn cleanup_prodex_stale_root_temp_files_at(paths: &AppPaths, now: SystemTime) -> usize {
    prodex_housekeeping::cleanup_prodex_stale_root_temp_files_at(
        paths,
        now,
        PROD_EX_TMP_LOGIN_RETENTION_SECONDS,
        runtime_process_pid_alive,
    )
}

pub(crate) fn collect_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
) -> Vec<String> {
    prodex_housekeeping::collect_orphan_managed_profile_dirs_at(
        paths,
        state,
        now,
        ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS,
    )
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
    prodex_housekeeping::cleanup_orphan_managed_profile_dirs_at(
        paths,
        state,
        now,
        ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS,
        |path| remove_dir_if_exists(path).is_ok(),
    )
}

pub(crate) fn prodex_runtime_log_paths_in_dir(dir: &Path) -> Vec<PathBuf> {
    prodex_housekeeping::prodex_runtime_log_paths_in_dir(dir, RUNTIME_PROXY_LOG_FILE_PREFIX)
}

pub(crate) fn cleanup_runtime_proxy_logs_in_dir(dir: &Path, now: SystemTime) -> usize {
    prodex_housekeeping::cleanup_runtime_proxy_logs_in_dir(
        dir,
        now,
        RUNTIME_PROXY_LOG_RETENTION_SECONDS,
        RUNTIME_PROXY_LOG_RETENTION_COUNT,
        RUNTIME_PROXY_LOG_FILE_PREFIX,
    )
}

pub(crate) fn newest_runtime_proxy_log_in_dir(dir: &Path) -> Option<PathBuf> {
    prodex_housekeeping::newest_runtime_proxy_log_in_dir(dir, RUNTIME_PROXY_LOG_FILE_PREFIX)
}

pub(crate) use prodex_housekeeping::cleanup_runtime_proxy_latest_pointer;

pub(crate) fn cleanup_runtime_proxy_log_housekeeping() {
    let temp_dir = runtime_proxy_log_dir();
    cleanup_runtime_proxy_logs_in_dir(&temp_dir, SystemTime::now());
    cleanup_runtime_proxy_latest_pointer(&runtime_proxy_latest_log_pointer_path());
}

pub(crate) fn cleanup_stale_login_dirs_at(paths: &AppPaths, now: SystemTime) -> usize {
    prodex_housekeeping::cleanup_stale_login_dirs_at(
        paths,
        now,
        PROD_EX_TMP_LOGIN_RETENTION_SECONDS,
        |path| remove_dir_if_exists(path).is_ok(),
    )
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

pub(crate) fn cleanup_prodex_chat_history_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
) -> usize {
    prodex_housekeeping::cleanup_prodex_chat_history_at(
        paths,
        state,
        now,
        PRODEX_CHAT_HISTORY_RETENTION_SECONDS,
    )
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
