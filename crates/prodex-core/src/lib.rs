use anyhow::{Context, Result};
use dirs::home_dir;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_PRODEX_DIR: &str = ".prodex";
pub const DEFAULT_CODEX_DIR: &str = ".codex";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub state_file: PathBuf,
    pub managed_profiles_root: PathBuf,
    pub shared_codex_root: PathBuf,
    pub legacy_shared_codex_root: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let root = match env::var_os("PRODEX_HOME") {
            Some(path) => absolutize(PathBuf::from(path))?,
            None => home_dir()
                .context("failed to determine home directory")?
                .join(DEFAULT_PRODEX_DIR),
        };

        Ok(Self {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: match env::var_os("PRODEX_SHARED_CODEX_HOME") {
                Some(path) => resolve_shared_codex_root(&root, PathBuf::from(path)),
                None => prodex_default_shared_codex_root(&root)?,
            },
            legacy_shared_codex_root: root.join("shared"),
            root,
        })
    }
}

pub fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    Ok(current_dir.join(path))
}

pub fn legacy_default_codex_home() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CODEX_DIR))
}

pub fn default_codex_home(paths: &AppPaths) -> Result<PathBuf> {
    let legacy = legacy_default_codex_home()?;
    Ok(select_default_codex_home(
        &paths.shared_codex_root,
        &legacy,
        env::var_os("PRODEX_SHARED_CODEX_HOME").is_some(),
    ))
}

pub fn select_default_codex_home(
    shared_codex_root: &Path,
    legacy_codex_home: &Path,
    override_active: bool,
) -> PathBuf {
    if override_active || shared_codex_root.exists() || !legacy_codex_home.exists() {
        shared_codex_root.to_path_buf()
    } else {
        legacy_codex_home.to_path_buf()
    }
}

pub fn prodex_default_shared_codex_root(_root: &Path) -> Result<PathBuf> {
    legacy_default_codex_home()
}

pub fn prodex_previous_default_shared_codex_root(root: &Path) -> PathBuf {
    root.join(DEFAULT_CODEX_DIR)
}

pub fn resolve_shared_codex_root(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

pub fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn path_is_under_root(root: &Path, path: &Path) -> bool {
    let root = normalize_path_for_compare(root);
    let path = normalize_path_for_compare(path);
    path == root || path.starts_with(root)
}

pub fn system_time_to_unix_seconds(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
}

pub fn owned_root_temp_file_name(name: &str) -> bool {
    name.starts_with("state.json.")
        || name.starts_with("runtime-")
        || name.starts_with("update-check.json.")
}

pub fn root_temp_file_pid(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".tmp")?;
    let mut parts = stem.rsplitn(4, '.');
    let _sequence = parts.next()?;
    let _nanos = parts.next()?;
    let pid = parts.next()?;
    let _base_name = parts.next()?;
    pid.parse::<u32>().ok()
}

pub fn should_remove_stale_root_temp_file(
    name: &str,
    modified_epoch_seconds: i64,
    oldest_allowed_epoch_seconds: i64,
    pid_alive: bool,
) -> bool {
    !pid_alive
        && (modified_epoch_seconds < oldest_allowed_epoch_seconds
            || root_temp_file_pid(name).is_some())
}

pub fn runtime_proxy_log_file_name_is_owned(name: &str, prefix: &str) -> bool {
    name.starts_with(prefix) && name.ends_with(".log")
}

pub fn select_runtime_log_paths_to_remove(
    mut paths: Vec<(PathBuf, i64)>,
    oldest_allowed_epoch_seconds: i64,
    retention_count: usize,
) -> Vec<PathBuf> {
    paths.sort_by_key(|(path, modified)| (*modified, path.clone()));
    let excess = paths.len().saturating_sub(retention_count);
    paths
        .into_iter()
        .enumerate()
        .filter_map(|(index, (path, modified))| {
            (modified < oldest_allowed_epoch_seconds || index < excess).then_some(path)
        })
        .collect()
}

pub fn select_newest_modified_path(paths: Vec<(u128, PathBuf)>) -> Option<PathBuf> {
    paths
        .into_iter()
        .max_by(|(left_modified, left_path), (right_modified, right_path)| {
            left_modified
                .cmp(right_modified)
                .then_with(|| left_path.cmp(right_path))
        })
        .map(|(_, path)| path)
}

pub fn login_temp_dir_name_is_owned(name: &str) -> bool {
    name.starts_with(".login-")
}

pub fn runtime_broker_artifact_key(name: &str, is_dir: bool) -> Option<&str> {
    if let Some(key) = name
        .strip_prefix("runtime-broker-")
        .and_then(|suffix| suffix.strip_suffix(".json"))
    {
        return Some(key);
    }
    if let Some(key) = name
        .strip_prefix("runtime-broker-")
        .and_then(|suffix| suffix.strip_suffix(".json.last-good"))
    {
        return Some(key);
    }
    if is_dir {
        return name
            .strip_prefix("runtime-broker-")
            .and_then(|suffix| suffix.strip_suffix("-leases"));
    }
    None
}

pub fn runtime_broker_lease_pid(file_name: &str) -> Option<u32> {
    file_name
        .split('-')
        .next()
        .and_then(|value| value.parse::<u32>().ok())
}

pub fn chat_history_file_path_is_owned(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("jsonl") || extension.eq_ignore_ascii_case("json")
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl PathDate {
    pub fn new(year: i32, month: u32, day: u32) -> Option<Self> {
        let max_day = days_in_month(year, month)?;
        (day >= 1 && day <= max_day).then_some(Self { year, month, day })
    }
}

pub fn session_path_date(path: &Path) -> Option<PathDate> {
    let parts = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    for window in parts.windows(3) {
        let year = window[0];
        let month = window[1];
        let day = window[2];
        if year.len() != 4
            || month.len() != 2
            || day.len() != 2
            || !year.chars().all(|ch| ch.is_ascii_digit())
            || !month.chars().all(|ch| ch.is_ascii_digit())
            || !day.chars().all(|ch| ch.is_ascii_digit())
        {
            continue;
        }

        let Ok(year) = year.parse::<i32>() else {
            continue;
        };
        let Ok(month) = month.parse::<u32>() else {
            continue;
        };
        let Ok(day) = day.parse::<u32>() else {
            continue;
        };
        if let Some(date) = PathDate::new(year, month, day) {
            return Some(date);
        }
    }
    None
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn format_binary_resolution(binary: &OsString) -> String {
    let configured = binary.to_string_lossy();
    match resolve_binary_path(binary) {
        Some(path) => format!("{configured} ({})", path.display()),
        None => format!("{configured} (not found)"),
    }
}

pub fn resolve_binary_path(binary: &OsString) -> Option<PathBuf> {
    let candidate = PathBuf::from(binary);
    if candidate.components().count() > 1 {
        if candidate.is_file() {
            return Some(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
        return None;
    }

    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        let full_path = directory.join(&candidate);
        if full_path.is_file() {
            return Some(full_path);
        }
    }

    None
}

#[cfg(test)]
#[path = "../../../tests/unit/crates/prodex-core/src/lib.rs"]
mod tests;
