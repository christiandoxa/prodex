use super::{
    RuntimeLogCleanupReport, RuntimeLogFileEntry, RuntimeLogPolicy, remove_runtime_log_file,
    runtime_log_is_removable,
};
use prodex_mojo_core::log_throughput_policy::{self as mojo_retention, LogRetentionCandidate};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn candidate(
    log: &RuntimeLogFileEntry,
    removable: bool,
    unavailable: bool,
) -> LogRetentionCandidate<'_> {
    LogRetentionCandidate {
        file_name: log
            .path
            .file_name()
            .expect("scanned runtime log has a file name")
            .as_encoded_bytes(),
        size: log.size,
        modified_epoch_seconds: log.modified_epoch_seconds,
        removable,
        unavailable,
    }
}

pub(super) fn remove_expired_runtime_logs(
    logs: &[RuntimeLogFileEntry],
    oldest_allowed: i64,
    protected_paths: &BTreeSet<&PathBuf>,
    removed_paths: &mut BTreeSet<PathBuf>,
    report: &mut RuntimeLogCleanupReport,
    total_bytes: &mut u64,
    remaining_count: &mut usize,
) {
    let candidates = logs
        .iter()
        .map(|log| {
            candidate(
                log,
                runtime_log_is_removable(&log.path, protected_paths),
                false,
            )
        })
        .collect::<Vec<_>>();
    let selected = mojo_retention::log_expired_candidate_plan(&candidates, oldest_allowed)
        .expect("Mojo runtime-log expiry candidates returned invalid output");
    for (log, selected) in logs.iter().zip(selected) {
        if selected && remove_runtime_log_file(log, report, total_bytes, remaining_count) {
            removed_paths.insert(log.path.clone());
        }
    }
}

pub(super) fn remove_over_budget_runtime_logs(
    logs: &[RuntimeLogFileEntry],
    policy: RuntimeLogPolicy,
    protected_paths: &BTreeSet<&PathBuf>,
    removed_paths: &mut BTreeSet<PathBuf>,
    report: &mut RuntimeLogCleanupReport,
    total_bytes: &mut u64,
    remaining_count: &mut usize,
) {
    let mut unavailable_paths = removed_paths.clone();
    loop {
        let candidates = logs
            .iter()
            .map(|log| {
                candidate(
                    log,
                    runtime_log_is_removable(&log.path, protected_paths),
                    unavailable_paths.contains(&log.path),
                )
            })
            .collect::<Vec<_>>();
        let selected = mojo_retention::log_over_budget_candidate_plan(
            &candidates,
            *remaining_count,
            policy.max_files,
            *total_bytes,
            policy.total_bytes,
        )
        .expect("Mojo runtime-log budget candidates returned invalid output");
        if selected.is_empty() {
            break;
        }
        let mut failed = false;
        for index in selected {
            let log = logs
                .get(index)
                .expect("Mojo runtime-log budget plan returned an invalid candidate index");
            if remove_runtime_log_file(log, report, total_bytes, remaining_count) {
                removed_paths.insert(log.path.clone());
                unavailable_paths.insert(log.path.clone());
            } else {
                unavailable_paths.insert(log.path.clone());
                failed = true;
                break;
            }
        }
        if !failed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn budget_selection_replans_after_an_oldest_file_cannot_be_removed() {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-log-replan-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let failed_path = root.join("prodex-runtime-a-fail.log");
        let removable_path = root.join("prodex-runtime-b-remove.log");
        fs::create_dir(&failed_path).unwrap();
        fs::write(&removable_path, b"12345").unwrap();
        let logs = [
            RuntimeLogFileEntry {
                path: removable_path.clone(),
                size: 5,
                modified_epoch_seconds: 1,
            },
            RuntimeLogFileEntry {
                path: failed_path.clone(),
                size: 5,
                modified_epoch_seconds: 1,
            },
        ];
        let policy = RuntimeLogPolicy {
            max_files: 1,
            total_bytes: 5,
            ..RuntimeLogPolicy::default()
        };
        let mut removed_paths = BTreeSet::new();
        let mut report = RuntimeLogCleanupReport::default();
        let mut total_bytes = 10;
        let mut remaining_count = 2;

        remove_over_budget_runtime_logs(
            &logs,
            policy,
            &BTreeSet::new(),
            &mut removed_paths,
            &mut report,
            &mut total_bytes,
            &mut remaining_count,
        );

        assert!(failed_path.is_dir());
        assert!(!removable_path.exists());
        assert_eq!(report.removed, 1);
        assert_eq!(report.delete_failures, 1);
        assert_eq!(total_bytes, 5);
        assert_eq!(remaining_count, 1);
        assert_eq!(removed_paths, BTreeSet::from([removable_path]));
        fs::remove_dir_all(root).unwrap();
    }
}
