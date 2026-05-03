use super::*;

struct RuntimeProxyLogTestDir {
    path: PathBuf,
}

impl RuntimeProxyLogTestDir {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "prodex-runtime-log-test-{}-{}",
            std::process::id(),
            RUNTIME_PROXY_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("runtime proxy log test dir should exist");
        Self { path }
    }

    fn log_path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for RuntimeProxyLogTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct RuntimeProxyAsyncLoggerPauseGuard;

impl RuntimeProxyAsyncLoggerPauseGuard {
    fn pause() -> Self {
        runtime_proxy_async_logger_set_pause_writes(true);
        Self
    }
}

impl Drop for RuntimeProxyAsyncLoggerPauseGuard {
    fn drop(&mut self) {
        runtime_proxy_async_logger_set_pause_writes(false);
    }
}

#[test]
fn runtime_proxy_log_to_path_flushes_async_entries() {
    let _runtime_lock = acquire_test_runtime_lock();
    let dir = RuntimeProxyLogTestDir::new();
    let log_path = dir.log_path("async.log");

    let pause_guard = RuntimeProxyAsyncLoggerPauseGuard::pause();
    runtime_proxy_log_to_path(&log_path, "async entry line1\nline2 request=7");

    assert_eq!(
        runtime_proxy_async_logger().pending_count_for_path(&log_path),
        1,
        "queued entry should remain pending while worker writes are paused"
    );
    assert!(
        fs::read_to_string(&log_path).unwrap_or_default().is_empty(),
        "async logger should not synchronously write to disk on caller path"
    );

    drop(pause_guard);
    runtime_proxy_flush_logs_for_path(&log_path);

    let log = fs::read_to_string(&log_path).expect("async runtime log should flush to disk");
    assert!(log.contains("async entry line1 line2 request=7"));
    assert!(log.ends_with('\n'));
}

#[test]
fn runtime_proxy_log_to_path_marks_async_queue_drops_after_recovery() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _format = TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_FORMAT", "text");
    let dir = RuntimeProxyLogTestDir::new();
    let log_path = dir.log_path("dropped.log");
    let logger = runtime_proxy_async_logger();
    let capacity = logger.capacity();

    let pause_guard = RuntimeProxyAsyncLoggerPauseGuard::pause();
    for index in 0..capacity.saturating_add(64) {
        runtime_proxy_log_to_path(&log_path, &format!("queued entry index={index}"));
    }

    assert!(
        logger.pending_count_for_path(&log_path) >= capacity,
        "queued entries and drop marker should remain pending while writes are paused"
    );
    assert!(
        fs::read_to_string(&log_path).unwrap_or_default().is_empty(),
        "full async logger should not synchronously write drop markers on caller path"
    );

    drop(pause_guard);
    runtime_proxy_flush_logs_for_path(&log_path);

    let log = fs::read_to_string(&log_path).expect("async runtime log should flush to disk");
    let marker = log
        .lines()
        .find(|line| line.contains(RUNTIME_PROXY_DROPPED_LOG_EVENT))
        .expect("runtime log should include a dropped-log marker after recovery");
    let dropped_count = marker
        .split_whitespace()
        .find_map(|token| token.strip_prefix("dropped_count="))
        .and_then(|value| value.parse::<u64>().ok())
        .expect("drop marker should include a numeric dropped_count field");

    assert!(dropped_count > 0);
    assert!(marker.contains("reason=queue_full"));
    assert!(marker.contains("queue_capacity="));
    assert!(log.contains("queued entry index=0"));
}

#[test]
fn runtime_proxy_log_to_path_preserves_valid_json_format() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _format = TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_FORMAT", "json");
    let dir = RuntimeProxyLogTestDir::new();
    let log_path = dir.log_path("json.log");

    runtime_proxy_log_to_path(
        &log_path,
        &runtime_proxy_structured_log_message(
            "selection",
            [
                runtime_proxy_log_field("route", "responses"),
                runtime_proxy_log_field("profile", "main"),
                runtime_proxy_log_field("note", "hello next"),
                runtime_proxy_log_field("payload", "{\"kind\":\"sample\"}"),
            ],
        ),
    );
    runtime_proxy_flush_logs_for_path(&log_path);

    let line = fs::read_to_string(&log_path).expect("json runtime log should be readable");
    let value = serde_json::from_str::<serde_json::Value>(line.trim_end())
        .expect("runtime log line should be valid json");

    assert_eq!(
        value.get("message").and_then(|value| value.as_str()),
        Some(
            "selection route=responses profile=main note=\"hello next\" payload=\"{\\\"kind\\\":\\\"sample\\\"}\""
        )
    );
    assert_eq!(
        value.get("event").and_then(|value| value.as_str()),
        Some("selection")
    );
    assert_eq!(
        value
            .pointer("/fields/route")
            .and_then(|value| value.as_str()),
        Some("responses")
    );
    assert_eq!(
        value
            .pointer("/fields/profile")
            .and_then(|value| value.as_str()),
        Some("main")
    );
    assert_eq!(
        value
            .pointer("/fields/note")
            .and_then(|value| value.as_str()),
        Some("hello next")
    );
    assert_eq!(
        value
            .pointer("/fields/payload")
            .and_then(|value| value.as_str()),
        Some("{\"kind\":\"sample\"}")
    );
    assert_eq!(
        value.get("pid").and_then(|value| value.as_u64()),
        Some(std::process::id().into())
    );
}

#[test]
fn runtime_proxy_structured_log_message_quotes_spaced_field_values() {
    let message = runtime_proxy_structured_log_message(
        "dispatch_error",
        [
            runtime_proxy_log_field("request", "42"),
            runtime_proxy_log_field("transport", "http"),
            runtime_proxy_log_field("error", "failed with \"quoted\" value"),
            runtime_proxy_log_field("detail", "line1\nline2"),
            runtime_proxy_log_field("empty", ""),
        ],
    );

    assert_eq!(
        message,
        "dispatch_error request=42 transport=http error=\"failed with \\\"quoted\\\" value\" detail=\"line1 line2\" empty=\"\""
    );
    assert_eq!(runtime_proxy_log_event(&message), Some("dispatch_error"));

    let fields = runtime_proxy_log_fields(&message);
    assert_eq!(fields.get("request").map(String::as_str), Some("42"));
    assert_eq!(fields.get("transport").map(String::as_str), Some("http"));
    assert_eq!(
        fields.get("error").map(String::as_str),
        Some("failed with \"quoted\" value")
    );
    assert_eq!(
        fields.get("detail").map(String::as_str),
        Some("line1 line2")
    );
    assert_eq!(fields.get("empty").map(String::as_str), Some(""));
}

#[test]
fn runtime_proxy_format_log_line_sanitizes_text_format() {
    let line = runtime_proxy_format_log_line(
        "transport_failure profile=main\nroute=responses",
        RuntimeLogFormat::Text,
    );

    assert!(line.contains("transport_failure profile=main route=responses"));
    assert!(!line.contains('\r'));
    assert!(line.ends_with('\n'));
}
