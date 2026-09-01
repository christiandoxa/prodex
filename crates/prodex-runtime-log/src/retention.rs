use super::RuntimeAsyncLoggerInner;
use fs2::FileExt;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const RUNTIME_LOG_FILE_PREFIX: &str = "prodex-runtime";
const RUNTIME_LOG_LATEST_POINTER_FILE: &str = "prodex-runtime-latest.path";
const RUNTIME_LOG_DIRECTORY_LOCK_FILE: &str = "prodex-runtime.lock";
const RUNTIME_LOG_ACTIVE_LOCK_SUFFIX: &str = ".lock";
const RUNTIME_LOG_MAX_FILE_BYTES_ENV: &str = "PRODEX_RUNTIME_LOG_MAX_BYTES";
const RUNTIME_LOG_MAX_FILES_ENV: &str = "PRODEX_RUNTIME_LOG_MAX_FILES";
const RUNTIME_LOG_TOTAL_BYTES_ENV: &str = "PRODEX_RUNTIME_LOG_TOTAL_BYTES";
const RUNTIME_LOG_MAX_AGE_SECONDS_ENV: &str = "PRODEX_RUNTIME_LOG_MAX_AGE_SECONDS";
const RUNTIME_LOG_RECORD_ENV: &str = "PRODEX_RUNTIME_LOG_RECORD";

pub const DEFAULT_RUNTIME_LOG_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_RUNTIME_LOG_MAX_FILES: usize = 5;
pub const DEFAULT_RUNTIME_LOG_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
pub const DEFAULT_RUNTIME_LOG_MAX_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;
const MAX_RUNTIME_LOG_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_RUNTIME_LOG_FILES: usize = 256;
const MAX_RUNTIME_LOG_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_RUNTIME_LOG_AGE_SECONDS: i64 = 365 * 24 * 60 * 60;

static RUNTIME_LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLogPolicy {
    pub max_file_bytes: u64,
    pub max_files: usize,
    pub total_bytes: u64,
    pub max_age_seconds: i64,
    pub record_to_disk: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLogCleanupReport {
    pub removed: usize,
    pub scan_failures: usize,
    pub delete_failures: usize,
}

impl Default for RuntimeLogPolicy {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_RUNTIME_LOG_MAX_FILE_BYTES,
            max_files: DEFAULT_RUNTIME_LOG_MAX_FILES,
            total_bytes: DEFAULT_RUNTIME_LOG_TOTAL_BYTES,
            max_age_seconds: DEFAULT_RUNTIME_LOG_MAX_AGE_SECONDS,
            record_to_disk: false,
        }
    }
}

impl RuntimeLogPolicy {
    pub fn from_environment() -> Self {
        Self::from_environment_with_retention(
            DEFAULT_RUNTIME_LOG_MAX_AGE_SECONDS,
            DEFAULT_RUNTIME_LOG_MAX_FILES,
        )
    }

    pub fn from_environment_with_retention(
        default_max_age_seconds: i64,
        default_max_files: usize,
    ) -> Self {
        let defaults = Self {
            max_files: default_max_files,
            max_age_seconds: default_max_age_seconds,
            ..Self::default()
        };
        Self {
            max_file_bytes: bounded_environment_u64(
                RUNTIME_LOG_MAX_FILE_BYTES_ENV,
                defaults.max_file_bytes,
                1,
                MAX_RUNTIME_LOG_FILE_BYTES,
            ),
            max_files: bounded_environment_usize(
                RUNTIME_LOG_MAX_FILES_ENV,
                defaults.max_files,
                1,
                MAX_RUNTIME_LOG_FILES,
            ),
            total_bytes: bounded_environment_u64(
                RUNTIME_LOG_TOTAL_BYTES_ENV,
                defaults.total_bytes,
                1,
                MAX_RUNTIME_LOG_TOTAL_BYTES,
            ),
            max_age_seconds: bounded_environment_i64(
                RUNTIME_LOG_MAX_AGE_SECONDS_ENV,
                defaults.max_age_seconds,
                1,
                MAX_RUNTIME_LOG_AGE_SECONDS,
            ),
            record_to_disk: runtime_log_recording_enabled(),
        }
    }

    pub(super) fn normalized(self) -> Self {
        let defaults = Self::default();
        Self {
            max_file_bytes: bounded_value(
                self.max_file_bytes,
                defaults.max_file_bytes,
                1,
                MAX_RUNTIME_LOG_FILE_BYTES,
            ),
            max_files: bounded_value(self.max_files, defaults.max_files, 1, MAX_RUNTIME_LOG_FILES),
            total_bytes: bounded_value(
                self.total_bytes,
                defaults.total_bytes,
                1,
                MAX_RUNTIME_LOG_TOTAL_BYTES,
            ),
            max_age_seconds: bounded_value(
                self.max_age_seconds,
                defaults.max_age_seconds,
                1,
                MAX_RUNTIME_LOG_AGE_SECONDS,
            ),
            record_to_disk: self.record_to_disk,
        }
    }
}

pub(super) fn runtime_log_recording_enabled() -> bool {
    std::env::var(RUNTIME_LOG_RECORD_ENV)
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

#[derive(Debug, Default)]
pub(super) struct RuntimeLogWriterState {
    active_paths: BTreeMap<PathBuf, PathBuf>,
    active_sizes: BTreeMap<PathBuf, u64>,
    active_locks: BTreeMap<PathBuf, fs::File>,
}

struct RuntimeLogDirectoryLock {
    file: fs::File,
}

impl RuntimeLogDirectoryLock {
    fn acquire(dir: &Path) -> io::Result<Self> {
        let file = open_runtime_log_lock_file(&dir.join(RUNTIME_LOG_DIRECTORY_LOCK_FILE))?;
        file.lock_exclusive()?;
        Ok(Self { file })
    }

    fn try_acquire(dir: &Path) -> io::Result<Option<Self>> {
        let file = open_runtime_log_lock_file(&dir.join(RUNTIME_LOG_DIRECTORY_LOCK_FILE))?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(Self { file })),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl Drop for RuntimeLogDirectoryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl RuntimeLogWriterState {
    fn ensure_active_path(&mut self, requested_path: &Path) -> io::Result<PathBuf> {
        if let Some(path) = self.active_paths.get(requested_path) {
            return Ok(path.clone());
        }
        let metadata = fs::symlink_metadata(requested_path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::other("runtime log path is not a regular file"));
        }
        let lock = acquire_runtime_log_path_lock(requested_path)?;
        let active_path = requested_path.to_path_buf();
        self.active_sizes
            .insert(active_path.clone(), metadata.len());
        self.active_locks.insert(active_path.clone(), lock);
        self.active_paths
            .insert(requested_path.to_path_buf(), active_path.clone());
        Ok(active_path)
    }

    fn active_size(&mut self, active_path: &Path) -> io::Result<u64> {
        if let Some(size) = self.active_sizes.get(active_path).copied() {
            return Ok(size);
        }
        let size = fs::symlink_metadata(active_path)?.len();
        self.active_sizes.insert(active_path.to_path_buf(), size);
        Ok(size)
    }

    fn rotate(&mut self, requested_path: &Path, active_path: &Path) -> io::Result<PathBuf> {
        let Some(parent) = active_path.parent() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime log path has no parent directory",
            ));
        };
        let next_path = create_rotated_runtime_log_path(parent)?;
        let next_lock = match acquire_runtime_log_path_lock(&next_path) {
            Ok(lock) => lock,
            Err(error) => {
                let _ = fs::remove_file(&next_path);
                return Err(error);
            }
        };
        drop(self.active_locks.remove(active_path));
        self.active_sizes.remove(active_path);
        self.active_sizes.insert(next_path.clone(), 0);
        self.active_locks.insert(next_path.clone(), next_lock);
        self.active_paths
            .insert(requested_path.to_path_buf(), next_path.clone());
        let _ = write_runtime_latest_log_pointer(&next_path);
        Ok(next_path)
    }
}

pub(super) fn write_log_line(
    inner: &RuntimeAsyncLoggerInner,
    requested_path: &Path,
    line: &str,
) -> io::Result<()> {
    inner.live.append(requested_path, line);
    if !inner.policy.record_to_disk {
        return Ok(());
    }
    let Some(log_dir) = requested_path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime log path has no parent directory",
        ));
    };
    let _directory_lock = RuntimeLogDirectoryLock::acquire(log_dir)?;
    let mut writer = inner
        .writer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut active_path = writer.ensure_active_path(requested_path)?;
    let line_len = u64::try_from(line.len()).unwrap_or(u64::MAX);
    let current_size = writer.active_size(&active_path)?;
    let mut rotated = false;
    if current_size > 0 && current_size.saturating_add(line_len) > inner.policy.max_file_bytes {
        active_path = writer.rotate(requested_path, &active_path)?;
        rotated = true;
    }

    let mut file = open_runtime_log_for_append(&active_path)?;
    file.write_all(line.as_bytes())?;
    drop(file);
    let next_size = writer.active_size(&active_path)?.saturating_add(line_len);
    writer.active_sizes.insert(active_path.clone(), next_size);

    if line_len > inner.policy.max_file_bytes && writer.rotate(requested_path, &active_path).is_ok()
    {
        rotated = true;
    }
    if rotated {
        let active_paths = writer.active_paths.values().cloned().collect::<Vec<_>>();
        let _ = cleanup_runtime_log_directory_locked(
            log_dir,
            SystemTime::now(),
            inner.policy,
            &active_paths,
            RUNTIME_LOG_FILE_PREFIX,
        );
    }
    Ok(())
}

pub fn cleanup_runtime_log_directory(
    dir: &Path,
    now: SystemTime,
    policy: RuntimeLogPolicy,
) -> RuntimeLogCleanupReport {
    cleanup_runtime_log_directory_with_prefix(dir, now, policy, RUNTIME_LOG_FILE_PREFIX)
}

pub fn cleanup_runtime_log_directory_with_prefix(
    dir: &Path,
    now: SystemTime,
    policy: RuntimeLogPolicy,
    log_prefix: &str,
) -> RuntimeLogCleanupReport {
    let policy = policy.normalized();
    let _lock = match RuntimeLogDirectoryLock::try_acquire(dir) {
        Ok(Some(lock)) => lock,
        Ok(None) => return RuntimeLogCleanupReport::default(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return RuntimeLogCleanupReport::default();
        }
        Err(_) => {
            return RuntimeLogCleanupReport {
                scan_failures: 1,
                ..RuntimeLogCleanupReport::default()
            };
        }
    };
    cleanup_runtime_log_directory_locked(dir, now, policy, &[], log_prefix)
}

#[derive(Debug)]
struct RuntimeLogFileEntry {
    path: PathBuf,
    size: u64,
    modified_epoch_seconds: i64,
}

fn open_runtime_log_lock_file(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
    }
    options.open(path)
}

fn runtime_log_path_lock_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-log");
    path.with_file_name(format!("{name}{RUNTIME_LOG_ACTIVE_LOCK_SUFFIX}"))
}

fn acquire_runtime_log_path_lock(path: &Path) -> io::Result<fs::File> {
    let lock = open_runtime_log_lock_file(&runtime_log_path_lock_path(path))?;
    lock.try_lock_exclusive().map_err(|error| {
        if error.kind() == io::ErrorKind::WouldBlock {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "runtime log path is already owned by a live writer",
            )
        } else {
            error
        }
    })?;
    Ok(lock)
}

fn open_runtime_log_for_append(path: &Path) -> io::Result<fs::File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other("runtime log path is not a regular file"));
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

fn create_rotated_runtime_log_path(dir: &Path) -> io::Result<PathBuf> {
    for _ in 0..128 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = RUNTIME_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(
            "{RUNTIME_LOG_FILE_PREFIX}-{}-{timestamp}-{sequence}.log",
            std::process::id()
        ));
        match create_runtime_log_file(&path) {
            Ok(file) => {
                drop(file);
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "failed to allocate a rotated runtime log path",
    ))
}

fn create_runtime_log_file(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn write_runtime_latest_log_pointer(log_path: &Path) -> io::Result<()> {
    let Some(dir) = log_path.parent() else {
        return Ok(());
    };
    let pointer = dir.join(RUNTIME_LOG_LATEST_POINTER_FILE);
    let sequence = RUNTIME_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = pointer.with_file_name(format!(
        "{RUNTIME_LOG_LATEST_POINTER_FILE}.{}.{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        sequence
    ));
    let mut file = create_runtime_log_file(&temp)?;
    file.write_all(format!("{}\n", log_path.display()).as_bytes())?;
    drop(file);
    if let Err(error) = fs::rename(&temp, &pointer) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

fn cleanup_runtime_log_directory_locked(
    dir: &Path,
    now: SystemTime,
    policy: RuntimeLogPolicy,
    protected_paths: &[PathBuf],
    log_prefix: &str,
) -> RuntimeLogCleanupReport {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return RuntimeLogCleanupReport::default();
        }
        Err(_) => {
            return RuntimeLogCleanupReport {
                scan_failures: 1,
                ..RuntimeLogCleanupReport::default()
            };
        }
    };
    let mut report = RuntimeLogCleanupReport::default();
    let mut logs = Vec::new();
    let mut lock_paths = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            report.scan_failures += 1;
            continue;
        };
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with(log_prefix) && name.ends_with(RUNTIME_LOG_ACTIVE_LOCK_SUFFIX) {
            lock_paths.push(path);
            continue;
        }
        if !(name.starts_with(log_prefix) && name.ends_with(".log")) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            report.scan_failures += 1;
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        let Some(modified_epoch_seconds) = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        else {
            report.scan_failures += 1;
            continue;
        };
        logs.push(RuntimeLogFileEntry {
            path,
            size: metadata.len(),
            modified_epoch_seconds,
        });
    }
    logs.sort_by(|left, right| {
        left.modified_epoch_seconds
            .cmp(&right.modified_epoch_seconds)
            .then_with(|| left.path.cmp(&right.path))
    });
    let protected_paths = protected_paths.iter().collect::<BTreeSet<_>>();
    let now_epoch = now
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX);
    let oldest_allowed = now_epoch.saturating_sub(policy.max_age_seconds);
    let mut removed_paths = BTreeSet::new();
    let mut total_bytes = logs.iter().map(|log| log.size).sum::<u64>();
    let mut remaining_count = logs.len();

    for log in &logs {
        if log.modified_epoch_seconds < oldest_allowed
            && !protected_paths.contains(&log.path)
            && !runtime_log_path_is_active(&log.path)
            && remove_runtime_log_file(log, &mut report, &mut total_bytes, &mut remaining_count)
        {
            removed_paths.insert(log.path.clone());
        }
    }
    for log in &logs {
        if remaining_count <= policy.max_files && total_bytes <= policy.total_bytes {
            break;
        }
        if removed_paths.contains(&log.path)
            || protected_paths.contains(&log.path)
            || runtime_log_path_is_active(&log.path)
        {
            continue;
        }
        if remove_runtime_log_file(log, &mut report, &mut total_bytes, &mut remaining_count) {
            removed_paths.insert(log.path.clone());
        }
    }

    for lock_path in lock_paths {
        let Some(log_path) = lock_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(RUNTIME_LOG_ACTIVE_LOCK_SUFFIX))
            .map(|name| dir.join(name))
        else {
            continue;
        };
        if !log_path.exists() && !runtime_log_path_is_active_from_lock(&lock_path) {
            match fs::remove_file(lock_path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => report.delete_failures += 1,
            }
        }
    }
    report
}

fn runtime_log_path_is_active(path: &Path) -> bool {
    runtime_log_path_is_active_from_lock(&runtime_log_path_lock_path(path))
}

fn runtime_log_path_is_active_from_lock(lock_path: &Path) -> bool {
    let Ok(lock) = open_runtime_log_lock_file(lock_path) else {
        return true;
    };
    lock.try_lock_exclusive().is_err()
}

fn remove_runtime_log_file(
    log: &RuntimeLogFileEntry,
    report: &mut RuntimeLogCleanupReport,
    total_bytes: &mut u64,
    remaining_count: &mut usize,
) -> bool {
    match fs::remove_file(&log.path) {
        Ok(()) => {
            report.removed += 1;
            *total_bytes = total_bytes.saturating_sub(log.size);
            *remaining_count = remaining_count.saturating_sub(1);
            match fs::remove_file(runtime_log_path_lock_path(&log.path)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => report.delete_failures += 1,
            }
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => {
            report.delete_failures += 1;
            false
        }
    }
}

fn bounded_environment_u64(name: &str, default: u64, min: u64, max: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(default, |value| bounded_value(value, default, min, max))
}

fn bounded_environment_usize(name: &str, default: usize, min: usize, max: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(default, |value| bounded_value(value, default, min, max))
}

fn bounded_environment_i64(name: &str, default: i64, min: i64, max: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(default, |value| bounded_value(value, default, min, max))
}

fn bounded_value<T: Ord + Copy>(value: T, default: T, min: T, max: T) -> T {
    if value >= min && value <= max {
        value
    } else {
        default
    }
}
