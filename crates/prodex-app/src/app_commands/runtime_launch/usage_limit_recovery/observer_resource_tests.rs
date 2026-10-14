use super::*;

fn append_recoverable_failure(path: &Path) {
    append_user_event(path);
    writeln!(
        fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{}",
        serde_json::json!({"type":"event_msg","payload":{"type":"task_complete",
            "error":{"message":"synthetic capacity error","codex_error_info":"server_overloaded"}}})
    )
    .unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn monitor_descriptor_pressure_preserves_child_and_cursor() {
    const CHILD: &str = "PRODEX_MONITOR_DESCRIPTOR_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "monitor_descriptor_pressure_preserves_child_and_cursor",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let root = fixture_root("observer-descriptor-pressure");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    let marker = root.join("owned.id");
    fs::write(&marker, SESSION_ID).unwrap();
    let mut monitor = GoalUsageLimitMonitor::new(paths, None, marker, Some(SESSION_ID.to_string()));
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    let cursor = monitor.session_scan_offset;
    append_recoverable_failure(&session);
    let mut original = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut original) },
        0
    );
    let restricted = libc::rlimit {
        rlim_cur: original.rlim_cur.min(96),
        rlim_max: original.rlim_max,
    };
    assert_eq!(
        unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &restricted) },
        0
    );
    let mut descriptors = Vec::new();
    let exhaustion = loop {
        match fs::File::open("/dev/null") {
            Ok(file) => descriptors.push(file),
            Err(error) => break error,
        }
    };
    let signal = monitor.take_usage_limit_signal();
    drop(descriptors);
    assert_eq!(
        unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &original) },
        0
    );
    assert_eq!(exhaustion.raw_os_error(), Some(libc::EMFILE));
    assert_eq!(
        signal.unwrap(),
        None,
        "observer pressure must not request child termination"
    );
    assert_eq!(monitor.session_scan_offset, cursor);
    assert_eq!(monitor.session_id.as_deref(), Some(SESSION_ID));
    assert!(monitor.observer_retry_at.is_some());
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap(),
        None,
        "resource retry must back off"
    );
    monitor.observer_retry_at = Some(Instant::now());
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    monitor.acknowledge_in_place_recovery();
    let count = || fs::read_dir("/proc/self/fd").unwrap().count();
    let baseline = count();
    for _ in 0..256 {
        assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    }
    assert!(
        count() <= baseline + 1,
        "repeated monitor polls must release descriptors"
    );
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unavailable_goal_database_does_not_consume_terminal_failure() {
    let root = fixture_root("observer-database-retry");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    let db_path = paths.shared_codex_root.join("goals_1.sqlite");
    fs::create_dir(&db_path).unwrap();
    let mut monitor = GoalUsageLimitMonitor::new(
        paths,
        Some(db_path.clone()),
        root.join("owned.id"),
        Some(SESSION_ID.to_string()),
    );
    let cursor = monitor.session_scan_offset;
    append_recoverable_failure(&session);
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert_eq!(
        monitor.session_scan_offset, cursor,
        "failed DB observation must not skip terminal record"
    );
    fs::remove_dir(&db_path).unwrap();
    let db = rusqlite::Connection::open(&db_path).unwrap();
    db.execute_batch("CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, status TEXT NOT NULL, updated_at_ms INTEGER NOT NULL);").unwrap();
    db.execute(
        "INSERT INTO thread_goals VALUES (?1, 'blocked', 1)",
        [SESSION_ID],
    )
    .unwrap();
    drop(db);
    monitor.observer_retry_at = Some(Instant::now());
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    monitor.acknowledge_in_place_recovery();
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}
