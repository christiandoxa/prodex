use super::{
    FollowedLog, LOG_FOLLOW_MAX_FILES, bounded_followed_log_paths, collect_new_followed_lines,
    runtime_log_paths_for_follow,
};
use crate::TestEnvVarGuard;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
#[cfg(unix)]
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
fn open_fd_count() -> usize {
    fs::read_dir("/proc/self/fd")
        .expect("the test process should expose /proc/self/fd")
        .count()
}

fn exercise_combined_follow_budget(root: &std::path::Path, _assert_low_rlimit: bool) {
    let runtime_paths = (0..64)
        .map(|index| root.join(format!("{index:03}-runtime.log")))
        .collect::<Vec<_>>();
    let session_paths = (0..64)
        .map(|index| root.join(format!("{index:03}-session.jsonl")))
        .collect::<Vec<_>>();
    for path in runtime_paths.iter().chain(&session_paths) {
        fs::write(path, b"event\n").expect("log fixture should be written");
    }

    let (runtime_paths, session_paths) = bounded_followed_log_paths(&runtime_paths, &session_paths);
    assert_eq!(
        runtime_paths.len() + session_paths.len(),
        LOG_FOLLOW_MAX_FILES
    );
    assert!(!runtime_paths.is_empty());
    assert!(!session_paths.is_empty());

    #[cfg(unix)]
    let baseline_fds = open_fd_count();
    let mut followed_runtime_logs = BTreeMap::<PathBuf, FollowedLog>::new();
    let mut followed_session_logs = BTreeMap::<PathBuf, FollowedLog>::new();
    for (paths, followed) in [
        (&runtime_paths, &mut followed_runtime_logs),
        (&session_paths, &mut followed_session_logs),
    ] {
        for path in paths {
            let state = followed
                .entry(path.clone())
                .or_insert_with(|| FollowedLog::with_offset(0));
            assert_eq!(
                collect_new_followed_lines(path, state).expect("log should be readable"),
                ["event"]
            );
        }
    }

    #[cfg(unix)]
    {
        assert!(open_fd_count() <= baseline_fds + LOG_FOLLOW_MAX_FILES + 1);
        if _assert_low_rlimit {
            assert!(
                open_fd_count() < 64,
                "combined log followers exceeded RLIMIT_NOFILE"
            );
        }
    }
    drop(followed_runtime_logs);
    drop(followed_session_logs);
}

#[test]
fn runtime_log_follow_is_bounded_across_repeated_reads() {
    let root = std::env::temp_dir().join(format!(
        "prodex-log-descriptor-regression-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("runtime log test directory should be created");
    let _log_dir = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        root.to_str().expect("runtime log test path should be utf8"),
    );
    for index in 0..128 {
        fs::write(
            root.join(format!("prodex-runtime-regression-{index:04}.log")),
            b"event\n",
        )
        .expect("runtime log fixture should be written");
    }

    let paths = runtime_log_paths_for_follow();
    assert_eq!(paths.len(), LOG_FOLLOW_MAX_FILES);

    #[cfg(unix)]
    let baseline_fds = open_fd_count();
    for _ in 0..3 {
        let mut followed = BTreeMap::<PathBuf, FollowedLog>::new();
        for path in &paths {
            let state = followed
                .entry(path.clone())
                .or_insert_with(|| FollowedLog::with_offset(0));
            assert_eq!(
                collect_new_followed_lines(path, state).expect("runtime log should be readable"),
                ["event"]
            );
        }
        #[cfg(unix)]
        assert!(open_fd_count() <= baseline_fds + LOG_FOLLOW_MAX_FILES + 1);
        drop(followed);
        #[cfg(unix)]
        assert!(open_fd_count() <= baseline_fds + 1);
    }

    fs::remove_dir_all(root).expect("runtime log test directory should be removed");
}

#[test]
fn runtime_and_session_followers_share_one_descriptor_budget() {
    let root = std::env::temp_dir().join(format!(
        "prodex-log-combined-descriptor-regression-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("combined log test directory should be created");
    exercise_combined_follow_budget(&root, false);
    fs::remove_dir_all(root).expect("combined log test directory should be removed");
}

#[cfg(unix)]
#[test]
fn combined_follow_budget_low_rlimit_subprocess() {
    let output = Command::new("sh")
        .args([
            "-c",
            "ulimit -n 64; exec \"$1\" combined_follow_budget_low_rlimit_child --nocapture",
            "prodex-log-fd-regression",
        ])
        .arg(std::env::current_exe().expect("test executable should be available"))
        .output()
        .expect("low-RLIMIT log regression subprocess should start");
    assert!(
        output.status.success(),
        "low-RLIMIT child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn combined_follow_budget_low_rlimit_child() {
    let root = std::env::temp_dir().join(format!(
        "prodex-log-low-rlimit-regression-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("low-RLIMIT log test directory should be created");
    exercise_combined_follow_budget(&root, true);
    fs::remove_dir_all(root).expect("low-RLIMIT log test directory should be removed");
}
