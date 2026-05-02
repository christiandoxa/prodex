use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const RUNTIME_LOG_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

pub const RUNTIME_ASYNC_LOG_DROPPED_EVENT: &str = "runtime_proxy_async_log_dropped";

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
            if let Some(event) = runtime_proxy::runtime_proxy_log_event(&sanitized) {
                value.insert(
                    "event".to_string(),
                    serde_json::Value::String(event.to_string()),
                );
            }
            let fields = runtime_proxy::runtime_proxy_log_fields(&sanitized);
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
}

#[derive(Debug)]
struct RuntimeAsyncLoggerInner {
    state: Mutex<RuntimeAsyncLoggerState>,
    work_available: Condvar,
    path_drained: Condvar,
    capacity: usize,
    dropped_marker_formatter: RuntimeDroppedLogMarkerFormatter,
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
    ) -> Self {
        let inner = Arc::new(RuntimeAsyncLoggerInner {
            state: Mutex::new(RuntimeAsyncLoggerState::default()),
            work_available: Condvar::new(),
            path_drained: Condvar::new(),
            capacity: capacity.max(1),
            dropped_marker_formatter,
        });
        let worker_inner = Arc::clone(&inner);
        let _ = thread::Builder::new()
            .name("prodex-runtime-log".to_string())
            .spawn(move || runtime_async_logger_worker_loop(worker_inner));
        Self { inner }
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

    pub fn flush_path(&self, log_path: &Path) {
        let deadline = Instant::now() + RUNTIME_LOG_FLUSH_TIMEOUT;
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while state.pending_by_path.get(log_path).copied().unwrap_or(0) > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return;
            }
            let (next_state, _) = self
                .inner
                .path_drained
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next_state;
        }
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
                state = inner
                    .work_available
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };

        runtime_async_logger_wait_for_write_permit();

        let mut completed_line_path = None;
        let mut completed_marker_path = None;
        if let Some(entry) = work_item.line {
            runtime_write_log_line(&entry.log_path, &entry.line);
            completed_line_path = Some(entry.log_path);
        }
        if let Some(marker) = work_item.dropped_marker {
            let line = (inner.dropped_marker_formatter)(marker.marker);
            runtime_write_log_line(&marker.log_path, &line);
            completed_marker_path = Some(marker.log_path);
        }

        let mut state = inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(log_path) = completed_line_path {
            state.decrement_pending_for_path(&log_path);
        }
        if let Some(log_path) = completed_marker_path {
            state.decrement_pending_for_path(&log_path);
        }
        inner.path_drained.notify_all();
    }
}

fn runtime_write_log_line(log_path: &Path, line: &str) {
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

#[doc(hidden)]
pub fn runtime_async_logger_writes_are_paused_for_test() -> bool {
    runtime_async_logger_pause_writes()
}
