use super::{
    FollowedLog, LOG_FOLLOW_MAX_FILES, collect_new_followed_lines, runtime_log_paths_for_follow,
};
use crate::TestEnvVarGuard;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
fn open_fd_count() -> usize {
    fs::read_dir("/proc/self/fd")
        .expect("the test process should expose /proc/self/fd")
        .count()
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
