use super::*;

mod duplicates;

use self::duplicates::cleanup_duplicate_profiles;
use fs2::FileExt;
use prodex_core::{runtime_broker_artifact_key, runtime_broker_lease_pid};

pub(crate) use prodex_housekeeping::{ProdexCleanupCounts, ProdexCleanupSummary};

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

fn cleanup_prodex_transient_root_files_with_counts(paths: &AppPaths) -> ProdexCleanupCounts {
    prodex_housekeeping::cleanup_existing_files_under(
        &paths.root,
        prodex_cleanup_transient_root_file_paths(paths),
    )
    .counts()
}

fn cleanup_prodex_stale_root_temp_files_at_with_counts(
    paths: &AppPaths,
    now: SystemTime,
) -> ProdexCleanupCounts {
    prodex_housekeeping::cleanup_prodex_stale_root_temp_files_at_with_counts(
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

fn cleanup_orphan_managed_profile_dirs_with_retention_at_with_counts(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
) -> ProdexCleanupCounts {
    prodex_housekeeping::cleanup_orphan_managed_profile_dirs_at_with_counts(
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

#[cfg(test)]
pub(crate) fn cleanup_runtime_proxy_logs_in_dir(dir: &Path, now: SystemTime) -> usize {
    cleanup_runtime_proxy_logs_in_dir_with_counts(dir, now).removed
}

fn cleanup_runtime_proxy_logs_in_dir_with_counts(
    dir: &Path,
    now: SystemTime,
) -> ProdexCleanupCounts {
    prodex_housekeeping::cleanup_runtime_proxy_logs_in_dir_with_counts(
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

#[cfg(test)]
pub(crate) use prodex_housekeeping::cleanup_runtime_proxy_latest_pointer;

fn cleanup_runtime_proxy_latest_pointer_with_counts(path: &Path) -> ProdexCleanupCounts {
    prodex_housekeeping::cleanup_runtime_proxy_latest_pointer_with_counts(path)
}

fn add_cleanup_counts(summary: &mut ProdexCleanupSummary, counts: ProdexCleanupCounts) {
    summary.scan_failures += counts.scan_failures;
    summary.delete_failures += counts.delete_failures;
}

pub(crate) fn command_runs_auto_runtime_housekeeping(command: &Commands) -> bool {
    !crate::command_dispatch::command_is_native_dry_run(command)
        && !matches!(
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
    let runtime_logs = cleanup_runtime_proxy_logs_in_dir_with_counts(runtime_log_dir, now);
    let stale_pointer = cleanup_runtime_proxy_latest_pointer_with_counts(runtime_log_pointer_path);
    let stale_login_dirs = cleanup_stale_login_dirs_at_with_counts(paths, now);
    let stale_root_temp_files = cleanup_prodex_stale_root_temp_files_at_with_counts(paths, now);
    let broker_leases = cleanup_runtime_broker_stale_leases_for_all(paths);
    let broker_registries = cleanup_runtime_broker_stale_registries(paths)?;
    let mut summary = ProdexCleanupSummary {
        runtime_logs_removed: runtime_logs.removed,
        stale_runtime_log_pointer_removed: stale_pointer.removed,
        stale_login_dirs_removed: stale_login_dirs.removed,
        stale_root_temp_files_removed: stale_root_temp_files.removed,
        dead_runtime_broker_leases_removed: broker_leases.removed,
        dead_runtime_broker_registries_removed: broker_registries.removed,
        ..ProdexCleanupSummary::default()
    };
    for counts in [
        runtime_logs,
        stale_pointer,
        stale_login_dirs,
        stale_root_temp_files,
        broker_leases,
        broker_registries,
    ] {
        add_cleanup_counts(&mut summary, counts);
    }
    Ok(summary)
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

#[cfg(test)]
pub(crate) fn cleanup_stale_login_dirs_at(paths: &AppPaths, now: SystemTime) -> usize {
    cleanup_stale_login_dirs_at_with_counts(paths, now).removed
}

fn cleanup_stale_login_dirs_at_with_counts(
    paths: &AppPaths,
    now: SystemTime,
) -> ProdexCleanupCounts {
    prodex_housekeeping::cleanup_stale_login_dirs_at_with_counts(
        paths,
        now,
        PROD_EX_TMP_LOGIN_RETENTION_SECONDS,
        |path| remove_dir_if_exists(path).is_ok(),
    )
}

fn runtime_broker_artifact_keys(paths: &AppPaths) -> (Vec<String>, ProdexCleanupCounts) {
    let entries = match fs::read_dir(&paths.root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (Vec::new(), ProdexCleanupCounts::default());
        }
        Err(_) => {
            return (
                Vec::new(),
                ProdexCleanupCounts {
                    scan_failures: 1,
                    ..ProdexCleanupCounts::default()
                },
            );
        }
    };
    let mut counts = ProdexCleanupCounts::default();
    let mut keys = Vec::new();
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
        if !name.starts_with("runtime-broker-") {
            continue;
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                counts.scan_failures += 1;
                continue;
            }
        };
        let is_regular_dir = !metadata.file_type().is_symlink() && metadata.is_dir();
        if let Some(key) = runtime_broker_artifact_key(name, is_regular_dir) {
            keys.push(key.to_string());
        }
    }
    keys.sort();
    keys.dedup();
    (keys, counts)
}

fn runtime_broker_artifact_paths(paths: &AppPaths, broker_key: &str) -> [PathBuf; 3] {
    [
        runtime_broker_registry_file_path(paths, broker_key),
        runtime_broker_registry_last_good_file_path(paths, broker_key),
        runtime_broker_capability_file_path(paths, broker_key),
    ]
}

fn runtime_broker_artifact_exists(path: &Path, counts: &mut ProdexCleanupCounts) -> bool {
    match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => {
            counts.scan_failures += 1;
            false
        }
    }
}

pub(crate) fn cleanup_runtime_broker_stale_registries(
    paths: &AppPaths,
) -> Result<ProdexCleanupCounts> {
    let (broker_keys, mut counts) = runtime_broker_artifact_keys(paths);
    for broker_key in broker_keys {
        let Some(registry) = load_runtime_broker_registry(paths, &broker_key)? else {
            let capability_path = runtime_broker_capability_file_path(paths, &broker_key);
            let had_capability = runtime_broker_artifact_exists(&capability_path, &mut counts);
            let removed = remove_runtime_broker_orphaned_capability(paths, &broker_key);
            if removed {
                counts.removed += 1;
            } else if had_capability
                && runtime_broker_artifact_exists(&capability_path, &mut counts)
            {
                counts.delete_failures += 1;
            }
            continue;
        };
        if !runtime_process_absence_proven(registry.pid) {
            continue;
        }
        let artifact_paths = runtime_broker_artifact_paths(paths, &broker_key);
        let had_artifact = artifact_paths
            .iter()
            .any(|path| runtime_broker_artifact_exists(path, &mut counts));
        remove_runtime_broker_registry_if_instance_matches(
            paths,
            &broker_key,
            &registry.instance_id,
        );
        let remains = artifact_paths
            .iter()
            .any(|path| runtime_broker_artifact_exists(path, &mut counts));
        if had_artifact && !remains {
            counts.removed += 1;
        } else if had_artifact && remains {
            counts.delete_failures += 1;
        }
    }
    Ok(counts)
}

pub(crate) fn cleanup_runtime_broker_stale_leases_for_all(paths: &AppPaths) -> ProdexCleanupCounts {
    let (broker_keys, mut counts) = runtime_broker_artifact_keys(paths);
    for broker_key in broker_keys {
        let lease_counts = cleanup_runtime_broker_stale_leases_in_dir(&runtime_broker_lease_dir(
            paths,
            &broker_key,
        ));
        counts.removed += lease_counts.removed;
        counts.scan_failures += lease_counts.scan_failures;
        counts.delete_failures += lease_counts.delete_failures;
    }
    counts
}

fn cleanup_runtime_broker_stale_lease_path(path: &Path, counts: &mut ProdexCleanupCounts) {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(_) => {
            counts.scan_failures += 1;
            return;
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return;
    }
    let Some(pid) = runtime_broker_lease_pid(file_name) else {
        return;
    };
    if !runtime_process_absence_proven(pid) {
        return;
    }
    match fs::remove_file(path) {
        Ok(()) => counts.removed += 1,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => counts.delete_failures += 1,
    }
}

fn cleanup_runtime_broker_empty_lease_dir(lease_dir: &Path, counts: &mut ProdexCleanupCounts) {
    let mut remaining = match fs::read_dir(lease_dir) {
        Ok(remaining) => remaining,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(_) => {
            counts.scan_failures += 1;
            return;
        }
    };
    match remaining.next() {
        Some(Ok(_)) => return,
        Some(Err(_)) => {
            counts.scan_failures += 1;
            return;
        }
        None => {}
    }
    match fs::remove_dir(lease_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => counts.delete_failures += 1,
    }
}

fn cleanup_runtime_broker_stale_leases_in_dir(lease_dir: &Path) -> ProdexCleanupCounts {
    let metadata = match fs::symlink_metadata(lease_dir) {
        Ok(metadata) => metadata,
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
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return ProdexCleanupCounts::default();
    }
    let entries = match fs::read_dir(lease_dir) {
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
    let mut counts = ProdexCleanupCounts::default();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                counts.scan_failures += 1;
                continue;
            }
        };
        cleanup_runtime_broker_stale_lease_path(&entry.path(), &mut counts);
    }
    cleanup_runtime_broker_empty_lease_dir(lease_dir, &mut counts);
    counts
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
    let runtime_logs = cleanup_runtime_proxy_logs_in_dir_with_counts(runtime_log_dir, now);
    let stale_pointer = cleanup_runtime_proxy_latest_pointer_with_counts(runtime_log_pointer_path);
    let stale_login_dirs = cleanup_stale_login_dirs_at_with_counts(paths, now);
    let orphan_managed_profile_dirs =
        cleanup_orphan_managed_profile_dirs_with_retention_at_with_counts(
            paths,
            state,
            now,
            options.orphan_managed_profile_retention_seconds,
        );
    let transient_root_files = cleanup_prodex_transient_root_files_with_counts(paths);
    let stale_root_temp_files = cleanup_prodex_stale_root_temp_files_at_with_counts(paths, now);
    let broker_leases = cleanup_runtime_broker_stale_leases_for_all(paths);
    let broker_registries = cleanup_runtime_broker_stale_registries(paths)?;
    let mut summary = ProdexCleanupSummary {
        duplicate_profiles_removed: 0,
        duplicate_managed_profile_homes_removed: 0,
        runtime_logs_removed: runtime_logs.removed,
        stale_runtime_log_pointer_removed: stale_pointer.removed,
        stale_login_dirs_removed: stale_login_dirs.removed,
        orphan_managed_profile_dirs_removed: orphan_managed_profile_dirs.removed,
        transient_root_files_removed: transient_root_files.removed,
        stale_root_temp_files_removed: stale_root_temp_files.removed,
        dead_runtime_broker_leases_removed: broker_leases.removed,
        dead_runtime_broker_registries_removed: broker_registries.removed,
        ..ProdexCleanupSummary::default()
    };
    for counts in [
        runtime_logs,
        stale_pointer,
        stale_login_dirs,
        orphan_managed_profile_dirs,
        transient_root_files,
        stale_root_temp_files,
        broker_leases,
        broker_registries,
    ] {
        add_cleanup_counts(&mut summary, counts);
    }
    Ok(summary)
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
