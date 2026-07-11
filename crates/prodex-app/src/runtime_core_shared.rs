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

pub(crate) fn runtime_proxy_latest_log_path_from_pointer() -> Option<PathBuf> {
    runtime_proxy_latest_log_path_from_pointer_text(
        &runtime_proxy_log_dir(),
        &fs::read_to_string(runtime_proxy_latest_log_pointer_path()).ok()?,
    )
}

pub(crate) fn runtime_proxy_latest_log_path_from_pointer_text(
    log_dir: &Path,
    pointer_text: &str,
) -> Option<PathBuf> {
    let path = PathBuf::from(pointer_text.lines().next()?.trim());
    if path.as_os_str().is_empty() {
        return None;
    }
    let file_name = path.file_name().and_then(|name| name.to_str())?;
    if !file_name.starts_with(RUNTIME_PROXY_LOG_FILE_PREFIX) || !file_name.ends_with(".log") {
        return None;
    }
    let parent = path.parent()?;
    if !prodex_core::same_path(parent, log_dir) {
        return None;
    }
    if !prodex_core::path_is_under_root(log_dir, &path) {
        return None;
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => None,
        Ok(_) => Some(path),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Some(path),
        Err(_) => None,
    }
}

pub(super) fn initialize_runtime_proxy_log_path() -> PathBuf {
    let log_path = create_runtime_proxy_log_path();
    let _ = write_runtime_proxy_latest_log_pointer(&log_path);
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

fn write_runtime_proxy_latest_log_pointer(log_path: &Path) -> io::Result<()> {
    let pointer_path = runtime_proxy_latest_log_pointer_path();
    if let Some(parent) = pointer_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = runtime_proxy_latest_log_pointer_temp_path(&pointer_path);
    write_runtime_proxy_private_file(&temp_path, format!("{}\n", log_path.display()).as_bytes())?;
    if let Err(err) = fs::rename(&temp_path, &pointer_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }
    Ok(())
}

fn runtime_proxy_latest_log_pointer_temp_path(pointer_path: &Path) -> PathBuf {
    let file_name = pointer_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(RUNTIME_PROXY_LATEST_LOG_POINTER);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = RUNTIME_PROXY_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    pointer_path.with_file_name(format!(
        "{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        millis,
        sequence
    ))
}

fn write_runtime_proxy_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = open_runtime_proxy_private_file(path)?;
    file.write_all(bytes)
}

#[cfg(unix)]
fn open_runtime_proxy_private_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_runtime_proxy_private_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
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
#[path = "../tests/src/runtime_core_shared.rs"]
mod tests;
