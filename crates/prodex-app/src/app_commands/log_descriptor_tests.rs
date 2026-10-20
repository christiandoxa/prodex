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
fn run_isolated_fd_test(test_name: &str, env_key: &'static str) -> bool {
    if std::env::var_os(env_key).is_some() {
        return true;
    }
    let output =
        Command::new(std::env::current_exe().expect("test executable should be available"))
            .arg(test_name)
            .arg("--nocapture")
            .env(env_key, "1")
            .output()
            .expect("isolated descriptor regression subprocess should start");
    assert!(
        output.status.success(),
        "isolated descriptor child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

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
    #[cfg(unix)]
    if !run_isolated_fd_test(
        "runtime_log_follow_is_bounded_across_repeated_reads",
        "PRODEX_LOG_FD_RUNTIME_CHILD",
    ) {
        return;
    }
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
    #[cfg(unix)]
    if !run_isolated_fd_test(
        "runtime_and_session_followers_share_one_descriptor_budget",
        "PRODEX_LOG_FD_COMBINED_CHILD",
    ) {
        return;
    }
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
            "ulimit -n 64; PRODEX_LOG_LOW_RLIMIT_CHILD=1 exec \"$1\" combined_follow_budget_low_rlimit_child --nocapture",
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
    if std::env::var_os("PRODEX_LOG_LOW_RLIMIT_CHILD").is_none() {
        return;
    }
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

#[test]
fn compiled_log_follow_selection_reserves_slots_and_preserves_input_order() {
    let root = std::env::temp_dir().join(format!(
        "prodex-log-follow-selection-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let runtime = (0..40)
        .map(|index| root.join(format!("runtime-{index:02}.log")))
        .collect::<Vec<_>>();
    let sessions = (0..40)
        .map(|index| root.join(format!("session-{index:02}.jsonl")))
        .collect::<Vec<_>>();
    for (index, path) in runtime.iter().chain(sessions.iter()).enumerate() {
        fs::write(path, b"record\n").unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(
                UNIX_EPOCH
                    + std::time::Duration::from_secs(u64::try_from(index % 40 + 100).unwrap()),
            )
            .unwrap();
    }
    let (selected_runtime, selected_session) = bounded_followed_log_paths(&runtime, &sessions);
    assert_eq!(selected_runtime, runtime[24..].to_vec());
    assert_eq!(selected_session, sessions[24..].to_vec());
    assert_eq!(
        selected_runtime.len() + selected_session.len(),
        LOG_FOLLOW_MAX_FILES
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_log_follow_selection_retains_duplicated_paths_and_unused_source_slots() {
    let runtime = (0..64)
        .map(|index| PathBuf::from(format!("runtime-{index:03}.log")))
        .collect::<Vec<_>>();
    let session = (0..10)
        .map(|index| PathBuf::from(format!("session-{index:03}.jsonl")))
        .collect::<Vec<_>>();
    let (selected_runtime, selected_sessions) = bounded_followed_log_paths(&runtime, &session);
    assert_eq!(selected_runtime.len(), 22);
    assert_eq!(selected_sessions.len(), 10);
    assert_eq!(selected_runtime, runtime[..22].to_vec());
    assert_eq!(selected_sessions, session);

    let single = PathBuf::from("same-runtime.log");
    let duplicate_runtime = vec![single.clone(); 40];
    let two_sessions = vec![
        PathBuf::from("session-a.jsonl"),
        PathBuf::from("session-b.jsonl"),
    ];
    let (selected_runtime, selected_sessions) =
        bounded_followed_log_paths(&duplicate_runtime, &two_sessions);
    assert_eq!(
        selected_runtime, duplicate_runtime,
        "the old BTreeSet deduplicated slots, not the returned original rows",
    );
    assert_eq!(selected_sessions, two_sessions);

    let duplicate_within_capacity = vec![single; 32];
    let (selected_runtime, selected_sessions) =
        bounded_followed_log_paths(&duplicate_within_capacity, &[]);
    assert_eq!(selected_runtime, duplicate_within_capacity);
    assert!(selected_sessions.is_empty());
}
