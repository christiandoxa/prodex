use super::ProdexCleanupCounts;
use prodex_core::{runtime_proxy_log_file_name_is_owned, select_newest_modified_path};
use runtime_log::RuntimeLogPolicy;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn prodex_runtime_log_paths_in_dir(dir: &Path, log_prefix: &str) -> Vec<PathBuf> {
    prodex_runtime_log_paths_in_dir_with_counts(dir, log_prefix).0
}

pub fn prodex_runtime_log_paths_in_dir_with_counts(
    dir: &Path,
    log_prefix: &str,
) -> (Vec<PathBuf>, usize) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), 0),
        Err(_) => return (Vec::new(), 1),
    };
    let mut scan_failures = 0usize;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                scan_failures += 1;
                continue;
            }
        };
        let path = entry.path();
        let owned = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| runtime_proxy_log_file_name_is_owned(name, log_prefix));
        if !owned {
            continue;
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => {
                paths.push(path);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => scan_failures += 1,
        }
    }
    paths.sort();
    (paths, scan_failures)
}

pub fn cleanup_runtime_proxy_logs_in_dir(
    dir: &Path,
    now: SystemTime,
    retention_seconds: i64,
    retention_count: usize,
    log_prefix: &str,
) -> usize {
    cleanup_runtime_proxy_logs_in_dir_with_counts(
        dir,
        now,
        retention_seconds,
        retention_count,
        log_prefix,
    )
    .removed
}

pub fn cleanup_runtime_proxy_logs_in_dir_with_counts(
    dir: &Path,
    now: SystemTime,
    retention_seconds: i64,
    retention_count: usize,
    log_prefix: &str,
) -> ProdexCleanupCounts {
    let report = runtime_log::cleanup_runtime_log_directory_with_prefix(
        dir,
        now,
        RuntimeLogPolicy::from_environment_with_retention(retention_seconds, retention_count),
        log_prefix,
    );
    ProdexCleanupCounts {
        removed: report.removed,
        scan_failures: report.scan_failures,
        delete_failures: report.delete_failures,
    }
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
    cleanup_runtime_proxy_latest_pointer_with_counts(pointer_path).removed > 0
}

pub fn cleanup_runtime_proxy_latest_pointer_with_counts(
    pointer_path: &Path,
) -> ProdexCleanupCounts {
    let content = match fs::read_to_string(pointer_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ProdexCleanupCounts::default();
        }
        Err(_) => {
            return ProdexCleanupCounts {
                scan_failures: 1,
                ..ProdexCleanupCounts::default()
            };
        }
    };
    let target = PathBuf::from(content.trim());
    if target.as_os_str().is_empty() {
        return ProdexCleanupCounts::default();
    }
    match fs::symlink_metadata(&target) {
        Ok(_) => return ProdexCleanupCounts::default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return ProdexCleanupCounts {
                scan_failures: 1,
                ..ProdexCleanupCounts::default()
            };
        }
    }
    match fs::remove_file(pointer_path) {
        Ok(()) => ProdexCleanupCounts {
            removed: 1,
            ..ProdexCleanupCounts::default()
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ProdexCleanupCounts::default()
        }
        Err(_) => ProdexCleanupCounts {
            delete_failures: 1,
            ..ProdexCleanupCounts::default()
        },
    }
}
