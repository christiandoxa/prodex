use super::*;

#[cfg(all(target_os = "linux", target_env = "gnu", not(test)))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

const RUNTIME_PROXY_LOG_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_PROXY_DROPPED_LOG_EVENT: &str = "runtime_proxy_async_log_dropped";

#[derive(Debug)]
struct RuntimeProxyQueuedLogLine {
    log_path: PathBuf,
    line: String,
}

#[derive(Debug)]
struct RuntimeProxyDroppedLogCounter {
    log_path: PathBuf,
    dropped_count: u64,
}

#[derive(Debug)]
struct RuntimeProxyDroppedLogMarker {
    log_path: PathBuf,
    dropped_count: u64,
    queue_capacity: usize,
    overflow: bool,
}

#[derive(Debug)]
struct RuntimeProxyAsyncLoggerWorkItem {
    line: Option<RuntimeProxyQueuedLogLine>,
    dropped_marker: Option<RuntimeProxyDroppedLogMarker>,
}

#[derive(Debug, Default)]
struct RuntimeProxyAsyncLoggerState {
    queue: VecDeque<RuntimeProxyQueuedLogLine>,
    pending_by_path: BTreeMap<PathBuf, usize>,
    dropped_by_path: BTreeMap<PathBuf, u64>,
    dropped_overflow: Option<RuntimeProxyDroppedLogCounter>,
}

#[derive(Debug)]
struct RuntimeProxyAsyncLoggerInner {
    state: Mutex<RuntimeProxyAsyncLoggerState>,
    work_available: Condvar,
    path_drained: Condvar,
    capacity: usize,
}

#[derive(Debug, Clone)]
struct RuntimeProxyAsyncLogger {
    inner: Arc<RuntimeProxyAsyncLoggerInner>,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct RuntimeProxyAsyncLoggerTestState {
    pause_writes: bool,
}

fn runtime_proxy_log_queue_capacity() -> usize {
    thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .saturating_mul(256)
        .clamp(1024, 8192)
}

fn runtime_proxy_async_logger() -> &'static RuntimeProxyAsyncLogger {
    static LOGGER: OnceLock<RuntimeProxyAsyncLogger> = OnceLock::new();
    LOGGER.get_or_init(RuntimeProxyAsyncLogger::new)
}

#[cfg(test)]
fn runtime_proxy_async_logger_test_state()
-> &'static (Mutex<RuntimeProxyAsyncLoggerTestState>, Condvar) {
    static TEST_STATE: OnceLock<(Mutex<RuntimeProxyAsyncLoggerTestState>, Condvar)> =
        OnceLock::new();
    TEST_STATE.get_or_init(|| {
        (
            Mutex::new(RuntimeProxyAsyncLoggerTestState::default()),
            Condvar::new(),
        )
    })
}

impl RuntimeProxyAsyncLogger {
    fn new() -> Self {
        let inner = Arc::new(RuntimeProxyAsyncLoggerInner {
            state: Mutex::new(RuntimeProxyAsyncLoggerState::default()),
            work_available: Condvar::new(),
            path_drained: Condvar::new(),
            capacity: runtime_proxy_log_queue_capacity(),
        });
        let worker_inner = Arc::clone(&inner);
        let _ = thread::Builder::new()
            .name("prodex-runtime-log".to_string())
            .spawn(move || runtime_proxy_async_logger_worker_loop(worker_inner));
        Self { inner }
    }

    fn try_enqueue(&self, log_path: &Path, line: String) {
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
        state.queue.push_back(RuntimeProxyQueuedLogLine {
            log_path: log_path.to_path_buf(),
            line,
        });
        self.inner.work_available.notify_one();
    }

    fn flush_path(&self, log_path: &Path) {
        let deadline = Instant::now() + RUNTIME_PROXY_LOG_FLUSH_TIMEOUT;
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

    #[cfg(test)]
    fn pending_count_for_path(&self, log_path: &Path) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pending_by_path
            .get(log_path)
            .copied()
            .unwrap_or(0)
    }

    #[cfg(test)]
    fn capacity(&self) -> usize {
        self.inner.capacity
    }
}

impl RuntimeProxyAsyncLoggerState {
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

        self.dropped_overflow = Some(RuntimeProxyDroppedLogCounter {
            log_path: log_path.to_path_buf(),
            dropped_count: 1,
        });
        self.increment_pending_for_path(log_path);
    }

    fn pop_dropped_marker(
        &mut self,
        queue_capacity: usize,
    ) -> Option<RuntimeProxyDroppedLogMarker> {
        if let Some((log_path, dropped_count)) = self.dropped_by_path.pop_first() {
            return Some(RuntimeProxyDroppedLogMarker {
                log_path,
                dropped_count,
                queue_capacity,
                overflow: false,
            });
        }

        self.dropped_overflow
            .take()
            .map(|counter| RuntimeProxyDroppedLogMarker {
                log_path: counter.log_path,
                dropped_count: counter.dropped_count,
                queue_capacity,
                overflow: true,
            })
    }

    fn pop_work_item(&mut self, queue_capacity: usize) -> Option<RuntimeProxyAsyncLoggerWorkItem> {
        if let Some(entry) = self.queue.pop_front() {
            return Some(RuntimeProxyAsyncLoggerWorkItem {
                line: Some(entry),
                dropped_marker: self.pop_dropped_marker(queue_capacity),
            });
        }

        self.pop_dropped_marker(queue_capacity)
            .map(|dropped_marker| RuntimeProxyAsyncLoggerWorkItem {
                line: None,
                dropped_marker: Some(dropped_marker),
            })
    }
}

#[cfg(test)]
fn runtime_proxy_async_logger_set_pause_writes(paused: bool) {
    let (mutex, condvar) = runtime_proxy_async_logger_test_state();
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.pause_writes = paused;
    if !paused {
        condvar.notify_all();
    }
}

#[cfg(test)]
fn runtime_proxy_async_logger_pause_writes() -> bool {
    runtime_proxy_async_logger_test_state()
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .pause_writes
}

#[cfg(test)]
fn runtime_proxy_async_logger_wait_for_write_permit() {
    let (mutex, condvar) = runtime_proxy_async_logger_test_state();
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while state.pause_writes {
        state = condvar
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn runtime_proxy_async_logger_worker_loop(inner: Arc<RuntimeProxyAsyncLoggerInner>) {
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

        #[cfg(test)]
        runtime_proxy_async_logger_wait_for_write_permit();

        let mut completed_line_path = None;
        let mut completed_marker_path = None;
        if let Some(entry) = work_item.line {
            runtime_proxy_write_log_line(&entry.log_path, &entry.line);
            completed_line_path = Some(entry.log_path);
        }
        if let Some(marker) = work_item.dropped_marker {
            let line = runtime_proxy_format_dropped_log_marker(&marker);
            runtime_proxy_write_log_line(&marker.log_path, &line);
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

fn runtime_proxy_write_log_line(log_path: &Path, line: &str) {
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
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

pub(super) fn runtime_proxy_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(4, 12)
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

pub(super) fn runtime_proxy_long_lived_worker_count_default(parallelism: usize) -> usize {
    parallelism.saturating_mul(2).clamp(8, 24)
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
        parallelism.clamp(2, 4),
    )
    .clamp(1, 8)
}

pub(super) fn runtime_proxy_async_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(2, 4)
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
    let default_capacity = worker_count.saturating_mul(8).clamp(128, 1024);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY",
        runtime_policy_proxy().and_then(|policy| policy.long_lived_queue_capacity),
        default_capacity,
    )
    .max(1)
}

pub(super) fn runtime_proxy_active_request_limit_default(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    worker_count
        .saturating_add(long_lived_worker_count.saturating_mul(3))
        .clamp(64, 512)
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

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeProxyLaneLimits {
    pub(super) responses: usize,
    pub(super) compact: usize,
    pub(super) websocket: usize,
    pub(super) standard: usize,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeProxyLaneAdmission {
    pub(super) responses_active: Arc<AtomicUsize>,
    pub(super) compact_active: Arc<AtomicUsize>,
    pub(super) websocket_active: Arc<AtomicUsize>,
    pub(super) standard_active: Arc<AtomicUsize>,
    pub(super) responses_admissions_total: Arc<AtomicU64>,
    pub(super) compact_admissions_total: Arc<AtomicU64>,
    pub(super) websocket_admissions_total: Arc<AtomicU64>,
    pub(super) standard_admissions_total: Arc<AtomicU64>,
    pub(super) responses_global_limit_rejections_total: Arc<AtomicU64>,
    pub(super) compact_global_limit_rejections_total: Arc<AtomicU64>,
    pub(super) websocket_global_limit_rejections_total: Arc<AtomicU64>,
    pub(super) standard_global_limit_rejections_total: Arc<AtomicU64>,
    pub(super) responses_lane_limit_rejections_total: Arc<AtomicU64>,
    pub(super) compact_lane_limit_rejections_total: Arc<AtomicU64>,
    pub(super) websocket_lane_limit_rejections_total: Arc<AtomicU64>,
    pub(super) standard_lane_limit_rejections_total: Arc<AtomicU64>,
    pub(super) wait: Arc<(Mutex<()>, Condvar)>,
    pub(super) inflight_release_revision: Arc<AtomicU64>,
    pub(super) limits: RuntimeProxyLaneLimits,
}

impl RuntimeProxyLaneAdmission {
    pub(super) fn new(limits: RuntimeProxyLaneLimits) -> Self {
        Self {
            responses_active: Arc::new(AtomicUsize::new(0)),
            compact_active: Arc::new(AtomicUsize::new(0)),
            websocket_active: Arc::new(AtomicUsize::new(0)),
            standard_active: Arc::new(AtomicUsize::new(0)),
            responses_admissions_total: Arc::new(AtomicU64::new(0)),
            compact_admissions_total: Arc::new(AtomicU64::new(0)),
            websocket_admissions_total: Arc::new(AtomicU64::new(0)),
            standard_admissions_total: Arc::new(AtomicU64::new(0)),
            responses_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            compact_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            websocket_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            standard_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            responses_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            compact_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            websocket_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            standard_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            wait: Arc::new((Mutex::new(()), Condvar::new())),
            inflight_release_revision: Arc::new(AtomicU64::new(0)),
            limits,
        }
    }

    pub(super) fn active_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicUsize> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_active),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_active),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_active),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_active),
        }
    }

    pub(super) fn limit(&self, lane: RuntimeRouteKind) -> usize {
        match lane {
            RuntimeRouteKind::Responses => self.limits.responses,
            RuntimeRouteKind::Compact => self.limits.compact,
            RuntimeRouteKind::Websocket => self.limits.websocket,
            RuntimeRouteKind::Standard => self.limits.standard,
        }
    }

    pub(super) fn admissions_total_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_admissions_total),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_admissions_total),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_admissions_total),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_admissions_total),
        }
    }

    pub(super) fn global_limit_rejections_total_counter(
        &self,
        lane: RuntimeRouteKind,
    ) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => {
                Arc::clone(&self.responses_global_limit_rejections_total)
            }
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_global_limit_rejections_total),
            RuntimeRouteKind::Websocket => {
                Arc::clone(&self.websocket_global_limit_rejections_total)
            }
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_global_limit_rejections_total),
        }
    }

    pub(super) fn lane_limit_rejections_total_counter(
        &self,
        lane: RuntimeRouteKind,
    ) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_lane_limit_rejections_total),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_lane_limit_rejections_total),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_lane_limit_rejections_total),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_lane_limit_rejections_total),
        }
    }
}

pub(super) fn runtime_proxy_lane_limits(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
) -> RuntimeProxyLaneLimits {
    let global_limit = global_limit.max(1);
    RuntimeProxyLaneLimits {
        responses: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.responses_active_limit),
            (global_limit.saturating_mul(3) / 4).clamp(4, global_limit),
        )
        .min(global_limit)
        .max(1),
        compact: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.compact_active_limit),
            (global_limit / 4).clamp(2, 6).min(global_limit),
        )
        .min(global_limit)
        .max(1),
        websocket: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.websocket_active_limit),
            long_lived_worker_count.clamp(2, global_limit),
        )
        .min(global_limit)
        .max(1),
        standard: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.standard_active_limit),
            (worker_count / 2).clamp(2, 8).min(global_limit),
        )
        .min(global_limit)
        .max(1),
    }
}

pub(super) fn runtime_proxy_log_fields(message: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for token in message.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    fields
}

pub(super) fn runtime_proxy_log_event(message: &str) -> Option<&str> {
    message
        .split_whitespace()
        .find(|token| !token.contains('='))
        .filter(|token| !token.is_empty())
}

fn runtime_proxy_format_log_line(message: &str, format: RuntimeLogFormat) -> String {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    let sanitized = message.replace(['\r', '\n'], " ");
    match format {
        RuntimeLogFormat::Text => format!("[{timestamp}] {sanitized}\n"),
        RuntimeLogFormat::Json => {
            let mut value = serde_json::Map::new();
            value.insert(
                "timestamp".to_string(),
                serde_json::Value::String(timestamp.to_string()),
            );
            value.insert(
                "pid".to_string(),
                serde_json::Value::Number(std::process::id().into()),
            );
            value.insert(
                "message".to_string(),
                serde_json::Value::String(sanitized.clone()),
            );
            if let Some(event) = runtime_proxy_log_event(&sanitized) {
                value.insert(
                    "event".to_string(),
                    serde_json::Value::String(event.to_string()),
                );
            }
            let fields = runtime_proxy_log_fields(&sanitized);
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

fn runtime_proxy_format_dropped_log_marker(marker: &RuntimeProxyDroppedLogMarker) -> String {
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
            "selection route=responses profile=main note=\"hello\"\nnext",
        );
        runtime_proxy_flush_logs_for_path(&log_path);

        let line = fs::read_to_string(&log_path).expect("json runtime log should be readable");
        let value = serde_json::from_str::<serde_json::Value>(line.trim_end())
            .expect("runtime log line should be valid json");

        assert_eq!(
            value.get("message").and_then(|value| value.as_str()),
            Some("selection route=responses profile=main note=\"hello\" next")
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
            Some("hello")
        );
        assert_eq!(
            value.get("pid").and_then(|value| value.as_u64()),
            Some(std::process::id().into())
        );
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
