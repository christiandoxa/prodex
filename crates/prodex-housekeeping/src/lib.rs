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

const LAST_GOOD_FILE_SUFFIX: &str = ".last-good";

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
    cleanup_prodex_stale_root_temp_files_at_with_counts(paths, now, retention_seconds, pid_alive)
        .removed
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

pub fn cleanup_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
    retention_seconds: i64,
    remove_dir: impl Fn(&Path) -> bool,
) -> usize {
    cleanup_orphan_managed_profile_dirs_at_with_counts(
        paths,
        state,
        now,
        retention_seconds,
        remove_dir,
    )
    .removed
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
