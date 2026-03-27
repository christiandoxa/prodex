use super::*;

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
    let oldest_allowed = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
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
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
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

pub(crate) fn prodex_runtime_log_paths_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|entry| entry.ok().map(|item| item.path())))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(RUNTIME_PROXY_LOG_FILE_PREFIX) && name.ends_with(".log")
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub(crate) fn cleanup_runtime_proxy_logs_in_dir(dir: &Path, now: SystemTime) {
    let now_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let oldest_allowed = now_epoch.saturating_sub(RUNTIME_PROXY_LOG_RETENTION_SECONDS);
    let mut paths = prodex_runtime_log_paths_in_dir(dir)
        .into_iter()
        .filter_map(|path| {
            let modified = path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(i64::MIN);
            Some((path, modified))
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|(path, modified)| (*modified, path.clone()));
    let excess = paths
        .len()
        .saturating_sub(RUNTIME_PROXY_LOG_RETENTION_COUNT);
    for (index, (path, modified)) in paths.into_iter().enumerate() {
        if modified < oldest_allowed || index < excess {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn cleanup_runtime_proxy_latest_pointer(pointer_path: &Path) {
    let should_remove_pointer = fs::read_to_string(pointer_path)
        .ok()
        .map(|content| PathBuf::from(content.trim()))
        .is_some_and(|path| !path.exists());
    if should_remove_pointer {
        let _ = fs::remove_file(pointer_path);
    }
}

pub(crate) fn cleanup_runtime_proxy_log_housekeeping() {
    let temp_dir = runtime_proxy_log_dir();
    cleanup_runtime_proxy_logs_in_dir(&temp_dir, SystemTime::now());
    cleanup_runtime_proxy_latest_pointer(&runtime_proxy_latest_log_pointer_path());
}

pub(crate) fn cleanup_stale_login_dirs_at(paths: &AppPaths, now: SystemTime) {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return;
    };
    let oldest_allowed = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
        - PROD_EX_TMP_LOGIN_RETENTION_SECONDS;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(".login-") {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(i64::MIN);
        if modified < oldest_allowed {
            let _ = remove_dir_if_exists(&path);
        }
    }
}

pub(crate) fn cleanup_stale_login_dirs(paths: &AppPaths) {
    cleanup_stale_login_dirs_at(paths, SystemTime::now());
}
