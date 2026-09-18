use super::*;
use std::time::{Duration, SystemTime};

fn test_paths(name: &str) -> AppPaths {
    let root =
        std::env::temp_dir().join(format!("prodex-housekeeping-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
        root,
    }
}

#[test]
fn cleanup_summary_total_and_merge_count_all_fields() {
    let left = ProdexCleanupSummary {
        duplicate_profiles_removed: 1,
        runtime_logs_removed: 2,
        ..ProdexCleanupSummary::default()
    };
    let right = ProdexCleanupSummary {
        stale_login_dirs_removed: 4,
        dead_runtime_broker_registries_removed: 5,
        scan_failures: 2,
        delete_failures: 3,
        ..ProdexCleanupSummary::default()
    };

    let merged = left.merge(right);

    assert_eq!(merged.total_removed(), 12);
    assert_eq!(merged.runtime_logs_removed, 2);
    assert_eq!(merged.stale_login_dirs_removed, 4);
    assert_eq!(merged.failure_count(), 5);
}

#[test]
fn stale_login_cleanup_can_retry_after_delete_failure() {
    let paths = test_paths("stale-login-retry");
    fs::create_dir_all(&paths.managed_profiles_root).unwrap();
    let stale_login = paths.managed_profiles_root.join(".login-crashed");
    fs::create_dir_all(&stale_login).unwrap();
    let now = SystemTime::now()
        .checked_add(Duration::from_secs(2))
        .unwrap();

    let failed = cleanup_stale_login_dirs_at_with_counts(&paths, now, 1, |_| false);
    assert_eq!(failed.removed, 0);
    assert_eq!(failed.delete_failures, 1);
    assert!(stale_login.exists());
    assert_eq!(
        cleanup_stale_login_dirs_at(&paths, now, 1, |path| fs::remove_dir_all(path).is_ok()),
        1
    );
    assert!(!stale_login.exists());
}

#[test]
fn cleanup_reports_scan_failures_without_deleting_ambiguous_paths() {
    let paths = test_paths("cleanup-failure-counts");
    fs::write(&paths.root, "not a directory").unwrap();

    let root_temp =
        cleanup_prodex_stale_root_temp_files_at_with_counts(&paths, SystemTime::now(), 1, |_| {
            false
        });
    assert_eq!(root_temp.removed, 0);
    assert_eq!(root_temp.scan_failures, 1);

    let logs = cleanup_runtime_proxy_logs_in_dir_with_counts(
        &paths.root,
        SystemTime::now(),
        1,
        1,
        "prodex-runtime-",
    );
    assert_eq!(logs.removed, 0);
    assert_eq!(logs.scan_failures, 1);
    assert!(paths.root.exists());
    fs::remove_file(&paths.root).unwrap();
}

#[test]
fn cleanup_counts_delete_failures_and_keeps_directories() {
    let paths = test_paths("cleanup-delete-failure");
    fs::create_dir_all(&paths.root).unwrap();
    let protected = paths.root.join("protected");
    fs::create_dir(&protected).unwrap();

    let report = cleanup_existing_files_under(&paths.root, [protected.clone()]);

    assert_eq!(report.counts().delete_failures, 1);
    assert!(protected.is_dir());
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn cleanup_report_preserves_missing_and_failure_details() {
    let paths = test_paths("cleanup-report");
    fs::create_dir_all(&paths.root).unwrap();
    let removed = paths.root.join("removed");
    let missing = paths.root.join("missing");
    let outside = paths.root.with_extension("outside");
    fs::write(&removed, "remove").unwrap();
    fs::write(&outside, "keep").unwrap();

    let report =
        cleanup_existing_files_under(&paths.root, [removed.clone(), missing, outside.clone()]);

    assert_eq!(report.removed, 1);
    assert_eq!(report.missing, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].path, outside);
    assert_eq!(
        report.failures[0].kind,
        ProdexCleanupFailureKind::OutsideRoot
    );
    assert!(!removed.exists());
    assert!(report.failures[0].path.exists());
    let _ = fs::remove_dir_all(paths.root);
    let _ = fs::remove_file(report.failures[0].path.clone());
}

#[cfg(unix)]
#[test]
fn cleanup_report_rejects_symlink_parent_escape() {
    let paths = test_paths("cleanup-report-symlink-parent");
    let outside = paths.root.with_extension("outside");
    fs::create_dir_all(&paths.root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret"), "keep").unwrap();
    std::os::unix::fs::symlink(&outside, paths.root.join("linked")).unwrap();

    let report = cleanup_existing_files_under(&paths.root, [paths.root.join("linked/secret")]);

    assert_eq!(report.removed, 0);
    assert_eq!(
        report.failures[0].kind,
        ProdexCleanupFailureKind::OutsideRoot
    );
    assert!(outside.join("secret").exists());
    let _ = fs::remove_dir_all(paths.root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn runtime_log_discovery_skips_symlink_log_files() {
    let paths = test_paths("runtime-log-symlink");
    fs::create_dir_all(&paths.root).expect("root should exist");
    let regular = paths.root.join("prodex-runtime-1.log");
    let symlink = paths.root.join("prodex-runtime-2.log");
    let target = paths.root.join("secret.txt");
    fs::write(&regular, "runtime\n").expect("regular log should write");
    fs::write(&target, "do not read\n").expect("target should write");
    std::os::unix::fs::symlink(&target, &symlink).expect("symlink should create");

    let logs = prodex_runtime_log_paths_in_dir(&paths.root, "prodex-runtime");

    assert_eq!(logs, vec![regular]);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn zero_orphan_retention_selects_fresh_orphan_managed_home() {
    let paths = test_paths("zero-retention");
    fs::create_dir_all(&paths.managed_profiles_root).expect("profiles root should exist");
    let orphan = paths.managed_profiles_root.join("fresh-orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan marker should write");

    let state = AppState::default();

    let orphans = collect_orphan_managed_profile_dirs_at(&paths, &state, SystemTime::now(), 0);

    assert_eq!(orphans, vec!["fresh-orphan"]);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}
