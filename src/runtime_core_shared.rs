use super::*;

#[cfg(all(target_os = "linux", target_env = "gnu", not(test)))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

const RUNTIME_PROXY_DROPPED_LOG_EVENT: &str = runtime_log::RUNTIME_ASYNC_LOG_DROPPED_EVENT;

fn runtime_proxy_log_queue_capacity() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    runtime_proxy_log_queue_capacity_default(parallelism)
}

#[cfg(test)]
fn runtime_proxy_async_logger_set_pause_writes(paused: bool) {
    runtime_log::RuntimeAsyncLogger::set_pause_writes_for_test(paused);
}

#[cfg(test)]
fn runtime_proxy_async_logger_pause_writes() -> bool {
    runtime_log::runtime_async_logger_writes_are_paused_for_test()
}

fn runtime_proxy_async_logger() -> &'static runtime_log::RuntimeAsyncLogger {
    static LOGGER: OnceLock<runtime_log::RuntimeAsyncLogger> = OnceLock::new();
    LOGGER.get_or_init(|| {
        runtime_log::RuntimeAsyncLogger::new(
            runtime_proxy_log_queue_capacity(),
            runtime_proxy_format_dropped_log_marker,
        )
    })
}

pub(super) fn runtime_proxy_log_dir() -> PathBuf {
    env::var_os("PRODEX_RUNTIME_LOG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| runtime_policy_runtime().and_then(|policy| policy.log_dir))
        .unwrap_or_else(env::temp_dir)
}

pub(super) fn runtime_proxy_log_format() -> RuntimeLogFormat {
    env::var("PRODEX_RUNTIME_LOG_FORMAT")
        .ok()
        .and_then(|value| RuntimeLogFormat::parse(&value))
        .or_else(|| runtime_policy_runtime().and_then(|policy| policy.log_format))
        .unwrap_or(RuntimeLogFormat::Text)
}

pub(super) fn create_runtime_proxy_log_path() -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = RUNTIME_PROXY_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let dir = runtime_proxy_log_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join(format!(
        "{RUNTIME_PROXY_LOG_FILE_PREFIX}-{}-{millis}-{sequence}.log",
        std::process::id()
    ))
}

pub(super) fn runtime_proxy_latest_log_pointer_path() -> PathBuf {
    runtime_proxy_log_dir().join(RUNTIME_PROXY_LATEST_LOG_POINTER)
}

pub(super) fn initialize_runtime_proxy_log_path() -> PathBuf {
    cleanup_runtime_proxy_log_housekeeping();
    let log_path = create_runtime_proxy_log_path();
    let _ = fs::write(
        runtime_proxy_latest_log_pointer_path(),
        format!("{}\n", log_path.display()),
    );
    let (executable_path, executable_sha256) = runtime_current_binary_identity();
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy log initialized pid={} cwd={} prodex_version={} executable_path={} executable_sha256={}",
            std::process::id(),
            std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string()),
            runtime_current_prodex_version(),
            executable_path.unwrap_or_else(|| "-".to_string()),
            executable_sha256.unwrap_or_else(|| "-".to_string())
        ),
    );
    log_path
}

pub(super) fn runtime_proxy_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.worker_count),
        runtime_proxy_worker_count_default(parallelism),
    )
    .clamp(1, 64)
}

pub(super) fn runtime_proxy_long_lived_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.long_lived_worker_count),
        runtime_proxy_long_lived_worker_count_default(parallelism),
    )
    .clamp(1, 256)
}

pub(super) fn runtime_probe_refresh_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.probe_refresh_worker_count),
        runtime_probe_refresh_worker_count_default(parallelism),
    )
    .clamp(1, 8)
}

pub(super) fn runtime_proxy_async_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.async_worker_count),
        runtime_proxy_async_worker_count_default(parallelism),
    )
    .clamp(2, 8)
}

pub(super) fn runtime_proxy_long_lived_queue_capacity(worker_count: usize) -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY",
        runtime_policy_proxy().and_then(|policy| policy.long_lived_queue_capacity),
        runtime_proxy_long_lived_queue_capacity_default(worker_count),
    )
    .max(1)
}

pub(super) fn runtime_proxy_active_request_limit(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT",
        runtime_policy_proxy().and_then(|policy| policy.active_request_limit),
        runtime_proxy_active_request_limit_default(worker_count, long_lived_worker_count),
    )
    .max(1)
}

fn runtime_heap_trim_last_requested_at_ms() -> &'static AtomicU64 {
    static LAST_REQUESTED_AT_MS: OnceLock<AtomicU64> = OnceLock::new();
    LAST_REQUESTED_AT_MS.get_or_init(|| AtomicU64::new(0))
}

#[cfg(test)]
fn runtime_heap_trim_request_count_cell() -> &'static AtomicUsize {
    static REQUEST_COUNT: OnceLock<AtomicUsize> = OnceLock::new();
    REQUEST_COUNT.get_or_init(|| AtomicUsize::new(0))
}

fn runtime_heap_trim_reserve(now_ms: u64) -> bool {
    let cell = runtime_heap_trim_last_requested_at_ms();
    let min_interval_ms = std::num::NonZeroU64::new(RUNTIME_PROXY_HEAP_TRIM_MIN_INTERVAL_MS);
    loop {
        let previous = cell.load(Ordering::Relaxed);
        if let Some(min_interval_ms) = min_interval_ms
            && previous > 0
            && now_ms.saturating_sub(previous) < min_interval_ms.get()
        {
            return false;
        }
        match cell.compare_exchange(previous, now_ms, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(_) => continue,
        }
    }
}

pub(crate) fn runtime_maybe_trim_process_heap(released_bytes: usize) -> bool {
    if released_bytes < RUNTIME_PROXY_HEAP_TRIM_MIN_RELEASE_BYTES {
        return false;
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if !runtime_heap_trim_reserve(now_ms) {
        return false;
    }

    #[cfg(test)]
    {
        runtime_heap_trim_request_count_cell().fetch_add(1, Ordering::SeqCst);
        true
    }

    #[cfg(all(target_os = "linux", target_env = "gnu", not(test)))]
    unsafe {
        let _ = malloc_trim(0);
        true
    }

    #[cfg(not(any(test, all(target_os = "linux", target_env = "gnu"))))]
    {
        false
    }
}

#[cfg(test)]
pub(crate) fn reset_runtime_heap_trim_request_count() {
    runtime_heap_trim_request_count_cell().store(0, Ordering::SeqCst);
    runtime_heap_trim_last_requested_at_ms().store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn runtime_heap_trim_request_count() -> usize {
    runtime_heap_trim_request_count_cell().load(Ordering::SeqCst)
}

pub(crate) use prodex_runtime_state::{RuntimeProxyLaneAdmission, RuntimeProxyLaneLimits};

pub(super) fn runtime_proxy_lane_limits(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
) -> RuntimeProxyLaneLimits {
    let policy = runtime_policy_proxy();
    let responses_override = usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT",
        policy
            .as_ref()
            .and_then(|policy| policy.responses_active_limit),
        0,
    );
    let compact_override = usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT",
        policy
            .as_ref()
            .and_then(|policy| policy.compact_active_limit),
        0,
    );
    let websocket_override = usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT",
        policy
            .as_ref()
            .and_then(|policy| policy.websocket_active_limit),
        0,
    );
    let standard_override = usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT",
        policy
            .as_ref()
            .and_then(|policy| policy.standard_active_limit),
        0,
    );
    let limits = runtime_proxy_lane_limits_from_overrides(
        global_limit,
        worker_count,
        long_lived_worker_count,
        RuntimeProxyLaneLimitOverrides {
            responses: (responses_override > 0).then_some(responses_override),
            compact: (compact_override > 0).then_some(compact_override),
            websocket: (websocket_override > 0).then_some(websocket_override),
            standard: (standard_override > 0).then_some(standard_override),
        },
    );
    RuntimeProxyLaneLimits {
        responses: limits.responses,
        compact: limits.compact,
        websocket: limits.websocket,
        standard: limits.standard,
    }
}

pub(crate) use runtime_proxy_crate::{
    RuntimeProxyLogField, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
#[cfg(test)]
use runtime_proxy_crate::{runtime_proxy_log_event, runtime_proxy_log_fields};

fn runtime_proxy_format_log_line(message: &str, format: RuntimeLogFormat) -> String {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    let format = match format {
        RuntimeLogFormat::Text => runtime_log::RuntimeLogFormat::Text,
        RuntimeLogFormat::Json => runtime_log::RuntimeLogFormat::Json,
    };
    runtime_log::runtime_format_log_line(
        message,
        format,
        &timestamp.to_string(),
        std::process::id(),
    )
}

fn runtime_proxy_format_dropped_log_marker(marker: runtime_log::RuntimeDroppedLogMarker) -> String {
    let mut message = format!(
        "{RUNTIME_PROXY_DROPPED_LOG_EVENT} dropped_count={} reason=queue_full queue_capacity={}",
        marker.dropped_count, marker.queue_capacity
    );
    if marker.overflow {
        message.push_str(" scope=overflow");
    }
    runtime_proxy_format_log_line(&message, runtime_proxy_log_format())
}

pub(super) fn runtime_proxy_log_to_path(log_path: &Path, message: &str) {
    let logger = runtime_proxy_async_logger();
    let line = runtime_proxy_format_log_line(message, runtime_proxy_log_format());
    logger.try_enqueue(log_path, line);
    #[cfg(test)]
    if !runtime_proxy_async_logger_pause_writes() {
        logger.flush_path(log_path);
    }
}

pub(super) fn runtime_proxy_flush_logs_for_path(log_path: &Path) {
    runtime_proxy_async_logger().flush_path(log_path);
}

#[derive(Debug)]
pub(super) struct JsonFileLock {
    pub(super) file: fs::File,
}

impl Drop for JsonFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
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
}
