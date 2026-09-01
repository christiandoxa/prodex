mod live;
mod retention;

use live::RuntimeLiveLogStore;
pub use live::{
    DEFAULT_RUNTIME_LIVE_LOG_MAX_BYTES, DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES, RuntimeLiveLogEntry,
    RuntimeLiveLogSnapshot,
};
#[cfg(test)]
use retention::RUNTIME_LOG_FILE_PREFIX;
pub use retention::{
    DEFAULT_RUNTIME_LOG_MAX_AGE_SECONDS, DEFAULT_RUNTIME_LOG_MAX_FILE_BYTES,
    DEFAULT_RUNTIME_LOG_MAX_FILES, DEFAULT_RUNTIME_LOG_TOTAL_BYTES, RuntimeLogCleanupReport,
    RuntimeLogPolicy, cleanup_runtime_log_directory, cleanup_runtime_log_directory_with_prefix,
};
use retention::{RuntimeLogWriterState, write_log_line};
use runtime_proxy_crate as runtime_proxy;
use std::collections::{BTreeMap, VecDeque};
#[cfg(test)]
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

const RUNTIME_LOG_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

pub const RUNTIME_ASYNC_LOG_DROPPED_EVENT: &str = "runtime_proxy_async_log_dropped";

/// Returns whether explicit bounded raw runtime-log recording is enabled.
pub fn runtime_log_recording_enabled() -> bool {
    retention::runtime_log_recording_enabled()
}

/// Returns whether a runtime message is routine capacity telemetry rather than a user-facing
/// operational event. These messages are intentionally admitted only to bounded diagnostics.
pub fn runtime_log_message_is_routine_load(message: &str) -> bool {
    let event = runtime_proxy::runtime_proxy_parse_log_message(message);
    matches!(
        event.event(),
        Some(
            "profile_inflight_saturated"
                | "runtime_proxy_active_limit_reached"
                | "runtime_proxy_lane_limit_reached",
        )
    )
}

pub fn decode_zstd_bounded(payload: &[u8], max_bytes: usize) -> io::Result<Vec<u8>> {
    zstd::bulk::decompress(payload, max_bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDroppedLogMarker {
    pub dropped_count: u64,
    pub queue_capacity: usize,
    pub overflow: bool,
}

pub type RuntimeDroppedLogMarkerFormatter = fn(RuntimeDroppedLogMarker) -> String;

pub fn runtime_format_log_line(
    message: &str,
    format: RuntimeLogFormat,
    timestamp: &str,
    pid: u32,
) -> String {
    let sanitized = message.replace(['\r', '\n'], " ");
    match format {
        RuntimeLogFormat::Text => format!("[{timestamp}] {sanitized}\n"),
        RuntimeLogFormat::Json => {
            let mut value = serde_json::Map::new();
            value.insert(
                "timestamp".to_string(),
                serde_json::Value::String(timestamp.to_string()),
            );
            value.insert("pid".to_string(), serde_json::Value::Number(pid.into()));
            value.insert(
                "message".to_string(),
                serde_json::Value::String(sanitized.clone()),
            );
            let parsed = runtime_proxy::runtime_proxy_parse_log_message(&sanitized);
            if let Some(event) = parsed.event() {
                value.insert(
                    "event".to_string(),
                    serde_json::Value::String(event.to_string()),
                );
            }
            let fields = parsed.fields_map();
            if !fields.is_empty() {
                value.insert(
                    "fields".to_string(),
                    serde_json::Value::Object(
                        fields
                            .into_iter()
                            .map(|(key, value)| (key, serde_json::Value::String(value)))
                            .collect(),
                    ),
                );
            }
            match serde_json::to_string(&serde_json::Value::Object(value)) {
                Ok(serialized) => format!("{serialized}\n"),
                Err(_) => format!("[{timestamp}] {sanitized}\n"),
            }
        }
    }
}

#[derive(Debug)]
struct RuntimeQueuedLogLine {
    log_path: PathBuf,
    line: String,
}

#[derive(Debug)]
struct RuntimeDroppedLogCounter {
    log_path: PathBuf,
    dropped_count: u64,
}

#[derive(Debug)]
struct RuntimeDroppedLogWorkItem {
    log_path: PathBuf,
    marker: RuntimeDroppedLogMarker,
}

#[derive(Debug)]
struct RuntimeAsyncLoggerWorkItem {
    line: Option<RuntimeQueuedLogLine>,
    dropped_marker: Option<RuntimeDroppedLogWorkItem>,
}

#[derive(Debug, Default)]
struct RuntimeAsyncLoggerState {
    queue: VecDeque<RuntimeQueuedLogLine>,
    pending_by_path: BTreeMap<PathBuf, usize>,
    dropped_by_path: BTreeMap<PathBuf, u64>,
    dropped_overflow: Option<RuntimeDroppedLogCounter>,
    errors_by_path: BTreeMap<PathBuf, (io::ErrorKind, String)>,
}

#[derive(Debug)]
struct RuntimeAsyncLoggerInner {
    state: Mutex<RuntimeAsyncLoggerState>,
    writer: Mutex<RuntimeLogWriterState>,
    live: RuntimeLiveLogStore,
    work_available: Condvar,
    path_drained: Condvar,
    capacity: usize,
    dropped_marker_formatter: RuntimeDroppedLogMarkerFormatter,
    policy: RuntimeLogPolicy,
}

#[derive(Debug, Clone)]
pub struct RuntimeAsyncLogger {
    inner: Arc<RuntimeAsyncLoggerInner>,
}

#[derive(Debug, Default)]
struct RuntimeAsyncLoggerTestState {
    pause_writes: bool,
}

fn runtime_async_logger_test_state() -> &'static (Mutex<RuntimeAsyncLoggerTestState>, Condvar) {
    static TEST_STATE: OnceLock<(Mutex<RuntimeAsyncLoggerTestState>, Condvar)> = OnceLock::new();
    TEST_STATE.get_or_init(|| {
        (
            Mutex::new(RuntimeAsyncLoggerTestState::default()),
            Condvar::new(),
        )
    })
}

impl RuntimeAsyncLogger {
    pub fn new(
        capacity: usize,
        dropped_marker_formatter: RuntimeDroppedLogMarkerFormatter,
    ) -> io::Result<Self> {
        Self::new_with_recording(
            capacity,
            dropped_marker_formatter,
            retention::runtime_log_recording_enabled(),
        )
    }

    pub fn new_with_recording(
        capacity: usize,
        dropped_marker_formatter: RuntimeDroppedLogMarkerFormatter,
        record_to_disk: bool,
    ) -> io::Result<Self> {
        Self::new_with_policy(
            capacity,
            dropped_marker_formatter,
            RuntimeLogPolicy {
                record_to_disk,
                ..RuntimeLogPolicy::from_environment()
            },
        )
    }

    fn new_with_policy(
        capacity: usize,
        dropped_marker_formatter: RuntimeDroppedLogMarkerFormatter,
        policy: RuntimeLogPolicy,
    ) -> io::Result<Self> {
        let inner = Arc::new(RuntimeAsyncLoggerInner {
            state: Mutex::new(RuntimeAsyncLoggerState::default()),
            writer: Mutex::new(RuntimeLogWriterState::default()),
            live: RuntimeLiveLogStore::default(),
            work_available: Condvar::new(),
            path_drained: Condvar::new(),
            capacity: capacity.max(1),
            dropped_marker_formatter,
            policy: policy.normalized(),
        });
        let worker_inner = Arc::clone(&inner);
        thread::Builder::new()
            .name("prodex-runtime-log".to_string())
            .spawn(move || runtime_async_logger_worker_loop(worker_inner))?;
        Ok(Self { inner })
    }

    pub fn try_enqueue(&self, log_path: &Path, line: String) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.queue.len() >= self.inner.capacity {
            state.note_dropped_log(log_path, self.inner.capacity);
            self.inner.work_available.notify_one();
            return;
        }
        state.increment_pending_for_path(log_path);
        state.queue.push_back(RuntimeQueuedLogLine {
            log_path: log_path.to_path_buf(),
            line,
        });
        self.inner.work_available.notify_one();
    }

    pub fn flush_path(&self, log_path: &Path) -> io::Result<()> {
        let deadline = Instant::now() + RUNTIME_LOG_FLUSH_TIMEOUT;
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while state.pending_by_path.get(log_path).copied().unwrap_or(0) > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out flushing runtime log",
                ));
            }
            let (next_state, _) = self
                .inner
                .path_drained
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next_state;
        }
        match state.errors_by_path.remove(log_path) {
            Some((kind, message)) => Err(io::Error::new(kind, message)),
            None => Ok(()),
        }
    }

    pub fn live_log_snapshot_after(
        &self,
        log_path: &Path,
        after: u64,
        limit: usize,
    ) -> RuntimeLiveLogSnapshot {
        self.inner.live.snapshot_after(log_path, after, limit)
    }

    #[doc(hidden)]
    pub fn pending_count_for_path(&self, log_path: &Path) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pending_by_path
            .get(log_path)
            .copied()
            .unwrap_or(0)
    }

    #[doc(hidden)]
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    #[doc(hidden)]
    pub fn set_pause_writes_for_test(paused: bool) {
        let (mutex, condvar) = runtime_async_logger_test_state();
        let mut state = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.pause_writes = paused;
        if !paused {
            condvar.notify_all();
        }
    }
}

impl RuntimeAsyncLoggerState {
    fn increment_pending_for_path(&mut self, log_path: &Path) {
        self.pending_by_path
            .entry(log_path.to_path_buf())
            .and_modify(|pending| *pending += 1)
            .or_insert(1);
    }

    fn decrement_pending_for_path(&mut self, log_path: &Path) {
        if let Some(pending) = self.pending_by_path.get_mut(log_path) {
            *pending = pending.saturating_sub(1);
            if *pending == 0 {
                self.pending_by_path.remove(log_path);
            }
        }
    }

    fn record_write_error(&mut self, log_path: &Path, error: io::Error) {
        self.errors_by_path
            .insert(log_path.to_path_buf(), (error.kind(), error.to_string()));
    }

    fn note_dropped_log(&mut self, log_path: &Path, dropped_path_limit: usize) {
        if let Some(dropped_count) = self.dropped_by_path.get_mut(log_path) {
            *dropped_count = dropped_count.saturating_add(1);
            return;
        }

        if self.dropped_by_path.len() < dropped_path_limit {
            self.dropped_by_path.insert(log_path.to_path_buf(), 1);
            self.increment_pending_for_path(log_path);
            return;
        }

        if let Some(overflow) = self.dropped_overflow.as_mut() {
            overflow.dropped_count = overflow.dropped_count.saturating_add(1);
            return;
        }

        self.dropped_overflow = Some(RuntimeDroppedLogCounter {
            log_path: log_path.to_path_buf(),
            dropped_count: 1,
        });
        self.increment_pending_for_path(log_path);
    }

    fn pop_dropped_marker(&mut self, queue_capacity: usize) -> Option<RuntimeDroppedLogWorkItem> {
        if let Some((log_path, dropped_count)) = self.dropped_by_path.pop_first() {
            return Some(RuntimeDroppedLogWorkItem {
                log_path,
                marker: RuntimeDroppedLogMarker {
                    dropped_count,
                    queue_capacity,
                    overflow: false,
                },
            });
        }

        self.dropped_overflow
            .take()
            .map(|counter| RuntimeDroppedLogWorkItem {
                log_path: counter.log_path,
                marker: RuntimeDroppedLogMarker {
                    dropped_count: counter.dropped_count,
                    queue_capacity,
                    overflow: true,
                },
            })
    }

    fn pop_work_item(&mut self, queue_capacity: usize) -> Option<RuntimeAsyncLoggerWorkItem> {
        if let Some(entry) = self.queue.pop_front() {
            return Some(RuntimeAsyncLoggerWorkItem {
                line: Some(entry),
                dropped_marker: self.pop_dropped_marker(queue_capacity),
            });
        }

        self.pop_dropped_marker(queue_capacity)
            .map(|dropped_marker| RuntimeAsyncLoggerWorkItem {
                line: None,
                dropped_marker: Some(dropped_marker),
            })
    }
}

fn runtime_async_logger_pause_writes() -> bool {
    runtime_async_logger_test_state()
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .pause_writes
}

fn runtime_async_logger_wait_for_write_permit() {
    let (mutex, condvar) = runtime_async_logger_test_state();
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while state.pause_writes {
        state = condvar
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn runtime_async_logger_worker_loop(inner: Arc<RuntimeAsyncLoggerInner>) {
    loop {
        let work_item = {
            let mut state = inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            loop {
                if let Some(work_item) = state.pop_work_item(inner.capacity) {
                    break work_item;
                }
                if Arc::strong_count(&inner) == 1 {
                    return;
                }
                state = inner
                    .work_available
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };

        runtime_async_logger_wait_for_write_permit();

        let mut completed = Vec::with_capacity(2);
        if let Some(entry) = work_item.line {
            let result = write_log_line(&inner, &entry.log_path, &entry.line);
            completed.push((entry.log_path, result));
        }
        if let Some(marker) = work_item.dropped_marker {
            let line = (inner.dropped_marker_formatter)(marker.marker);
            let result = write_log_line(&inner, &marker.log_path, &line);
            completed.push((marker.log_path, result));
        }

        let mut state = inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (log_path, result) in completed {
            if let Err(error) = result {
                state.record_write_error(&log_path, error);
            }
            state.decrement_pending_for_path(&log_path);
        }
        inner.path_drained.notify_all();
    }
}

impl Drop for RuntimeAsyncLogger {
    fn drop(&mut self) {
        self.inner.work_available.notify_one();
    }
}

#[doc(hidden)]
pub fn runtime_async_logger_writes_are_paused_for_test() -> bool {
    runtime_async_logger_pause_writes()
}

#[cfg(test)]
mod tests {
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
    fn runtime_log_rotates_complete_records_by_bytes() {
        let root = test_path("rotate");
        fs::create_dir_all(&root).unwrap();
        let path = runtime_log_path(&root, "active");
        create_empty_log(&path);
        let logger =
            RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(5, 10, 1024))
                .unwrap();

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
            RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(4, 10, 1024))
                .unwrap();

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
        let active = runtime_log_path(&root, "active");
        create_empty_log(&active);
        let logger =
            RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(1024, 8, 8))
                .unwrap();
        logger.try_enqueue(&active, "active\n".to_string());
        logger.flush_path(&active).unwrap();

        for name in ["old-a", "old-b", "old-c"] {
            fs::write(runtime_log_path(&root, name), "12345\n").unwrap();
        }
        let report =
            cleanup_runtime_log_directory(&root, SystemTime::now(), test_policy(1024, 8, 8));

        assert!(report.removed >= 2);
        assert!(active.exists());
        let total = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("log"))
            .map(|path| fs::metadata(path).unwrap().len())
            .sum::<u64>();
        assert!(total <= 8, "runtime log budget exceeded: {total}");

        drop(logger);
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
            RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(1024, 8, 1024))
                .unwrap();

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
            RuntimeAsyncLogger::new_with_policy(4, dropped_marker, test_policy(1024, 8, 1024))
                .unwrap();

        logger.try_enqueue(&link, "entry\n".to_string());
        assert!(logger.flush_path(&link).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original\n");

        let _ = fs::remove_file(link);
        let _ = fs::remove_file(target);
    }
}
