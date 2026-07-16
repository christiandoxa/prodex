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

const LAST_GOOD_FILE_SUFFIX: &str = ".last-good";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProdexRepairSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProdexRepairActionKind {
    MissingStateFile,
    UnreadableStateFile,
    InvalidStateFile,
    RestoreLastGoodState,
    RemoveStaleRootTempFile,
    CreateMissingProfileHome,
    RemoveOrphanManagedProfileHome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProdexRepairPlanAction {
    pub kind: ProdexRepairActionKind,
    pub severity: ProdexRepairSeverity,
    pub path: PathBuf,
    pub secondary_path: Option<PathBuf>,
    pub profile_name: Option<String>,
    pub dry_run_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProdexRepairPlanOptions {
    pub stale_root_temp_retention_seconds: i64,
    pub orphan_managed_profile_retention_seconds: i64,
    pub redact_paths: bool,
}

impl Default for ProdexRepairPlanOptions {
    fn default() -> Self {
        Self {
            stale_root_temp_retention_seconds: 24 * 60 * 60,
            orphan_managed_profile_retention_seconds: 7 * 24 * 60 * 60,
            redact_paths: false,
        }
    }
}

fn last_good_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot.json");
    path.with_file_name(format!("{file_name}{LAST_GOOD_FILE_SUFFIX}"))
}

fn path_label(paths: &AppPaths, path: &Path, redact_paths: bool) -> String {
    if !redact_paths {
        return path.display().to_string();
    }
    if let Ok(suffix) = path.strip_prefix(&paths.root) {
        let suffix = suffix.display().to_string();
        return if suffix.is_empty() {
            "<prodex-home>".to_string()
        } else {
            format!("<prodex-home>/{suffix}")
        };
    }
    "<path>".to_string()
}

fn state_file_recoverable(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<AppState>(&content).ok())
        .is_some()
}

fn push_state_issue(
    actions: &mut Vec<ProdexRepairPlanAction>,
    paths: &AppPaths,
    kind: ProdexRepairActionKind,
    redact_paths: bool,
) {
    let last_good = last_good_file_path(&paths.state_file);
    if state_file_recoverable(&last_good) {
        let prefix = match kind {
            ProdexRepairActionKind::UnreadableStateFile => "would restore unreadable",
            ProdexRepairActionKind::InvalidStateFile => "would restore invalid",
            _ => "would restore",
        };
        actions.push(ProdexRepairPlanAction {
            kind: ProdexRepairActionKind::RestoreLastGoodState,
            severity: ProdexRepairSeverity::Warning,
            path: paths.state_file.clone(),
            secondary_path: Some(last_good.clone()),
            profile_name: None,
            dry_run_text: format!(
                "{prefix} {} from {}",
                path_label(paths, &paths.state_file, redact_paths),
                path_label(paths, &last_good, redact_paths)
            ),
        });
        return;
    }

    let label = match kind {
        ProdexRepairActionKind::MissingStateFile => "missing",
        ProdexRepairActionKind::UnreadableStateFile => "unreadable",
        ProdexRepairActionKind::InvalidStateFile => "invalid",
        _ => "invalid",
    };
    actions.push(ProdexRepairPlanAction {
        kind,
        severity: ProdexRepairSeverity::Critical,
        path: paths.state_file.clone(),
        secondary_path: None,
        profile_name: None,
        dry_run_text: format!(
            "would report {label} prodex state file {}",
            path_label(paths, &paths.state_file, redact_paths)
        ),
    });
}

fn plan_state_file_repair(
    actions: &mut Vec<ProdexRepairPlanAction>,
    paths: &AppPaths,
    redact_paths: bool,
) {
    match fs::read_to_string(&paths.state_file) {
        Ok(content) if serde_json::from_str::<AppState>(&content).is_ok() => {}
        Ok(_) => push_state_issue(
            actions,
            paths,
            ProdexRepairActionKind::InvalidStateFile,
            redact_paths,
        ),
        Err(err) if err.kind() == io::ErrorKind::NotFound => push_state_issue(
            actions,
            paths,
            ProdexRepairActionKind::MissingStateFile,
            redact_paths,
        ),
        Err(_) => push_state_issue(
            actions,
            paths,
            ProdexRepairActionKind::UnreadableStateFile,
            redact_paths,
        ),
    }
}

fn plan_stale_root_temp_repairs(
    actions: &mut Vec<ProdexRepairPlanAction>,
    paths: &AppPaths,
    now: SystemTime,
    retention_seconds: i64,
    pid_alive: &impl Fn(u32) -> bool,
    redact_paths: bool,
) {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return;
    };
    let oldest_allowed = system_time_to_unix_seconds(now).unwrap_or_default() - retention_seconds;
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
        let pid_alive = root_temp_file_pid(name).is_some_and(pid_alive);
        if should_remove_stale_root_temp_file(name, modified, oldest_allowed, pid_alive) {
            actions.push(ProdexRepairPlanAction {
                kind: ProdexRepairActionKind::RemoveStaleRootTempFile,
                severity: ProdexRepairSeverity::Info,
                path: path.clone(),
                secondary_path: None,
                profile_name: None,
                dry_run_text: format!(
                    "would remove stale prodex temp file {}",
                    path_label(paths, &path, redact_paths)
                ),
            });
        }
    }
}

fn plan_missing_profile_home_repairs(
    actions: &mut Vec<ProdexRepairPlanAction>,
    paths: &AppPaths,
    state: &AppState,
    redact_paths: bool,
) {
    for (profile_name, profile) in &state.profiles {
        if profile.codex_home.exists() {
            continue;
        }
        actions.push(ProdexRepairPlanAction {
            kind: ProdexRepairActionKind::CreateMissingProfileHome,
            severity: ProdexRepairSeverity::Warning,
            path: profile.codex_home.clone(),
            secondary_path: None,
            profile_name: Some(profile_name.clone()),
            dry_run_text: format!(
                "would create missing Codex home for profile {profile_name}: {}",
                path_label(paths, &profile.codex_home, redact_paths)
            ),
        });
    }
}

fn plan_orphan_managed_profile_home_repairs(
    actions: &mut Vec<ProdexRepairPlanAction>,
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
    redact_paths: bool,
) {
    for name in collect_orphan_managed_profile_dirs_at(paths, state, now, retention_seconds) {
        let path = paths.managed_profiles_root.join(&name);
        actions.push(ProdexRepairPlanAction {
            kind: ProdexRepairActionKind::RemoveOrphanManagedProfileHome,
            severity: ProdexRepairSeverity::Info,
            path: path.clone(),
            secondary_path: None,
            profile_name: Some(name),
            dry_run_text: format!(
                "would remove orphaned managed Codex home {}",
                path_label(paths, &path, redact_paths)
            ),
        });
    }
}

pub fn plan_prodex_state_repairs_at(
    paths: &AppPaths,
    state: Option<&AppState>,
    now: SystemTime,
    options: ProdexRepairPlanOptions,
    pid_alive: impl Fn(u32) -> bool,
) -> Vec<ProdexRepairPlanAction> {
    let mut actions = Vec::new();

    plan_state_file_repair(&mut actions, paths, options.redact_paths);
    plan_stale_root_temp_repairs(
        &mut actions,
        paths,
        now,
        options.stale_root_temp_retention_seconds,
        &pid_alive,
        options.redact_paths,
    );
    if let Some(state) = state {
        plan_missing_profile_home_repairs(&mut actions, paths, state, options.redact_paths);
        plan_orphan_managed_profile_home_repairs(
            &mut actions,
            paths,
            state,
            now,
            options.orphan_managed_profile_retention_seconds,
            options.redact_paths,
        );
    }

    actions.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.kind_label().cmp(right.kind_label()))
            .then_with(|| left.path.cmp(&right.path))
    });
    actions
}

impl ProdexRepairPlanAction {
    fn kind_label(&self) -> &'static str {
        match self.kind {
            ProdexRepairActionKind::MissingStateFile => "missing_state_file",
            ProdexRepairActionKind::UnreadableStateFile => "unreadable_state_file",
            ProdexRepairActionKind::InvalidStateFile => "invalid_state_file",
            ProdexRepairActionKind::RestoreLastGoodState => "restore_last_good_state",
            ProdexRepairActionKind::RemoveStaleRootTempFile => "remove_stale_root_temp_file",
            ProdexRepairActionKind::CreateMissingProfileHome => "create_missing_profile_home",
            ProdexRepairActionKind::RemoveOrphanManagedProfileHome => {
                "remove_orphan_managed_profile_home"
            }
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

pub fn cleanup_existing_files<I>(paths: I) -> usize
where
    I: IntoIterator<Item = PathBuf>,
{
    paths
        .into_iter()
        .filter(|path| remove_file_if_exists(path))
        .count()
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

fn path_is_contained_without_symlink_parents(root: &Path, path: &Path) -> bool {
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
        .filter(|path| runtime_proxy_log_path_is_regular_owned(path, log_prefix))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn runtime_proxy_log_path_is_regular_owned(path: &Path, log_prefix: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| runtime_proxy_log_file_name_is_owned(name, log_prefix))
        && fs::symlink_metadata(path)
            .map(|metadata| !metadata.file_type().is_symlink() && metadata.is_file())
            .unwrap_or(false)
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
