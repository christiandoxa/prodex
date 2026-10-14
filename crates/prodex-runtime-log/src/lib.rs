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

pub fn decode_zstd_bounded(payload: &[u8], max_bytes: usize) -> io::Result<Vec<u8>> {
    zstd::bulk::decompress(payload, max_bytes)
}

/// Returns whether a runtime message is routine capacity telemetry rather than a user-facing
/// operational event. Human renderers may filter these after JSON/live capture.
pub fn runtime_log_message_is_routine_load(message: &str) -> bool {
    let event = runtime_proxy::runtime_proxy_parse_log_message(message);
    event
        .event()
        .and_then(|name| prodex_mojo_core::log_load::try_is_routine_event(name).ok())
        .unwrap_or(false)
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
    let sanitized =
        runtime_proxy::runtime_proxy_redact_log_text(message).replace(['\r', '\n'], " ");
    // Text logs never required structured parsing; preserve the old hot-path
    // cost while Mojo owns both final output formats.
    let (format, event, fields) = match format {
        RuntimeLogFormat::Text => (
            prodex_mojo_core::log::RUNTIME_LOG_FORMAT_TEXT,
            None,
            BTreeMap::new(),
        ),
        RuntimeLogFormat::Json => {
            let parsed = runtime_proxy::runtime_proxy_parse_log_message(&sanitized);
            (
                prodex_mojo_core::log::RUNTIME_LOG_FORMAT_JSON,
                parsed.event().map(str::to_owned),
                parsed.fields_map(),
            )
        }
    };
    prodex_mojo_core::log::render_runtime_log_line(
        format,
        timestamp,
        pid,
        &sanitized,
        event.as_deref(),
        &fields,
    )
    .expect("Mojo runtime-log renderer returned invalid output")
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
        let Some(work_item) = next_runtime_async_logger_work_item(&inner) else {
            return;
        };

        runtime_async_logger_wait_for_write_permit();
        complete_runtime_async_logger_work_item(&inner, work_item);
    }
}

fn next_runtime_async_logger_work_item(
    inner: &Arc<RuntimeAsyncLoggerInner>,
) -> Option<RuntimeAsyncLoggerWorkItem> {
    let mut state = inner
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        if let Some(work_item) = state.pop_work_item(inner.capacity) {
            return Some(work_item);
        }
        if Arc::strong_count(inner) == 1 {
            return None;
        }
        state = inner
            .work_available
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn complete_runtime_async_logger_work_item(
    inner: &Arc<RuntimeAsyncLoggerInner>,
    work_item: RuntimeAsyncLoggerWorkItem,
) {
    let mut completed = Vec::with_capacity(2);
    if let Some(entry) = work_item.line {
        let result = write_log_line(inner, &entry.log_path, &entry.line);
        completed.push((entry.log_path, result));
    }
    if let Some(marker) = work_item.dropped_marker {
        let line = (inner.dropped_marker_formatter)(marker.marker);
        let result = write_log_line(inner, &marker.log_path, &line);
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

impl Drop for RuntimeAsyncLogger {
    fn drop(&mut self) {
        self.inner.work_available.notify_one();
    }
}

#[cfg(test)]
#[path = "../tests/src/runtime_log.rs"]
mod tests;
