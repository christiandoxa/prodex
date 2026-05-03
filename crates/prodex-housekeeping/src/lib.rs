//! Filesystem housekeeping helpers.
//!
//! The binary crate owns command orchestration and state persistence. This crate
//! keeps bounded cleanup rules reusable and testable without depending on the
//! runtime proxy hot path.

use chrono::{Local, TimeZone};
use prodex_core::{
    AppPaths, chat_history_file_path_is_owned, login_temp_dir_name_is_owned,
    owned_root_temp_file_name, path_is_under_root, root_temp_file_pid,
    runtime_proxy_log_file_name_is_owned, select_newest_modified_path,
    select_runtime_log_paths_to_remove, session_path_date, should_remove_stale_root_temp_file,
    system_time_to_unix_seconds,
};
use prodex_runtime_claude::{
    runtime_proxy_claude_config_dir, runtime_proxy_shared_claude_config_dir,
};
use prodex_state::AppState;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufRead, Write};
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
    pub chat_history_entries_removed: usize,
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
            + self.chat_history_entries_removed
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
        self.chat_history_entries_removed += other.chat_history_entries_removed;
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
    let oldest_allowed = system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds;
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

fn prodex_file_modified_epoch(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(system_time_to_unix_seconds)
}

fn prodex_history_epoch_from_json_value(value: &serde_json::Value) -> Option<i64> {
    fn normalize_epoch(value: i64) -> i64 {
        if value.abs() > 100_000_000_000 {
            value / 1_000
        } else {
            value
        }
    }

    for key in ["ts", "timestamp", "created_at", "updated_at"] {
        let Some(value) = value.get(key) else {
            continue;
        };
        if let Some(epoch) = value.as_i64() {
            return Some(normalize_epoch(epoch));
        }
        if let Some(text) = value.as_str() {
            if let Ok(epoch) = text.parse::<i64>() {
                return Some(normalize_epoch(epoch));
            }
            if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(text) {
                return Some(parsed.timestamp());
            }
        }
    }

    None
}

fn prodex_history_line_epoch(line: &str) -> Option<i64> {
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    prodex_history_epoch_from_json_value(&value)
}

fn prune_jsonl_history_file_at(
    path: &Path,
    oldest_allowed: i64,
    prune_unstamped_old_file: bool,
) -> usize {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return 0;
    }

    let file_is_old =
        prodex_file_modified_epoch(path).is_some_and(|modified| modified < oldest_allowed);
    let Ok(input) = fs::File::open(path) else {
        return 0;
    };
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return 0;
    };
    let tmp_path = path.with_file_name(format!(
        ".{file_name}.prodex-cleanup-{}.tmp",
        std::process::id()
    ));
    let _ = fs::remove_file(&tmp_path);
    let Ok(output) = fs::File::create(&tmp_path) else {
        return 0;
    };

    let mut reader = io::BufReader::new(input);
    let mut writer = io::BufWriter::new(output);
    let mut raw = String::new();
    let mut removed = 0usize;
    let mut retained = 0usize;

    loop {
        raw.clear();
        let Ok(bytes_read) = reader.read_line(&mut raw) else {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        };
        if bytes_read == 0 {
            break;
        }

        let line = raw.trim_end_matches(['\r', '\n']);
        let is_stale = prodex_history_line_epoch(line)
            .map(|epoch| epoch < oldest_allowed)
            .unwrap_or(prune_unstamped_old_file && file_is_old);
        if is_stale {
            removed += 1;
            continue;
        }

        if retained > 0 && writeln!(writer).is_err() {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        }
        if write!(writer, "{line}").is_err() {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        }
        retained += 1;
    }

    if removed == 0 {
        let _ = fs::remove_file(&tmp_path);
        return 0;
    }
    if writer.flush().is_err() {
        let _ = fs::remove_file(&tmp_path);
        return 0;
    }

    if retained == 0 {
        if fs::remove_file(path).is_err() {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        }
        let _ = fs::remove_file(&tmp_path);
        return removed;
    }

    if fs::rename(&tmp_path, path)
        .or_else(|_| {
            fs::copy(&tmp_path, path)?;
            fs::remove_file(&tmp_path)
        })
        .is_ok()
    {
        removed
    } else {
        let _ = fs::remove_file(&tmp_path);
        0
    }
}

fn prodex_session_path_date(path: &Path) -> Option<chrono::NaiveDate> {
    let date = session_path_date(path)?;
    chrono::NaiveDate::from_ymd_opt(date.year, date.month, date.day)
}

fn prodex_local_midnight_epoch(date: chrono::NaiveDate) -> Option<i64> {
    let naive = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|datetime| datetime.timestamp())
}

fn prodex_session_file_is_stale(path: &Path, oldest_allowed: i64) -> bool {
    if let Some(date) = prodex_session_path_date(path) {
        return date
            .succ_opt()
            .and_then(prodex_local_midnight_epoch)
            .is_some_and(|next_day_epoch| next_day_epoch <= oldest_allowed);
    }

    prodex_file_modified_epoch(path).is_some_and(|modified| modified < oldest_allowed)
}

fn cleanup_codex_session_history_tree_at(root: &Path, oldest_allowed: i64) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            removed += cleanup_codex_session_history_tree_at(&path, oldest_allowed);
            let _ = fs::remove_dir(&path);
            continue;
        }
        if metadata.is_file()
            && chat_history_file_path_is_owned(&path)
            && prodex_session_file_is_stale(&path, oldest_allowed)
            && fs::remove_file(&path).is_ok()
        {
            removed += 1;
        }
    }

    removed
}

fn cleanup_claude_project_history_tree_at(root: &Path, oldest_allowed: i64) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            removed += cleanup_claude_project_history_tree_at(&path, oldest_allowed);
            let _ = fs::remove_dir(&path);
            continue;
        }
        if metadata.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
        {
            removed += prune_jsonl_history_file_at(&path, oldest_allowed, true);
        }
    }

    removed
}

fn cleanup_codex_chat_history_root_at(root: &Path, oldest_allowed: i64) -> usize {
    prune_jsonl_history_file_at(&root.join("history.jsonl"), oldest_allowed, false)
        + cleanup_codex_session_history_tree_at(&root.join("sessions"), oldest_allowed)
}

fn cleanup_claude_chat_history_root_at(root: &Path, oldest_allowed: i64) -> usize {
    cleanup_claude_project_history_tree_at(&root.join("projects"), oldest_allowed)
}

pub fn cleanup_prodex_chat_history_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
) -> usize {
    let now_epoch = system_time_to_unix_seconds(now).unwrap_or_default();
    let oldest_allowed = now_epoch.saturating_sub(retention_seconds);
    let mut removed = 0usize;

    let mut codex_roots = BTreeSet::new();
    for root in [&paths.shared_codex_root, &paths.legacy_shared_codex_root] {
        if path_is_under_root(&paths.root, root) {
            codex_roots.insert(root.clone());
        }
    }
    for profile in state.profiles.values() {
        if profile.managed && path_is_under_root(&paths.root, &profile.codex_home) {
            codex_roots.insert(profile.codex_home.clone());
        }
    }
    for root in codex_roots {
        removed += cleanup_codex_chat_history_root_at(&root, oldest_allowed);
        removed += cleanup_claude_chat_history_root_at(
            &runtime_proxy_claude_config_dir(&root),
            oldest_allowed,
        );
    }

    let shared_claude_root = runtime_proxy_shared_claude_config_dir(&paths.root);
    if path_is_under_root(&paths.root, &shared_claude_root) {
        removed += cleanup_claude_chat_history_root_at(&shared_claude_root, oldest_allowed);
    }

    removed
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
