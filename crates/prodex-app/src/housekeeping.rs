use super::*;

mod duplicates;

use self::duplicates::cleanup_duplicate_profiles;
use fs2::FileExt;
use prodex_core::{runtime_broker_artifact_key, runtime_broker_lease_pid};

pub(crate) use prodex_housekeeping::ProdexCleanupSummary;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProdexCleanupOptions {
    pub(crate) orphan_managed_profile_retention_seconds: i64,
}

impl Default for ProdexCleanupOptions {
    fn default() -> Self {
        Self {
            orphan_managed_profile_retention_seconds:
                ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS,
        }
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

pub(crate) fn cleanup_orphan_managed_profile_dirs_with_retention_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
) -> usize {
    prodex_housekeeping::cleanup_orphan_managed_profile_dirs_at(
        paths,
        state,
        now,
        retention_seconds,
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

pub(crate) fn command_runs_auto_runtime_housekeeping(command: &Commands) -> bool {
    !matches!(
        command,
        Commands::Cleanup(_) | Commands::RuntimeBroker(_) | Commands::Update(_)
    )
}

pub(crate) fn schedule_prodex_auto_runtime_housekeeping(command: &Commands) {
    if !command_runs_auto_runtime_housekeeping(command) {
        return;
    }
    let _ = thread::Builder::new()
        .name("prodex-housekeeping".to_string())
        .spawn(|| {
            let _ = run_prodex_auto_runtime_housekeeping();
        });
}

fn auto_runtime_housekeeping_lock_path(paths: &AppPaths) -> PathBuf {
    paths.root.join(AUTO_RUNTIME_HOUSEKEEPING_LOCK_FILE)
}

fn auto_runtime_housekeeping_stamp_path(paths: &AppPaths) -> PathBuf {
    paths.root.join(AUTO_RUNTIME_HOUSEKEEPING_STAMP_FILE)
}

fn try_acquire_auto_runtime_housekeeping_lock(paths: &AppPaths) -> Result<Option<fs::File>> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let lock_path = auto_runtime_housekeeping_lock_path(paths);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to lock {}", lock_path.display())),
    }
}

fn auto_runtime_housekeeping_is_due_at(
    paths: &AppPaths,
    now: SystemTime,
    interval_seconds: i64,
) -> bool {
    if interval_seconds <= 0 {
        return true;
    }
    let Some(now_epoch) = prodex_core::system_time_to_unix_seconds(now) else {
        return false;
    };
    let stamp_path = auto_runtime_housekeeping_stamp_path(paths);
    let last_run = fs::read_to_string(&stamp_path)
        .ok()
        .and_then(|content| content.trim().parse::<i64>().ok())
        .or_else(|| {
            fs::metadata(stamp_path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(prodex_core::system_time_to_unix_seconds)
        });
    last_run.is_none_or(|last_run| now_epoch.saturating_sub(last_run) >= interval_seconds)
}

fn record_auto_runtime_housekeeping_run_at(paths: &AppPaths, now: SystemTime) {
    let stamp_path = auto_runtime_housekeeping_stamp_path(paths);
    if let Some(parent) = stamp_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let epoch = prodex_core::system_time_to_unix_seconds(now).unwrap_or_default();
    let _ = fs::write(stamp_path, format!("{epoch}\n"));
}

pub(crate) fn perform_prodex_auto_runtime_housekeeping_at(
    paths: &AppPaths,
    runtime_log_dir: &Path,
    runtime_log_pointer_path: &Path,
    now: SystemTime,
) -> Result<ProdexCleanupSummary> {
    Ok(ProdexCleanupSummary {
        runtime_logs_removed: cleanup_runtime_proxy_logs_in_dir(runtime_log_dir, now),
        stale_runtime_log_pointer_removed: usize::from(cleanup_runtime_proxy_latest_pointer(
            runtime_log_pointer_path,
        )),
        stale_login_dirs_removed: cleanup_stale_login_dirs_at(paths, now),
        stale_root_temp_files_removed: cleanup_prodex_stale_root_temp_files_at(paths, now),
        dead_runtime_broker_leases_removed: cleanup_runtime_broker_stale_leases_for_all(paths),
        dead_runtime_broker_registries_removed: cleanup_runtime_broker_stale_registries(paths)?,
        ..ProdexCleanupSummary::default()
    })
}

pub(crate) fn run_prodex_auto_runtime_housekeeping() -> Result<Option<ProdexCleanupSummary>> {
    let paths = AppPaths::discover()?;
    run_prodex_auto_runtime_housekeeping_for_paths_at(
        &paths,
        &runtime_proxy_log_dir(),
        &runtime_proxy_latest_log_pointer_path(),
        SystemTime::now(),
        AUTO_RUNTIME_HOUSEKEEPING_INTERVAL_SECONDS,
    )
}

pub(crate) fn run_prodex_auto_runtime_housekeeping_for_paths_at(
    paths: &AppPaths,
    runtime_log_dir: &Path,
    runtime_log_pointer_path: &Path,
    now: SystemTime,
    interval_seconds: i64,
) -> Result<Option<ProdexCleanupSummary>> {
    let Some(_lock) = try_acquire_auto_runtime_housekeeping_lock(paths)? else {
        return Ok(None);
    };
    if !auto_runtime_housekeeping_is_due_at(paths, now, interval_seconds) {
        return Ok(None);
    }
    let summary = perform_prodex_auto_runtime_housekeeping_at(
        paths,
        runtime_log_dir,
        runtime_log_pointer_path,
        now,
    )?;
    record_auto_runtime_housekeeping_run_at(paths, now);
    Ok(Some(summary))
}

pub(crate) fn cleanup_stale_login_dirs_at(paths: &AppPaths, now: SystemTime) -> usize {
    prodex_housekeeping::cleanup_stale_login_dirs_at(
        paths,
        now,
        PROD_EX_TMP_LOGIN_RETENTION_SECONDS,
        |path| remove_dir_if_exists(path).is_ok(),
    )
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
        let is_regular_dir = fs::symlink_metadata(&path)
            .map(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
            .unwrap_or(false);
        if let Some(key) = runtime_broker_artifact_key(name, is_regular_dir) {
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
        if !runtime_broker_lease_dir_is_regular_dir(&lease_dir) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&lease_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !runtime_broker_lease_path_is_regular_file(&path) {
                continue;
            }
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

#[cfg(test)]
pub(crate) fn perform_prodex_cleanup_at(
    paths: &AppPaths,
    state: &AppState,
    runtime_log_dir: &Path,
    runtime_log_pointer_path: &Path,
    now: SystemTime,
) -> Result<ProdexCleanupSummary> {
    perform_prodex_cleanup_with_options_at(
        paths,
        state,
        runtime_log_dir,
        runtime_log_pointer_path,
        now,
        ProdexCleanupOptions::default(),
    )
}

pub(crate) fn perform_prodex_cleanup_with_options_at(
    paths: &AppPaths,
    state: &AppState,
    runtime_log_dir: &Path,
    runtime_log_pointer_path: &Path,
    now: SystemTime,
    options: ProdexCleanupOptions,
) -> Result<ProdexCleanupSummary> {
    Ok(ProdexCleanupSummary {
        duplicate_profiles_removed: 0,
        duplicate_managed_profile_homes_removed: 0,
        runtime_logs_removed: cleanup_runtime_proxy_logs_in_dir(runtime_log_dir, now),
        stale_runtime_log_pointer_removed: usize::from(cleanup_runtime_proxy_latest_pointer(
            runtime_log_pointer_path,
        )),
        stale_login_dirs_removed: cleanup_stale_login_dirs_at(paths, now),
        orphan_managed_profile_dirs_removed: cleanup_orphan_managed_profile_dirs_with_retention_at(
            paths,
            state,
            now,
            options.orphan_managed_profile_retention_seconds,
        ),
        transient_root_files_removed: cleanup_prodex_transient_root_files(paths),
        stale_root_temp_files_removed: cleanup_prodex_stale_root_temp_files_at(paths, now),
        dead_runtime_broker_leases_removed: cleanup_runtime_broker_stale_leases_for_all(paths),
        dead_runtime_broker_registries_removed: cleanup_runtime_broker_stale_registries(paths)?,
    })
}

#[cfg(test)]
pub(crate) fn perform_prodex_cleanup(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<ProdexCleanupSummary> {
    perform_prodex_cleanup_with_options(paths, state, ProdexCleanupOptions::default())
}

pub(crate) fn perform_prodex_cleanup_with_options(
    paths: &AppPaths,
    state: &mut AppState,
    options: ProdexCleanupOptions,
) -> Result<ProdexCleanupSummary> {
    let duplicate_summary = cleanup_duplicate_profiles(paths, state)?;
    let artifact_summary = perform_prodex_cleanup_with_options_at(
        paths,
        state,
        &runtime_proxy_log_dir(),
        &runtime_proxy_latest_log_pointer_path(),
        SystemTime::now(),
        options,
    )?;
    Ok(duplicate_summary.merge(artifact_summary))
}
