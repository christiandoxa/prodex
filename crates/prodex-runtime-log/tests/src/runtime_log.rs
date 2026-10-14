use super::*;

fn dropped_marker(marker: RuntimeDroppedLogMarker) -> String {
    format!("dropped={}\n", marker.dropped_count)
}

fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "prodex-runtime-log-{}-{}-{name}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

fn runtime_log_path(root: &Path, name: &str) -> PathBuf {
    root.join(format!("{RUNTIME_LOG_FILE_PREFIX}-{name}.log"))
}

fn test_policy(max_file_bytes: u64, max_files: usize, total_bytes: u64) -> RuntimeLogPolicy {
    RuntimeLogPolicy {
        max_file_bytes,
        max_files,
        total_bytes,
        max_age_seconds: DEFAULT_RUNTIME_LOG_MAX_AGE_SECONDS,
        record_to_disk: true,
    }
}

fn create_empty_log(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, []).unwrap();
}

fn set_log_modified(path: &Path, modified: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
}

fn remove_test_root(root: &Path) {
    for _ in 0..100 {
        match fs::remove_dir_all(root) {
            Ok(()) => return,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return,
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn json_log_format_uses_typed_event_fields() {
    let line = runtime_format_log_line(
        r#"stream_read_error request=7 error="failed with spaces" empty="""#,
        RuntimeLogFormat::Json,
        "2026-05-12T00:00:00Z",
        42,
    );
    let value: serde_json::Value = match serde_json::from_str(line.trim()) {
        Ok(value) => value,
        Err(err) => panic!("failed to parse json log line: {err}; line={line:?}"),
    };

    assert_eq!(value["timestamp"], "2026-05-12T00:00:00Z");
    assert_eq!(value["pid"], 42);
    assert_eq!(value["event"], "stream_read_error");
    assert_eq!(value["fields"]["request"], "7");
    assert_eq!(value["fields"]["error"], "failed with spaces");
    assert_eq!(value["fields"]["empty"], "");
}

#[test]
fn runtime_log_format_redacts_unstructured_messages_before_storage() {
    let line = runtime_format_log_line(
        "upstream failed Authorization: Bearer fixture-token api_key=fixture-key",
        RuntimeLogFormat::Text,
        "2026-05-12T00:00:00Z",
        42,
    );

    assert!(line.contains("Authorization: Bearer <redacted>"));
    assert!(line.contains("api_key=<redacted>"));
    assert!(!line.contains("fixture-token"));
    assert!(!line.contains("fixture-key"));
}

#[test]
fn mojo_runtime_log_render_preserves_exact_text_and_json_output() {
    let timestamp = "2026-10-13T10:45:00Z";
    let text = runtime_format_log_line("first\r\nsecond", RuntimeLogFormat::Text, timestamp, 123);
    assert_eq!(text, format!("[{timestamp}] first  second\n"));

    let json_line = runtime_format_log_line(
        r#"stream_read_error request=7 error="failed with spaces" note=東京"#,
        RuntimeLogFormat::Json,
        timestamp,
        123,
    );
    assert!(json_line.ends_with('\n'));
    let json: serde_json::Value =
        serde_json::from_str(&json_line).expect("Mojo emitted valid JSON");
    assert_eq!(json["timestamp"], timestamp);
    assert_eq!(json["pid"], 123);
    assert_eq!(json["event"], "stream_read_error");
    assert_eq!(json["fields"]["error"], "failed with spaces");
    assert_eq!(json["fields"]["request"], "7");
    assert_eq!(json["fields"]["note"], "東京");
}

#[test]
fn routine_load_telemetry_is_distinguished_from_failures() {
    assert!(runtime_log_message_is_routine_load(
        "profile_inflight_saturated profile=main active=8 hard_limit=8"
    ));
    assert!(!runtime_log_message_is_routine_load(
        "runtime_proxy_queue_overloaded lane=responses reason=long_lived_queue_full"
    ));
    assert!(!runtime_log_message_is_routine_load(
        "profile_auth_recovery_failed profile=main"
    ));
}

#[test]
fn routine_load_telemetry_classification_is_required_mojo_and_input_sensitive() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    for event in [
        "profile_inflight_saturated",
        "runtime_proxy_active_limit_reached",
        "runtime_proxy_lane_limit_reached",
    ] {
        assert!(runtime_log_message_is_routine_load(event));
    }
    for event in [
        "profile_inflight",
        "runtime_proxy_lane_limit_reached_extra",
        "profile_inflight_saturatedly",
    ] {
        assert!(!runtime_log_message_is_routine_load(event));
    }
    assert!(!runtime_log_message_is_routine_load(&"x".repeat(257)));
}

#[test]
fn runtime_log_rotates_complete_records_by_bytes() {
    let root = test_path("rotate");
    fs::create_dir_all(&root).unwrap();
    let path = runtime_log_path(&root, "active");
    create_empty_log(&path);
    let logger =
        RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(5, 10, 1024)).unwrap();

    logger.try_enqueue(&path, "1234\n".to_string());
    logger.flush_path(&path).unwrap();
    logger.try_enqueue(&path, "next\n".to_string());
    logger.flush_path(&path).unwrap();

    let mut logs = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("log"))
        .collect::<Vec<_>>();
    logs.sort();
    assert_eq!(logs.len(), 2);
    assert_eq!(fs::read_to_string(&path).unwrap(), "1234\n");
    let rotated = logs.iter().find(|candidate| *candidate != &path).unwrap();
    assert_eq!(fs::read_to_string(rotated).unwrap(), "next\n");
    assert!(logs.iter().all(|path| {
        fs::read(path)
            .unwrap()
            .last()
            .is_some_and(|byte| *byte == b'\n')
    }));

    drop(logger);
    remove_test_root(&root);
}

#[test]
fn oversized_runtime_log_record_is_written_once_then_rotated() {
    let root = test_path("oversized");
    fs::create_dir_all(&root).unwrap();
    let path = runtime_log_path(&root, "active");
    create_empty_log(&path);
    let logger =
        RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(4, 10, 1024)).unwrap();

    logger.try_enqueue(&path, "oversized\n".to_string());
    logger.flush_path(&path).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "oversized\n");
    logger.try_enqueue(&path, "ok\n".to_string());
    logger.flush_path(&path).unwrap();

    let logs = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("log"))
        .count();
    assert_eq!(logs, 2);
    assert!(
        fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("log"))
            .any(|path| fs::read_to_string(path).unwrap() == "ok\n")
    );

    drop(logger);
    remove_test_root(&root);
}

#[test]
fn total_runtime_log_budget_removes_old_inactive_files_but_keeps_active() {
    let root = test_path("budget");
    fs::create_dir_all(&root).unwrap();
    let active = runtime_log_path(&root, "z-active");
    create_empty_log(&active);
    let active_logger =
        RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(1024, 8, 1024)).unwrap();
    active_logger.try_enqueue(&active, "zlive\n".to_string());
    active_logger.flush_path(&active).unwrap();

    let cutoff = runtime_log_path(&root, "b-cutoff");
    create_empty_log(&cutoff);
    let cutoff_logger =
        RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(1024, 8, 1024)).unwrap();
    cutoff_logger.try_enqueue(&cutoff, "ok\n".to_string());
    cutoff_logger.flush_path(&cutoff).unwrap();

    let expired = runtime_log_path(&root, "a-expired");
    let oldest = runtime_log_path(&root, "c-oldest");
    let middle = runtime_log_path(&root, "d-middle");
    let newest = runtime_log_path(&root, "e-kept");
    fs::write(&expired, "x\n").unwrap();
    fs::write(&oldest, "aaaa").unwrap();
    fs::write(&middle, "bbbbb").unwrap();
    fs::write(&newest, "ccccccc").unwrap();
    let timestamp = UNIX_EPOCH + Duration::from_secs(500_000);
    for path in [&active, &oldest, &middle, &newest] {
        set_log_modified(path, timestamp);
    }
    set_log_modified(&cutoff, UNIX_EPOCH + Duration::from_secs(400_000));
    set_log_modified(&expired, UNIX_EPOCH + Duration::from_secs(399_999));

    let policy = RuntimeLogPolicy {
        max_file_bytes: 1024,
        max_files: 3,
        total_bytes: 16,
        max_age_seconds: 600_000,
        record_to_disk: true,
    };
    let report =
        cleanup_runtime_log_directory(&root, UNIX_EPOCH + Duration::from_secs(1_000_000), policy);

    assert_eq!(report.removed, 3);
    assert_eq!(report.scan_failures, 0);
    assert_eq!(report.delete_failures, 0);
    let mut remaining_names = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("log"))
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_string())
        .collect::<Vec<_>>();
    remaining_names.sort();
    assert_eq!(
        remaining_names,
        [
            "prodex-runtime-b-cutoff.log",
            "prodex-runtime-e-kept.log",
            "prodex-runtime-z-active.log",
        ]
    );
    let total = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("log"))
        .map(|path| fs::metadata(path).unwrap().len())
        .sum::<u64>();
    assert_eq!(total, 16);

    drop(cutoff_logger);
    drop(active_logger);
    remove_test_root(&root);
}

#[test]
fn invalid_runtime_log_limits_fall_back_to_bounded_defaults() {
    assert_eq!(
        RuntimeLogPolicy {
            max_file_bytes: 0,
            max_files: 0,
            total_bytes: 0,
            max_age_seconds: 0,
            record_to_disk: false,
        }
        .normalized(),
        RuntimeLogPolicy::default()
    );
}

#[test]
fn live_logging_does_not_create_a_disk_journal_by_default() {
    let root = test_path("live-only");
    fs::create_dir_all(&root).unwrap();
    let path = runtime_log_path(&root, "active");
    let logger = RuntimeAsyncLogger::new_with_recording(4, dropped_marker, false).unwrap();

    logger.try_enqueue(&path, "event\n".to_string());
    logger.flush_path(&path).unwrap();

    assert!(!path.exists());
    let snapshot = logger.live_log_snapshot_after(&path, 0, 1);
    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].line, "event\n");

    drop(logger);
    remove_test_root(&root);
}

#[test]
fn async_logger_reports_missing_log_instead_of_creating_it() {
    let path = test_path("missing.log");
    let logger =
        RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(1024, 8, 1024)).unwrap();

    logger.try_enqueue(&path, "entry\n".to_string());
    let error = logger.flush_path(&path).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn async_logger_refuses_symlink_log_targets() {
    let target = test_path("target.log");
    let link = test_path("link.log");
    fs::write(&target, "original\n").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let logger =
        RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(1024, 8, 1024)).unwrap();

    logger.try_enqueue(&link, "entry\n".to_string());
    assert!(logger.flush_path(&link).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "original\n");

    let _ = fs::remove_file(link);
    let _ = fs::remove_file(target);
}
